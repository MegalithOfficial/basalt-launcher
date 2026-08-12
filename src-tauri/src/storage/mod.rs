pub mod graph;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    config::Instance, db::Db, error::Result, files::FileManager, paths::Paths, state::AppState,
    tasks::TaskHandle,
};

#[derive(Clone)]
pub struct Store {
    pub files: FileManager,
    pub paths: Paths,
    pub db: Db,
}

impl Store {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            files: state.files.clone(),
            paths: state.paths.clone(),
            db: state.db.clone(),
        }
    }
}

const PART_SUFFIXES: &[&str] = &[".basalt-part", ".part"];

#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub id: String,
    pub label: String,
    pub bytes: u64,
    pub path: Option<String>,
    pub children: Vec<Entry>,
}

impl Entry {
    fn leaf(id: &str, label: impl Into<String>, bytes: u64, path: Option<&Path>) -> Self {
        Self {
            id: id.to_string(),
            label: label.into(),
            bytes,
            path: path.map(|value| value.display().to_string()),
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Cache,
    Shared,
    Spare,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reclaimable {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub bytes: u64,
    pub count: u64,
    pub tier: Tier,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageReport {
    pub scanned_at: i64,
    pub root: String,
    pub total_bytes: u64,
    pub free_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub buckets: Vec<Entry>,
    pub reclaimable: Vec<Reclaimable>,
    pub unresolved: Option<String>,
    pub shared_dedupe: bool,
}

#[derive(Default)]
struct Counted {
    #[cfg(unix)]
    inodes: HashSet<u64>,
}

impl Counted {
    #[cfg(unix)]
    fn accept(&mut self, metadata: &std::fs::Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() <= 1 {
            return true;
        }
        self.inodes.insert(metadata.ino())
    }

    #[cfg(not(unix))]
    fn accept(&mut self, _metadata: &std::fs::Metadata) -> bool {
        true
    }
}

struct Walk {
    bytes: u64,
    partials: Vec<(PathBuf, u64)>,
}

fn walk(files: &FileManager, root: &Path, counted: &mut Counted) -> Walk {
    let mut pending = vec![root.to_path_buf()];
    let mut walk = Walk {
        bytes: 0,
        partials: Vec::new(),
    };

    while let Some(directory) = pending.pop() {
        let Ok(entries) = files.read_external_dir(&directory) else {
            continue;
        };
        for path in entries {
            let Ok(metadata) = files.external_symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            let size = metadata.len();
            let name = path
                .file_name()
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_default();
            if PART_SUFFIXES.iter().any(|suffix| name.ends_with(suffix)) {
                walk.partials.push((path.clone(), size));
            }
            if !counted.accept(&metadata) {
                continue;
            }
            walk.bytes += size;
        }
    }

    walk
}

fn size_of(files: &FileManager, path: &Path, counted: &mut Counted) -> u64 {
    walk(files, path, counted).bytes
}

pub fn directory_size(files: &FileManager, path: &Path) -> u64 {
    size_of(files, path, &mut Counted::default())
}

fn file_size(files: &FileManager, path: &Path) -> u64 {
    files
        .external_symlink_metadata(path)
        .map(|metadata| {
            if metadata.is_file() {
                metadata.len()
            } else {
                0
            }
        })
        .unwrap_or(0)
}

fn instance_breakdown(
    files: &FileManager,
    instance: &Instance,
    counted: &mut Counted,
) -> (u64, Vec<Entry>) {
    let root = PathBuf::from(&instance.dir);
    let mut children = Vec::new();
    let mut total = 0;
    let mut loose = 0;

    let Ok(entries) = files.read_external_dir(&root) else {
        return (0, children);
    };

    for path in entries {
        let Ok(metadata) = files.external_symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        if metadata.is_dir() {
            let bytes = size_of(files, &path, counted);
            total += bytes;
            if bytes > 0 {
                children.push(Entry::leaf(&name, name.clone(), bytes, Some(&path)));
            }
        } else if metadata.is_file() && counted.accept(&metadata) {
            loose += metadata.len();
            total += metadata.len();
        }
    }

    children.sort_by_key(|entry| std::cmp::Reverse(entry.bytes));
    if loose > 0 {
        children.push(Entry::leaf("loose", "Loose files", loose, None));
    }
    (total, children)
}

fn orphans_under(files: &FileManager, root: &Path, live: &HashSet<PathBuf>) -> (u64, u64) {
    let mut pending = vec![root.to_path_buf()];
    let mut counted = Counted::default();
    let mut bytes = 0;
    let mut count = 0;

    while let Some(directory) = pending.pop() {
        let Ok(entries) = files.read_external_dir(&directory) else {
            continue;
        };
        for path in entries {
            let Ok(metadata) = files.external_symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() && !live.contains(&path) && counted.accept(&metadata) {
                bytes += metadata.len();
                count += 1;
            }
        }
    }

    (bytes, count)
}

fn directory_names(files: &FileManager, root: &Path) -> Vec<String> {
    let Ok(entries) = files.read_external_dir(root) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .filter(|path| {
            files
                .external_symlink_metadata(path)
                .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
                .unwrap_or(false)
        })
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect()
}

fn measure_dirs(files: &FileManager, parent: &Path, names: &[String]) -> u64 {
    let mut counted = Counted::default();
    names
        .iter()
        .map(|name| size_of(files, &parent.join(name), &mut counted))
        .sum()
}

fn unreferenced(
    files: &FileManager,
    paths: &Paths,
    instances: &[Instance],
) -> Result<(Vec<Reclaimable>, Option<String>)> {
    if instances.is_empty() {
        return Ok((
            Vec::new(),
            Some("There are no instances, so nothing here counts as unused yet.".to_string()),
        ));
    }

    let live = match graph::resolve(files, paths, instances)? {
        graph::Graph::Unavailable(reason) => return Ok((Vec::new(), Some(reason))),
        graph::Graph::Resolved(live) => live,
    };

    let mut found = Vec::new();

    let mut versions: Vec<String> = directory_names(files, &paths.versions())
        .into_iter()
        .filter(|name| !live.versions.contains(name))
        .collect();
    versions.sort();
    if !versions.is_empty() {
        found.push(Reclaimable {
            id: "orphan-versions".to_string(),
            label: "Unused versions".to_string(),
            detail: "No instance can reach these. Repair fetches one again if you ever need it."
                .to_string(),
            bytes: measure_dirs(files, &paths.versions(), &versions),
            count: versions.len() as u64,
            tier: Tier::Shared,
            items: versions,
        });
    }

    let mut natives: Vec<String> = directory_names(files, &paths.natives())
        .into_iter()
        .filter(|name| !live.natives.contains(name))
        .collect();
    natives.sort();
    if !natives.is_empty() {
        found.push(Reclaimable {
            id: "orphan-natives".to_string(),
            label: "Unused native libraries".to_string(),
            detail: "Unpacked again from the libraries whenever a version launches.".to_string(),
            bytes: measure_dirs(files, &paths.natives(), &natives),
            count: natives.len() as u64,
            tier: Tier::Shared,
            items: natives,
        });
    }

    let (asset_bytes, asset_count) =
        orphans_under(files, &paths.assets_objects(), &live.asset_objects);
    if asset_count > 0 {
        found.push(Reclaimable {
            id: "orphan-assets".to_string(),
            label: "Unused assets".to_string(),
            detail: "Sounds and textures belonging to versions nothing plays any more.".to_string(),
            bytes: asset_bytes,
            count: asset_count,
            tier: Tier::Shared,
            items: Vec::new(),
        });
    }

    if !live.spare_profiles.is_empty() {
        found.push(Reclaimable {
            id: "spare-profiles".to_string(),
            label: "Loader versions nothing is on".to_string(),
            detail: "Kept so far because an instance still plays the game version underneath. Putting an instance back on one of these would download it again.".to_string(),
            bytes: measure_dirs(files, &paths.versions(), &live.spare_profiles),
            count: live.spare_profiles.len() as u64,
            tier: Tier::Spare,
            items: live.spare_profiles,
        });
    }

    Ok((found, None))
}

#[derive(Debug, Clone, Serialize)]
pub struct ReclaimOutcome {
    pub freed_bytes: u64,
    pub cleared: Vec<String>,
    pub failures: Vec<String>,
}

pub const WINDOW_CACHE_CAP: u64 = 256 * 1024 * 1024;

pub fn prune_window_cache(files: &FileManager, cap: u64) -> u64 {
    let path = files.paths().root.join("WebKitCache");
    let bytes = size_of(files, &path, &mut Counted::default());
    if bytes <= cap {
        return 0;
    }
    match files.remove_managed_dir_all_if_exists(&path) {
        Ok(_) => {
            tracing::info!(bytes, cap, "cleared the oversized window cache");
            bytes
        }
        Err(error) => {
            tracing::warn!(error = %error, "could not clear the window cache");
            0
        }
    }
}

fn clear_tree(files: &FileManager, path: &Path) -> Result<u64> {
    let bytes = size_of(files, path, &mut Counted::default());
    files.remove_managed_dir_all_if_exists(path)?;
    Ok(bytes)
}

fn clear_files_in(files: &FileManager, path: &Path) -> Result<u64> {
    let mut freed = 0;
    let Ok(entries) = files.read_external_dir(path) else {
        return Ok(0);
    };
    for entry in entries {
        let Ok(metadata) = files.external_symlink_metadata(&entry) else {
            continue;
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        let size = metadata.len();
        if files.remove_file_if_exists(&entry)? {
            freed += size;
        }
    }
    Ok(freed)
}

fn clear_orphan_files(files: &FileManager, root: &Path, live: &HashSet<PathBuf>) -> Result<u64> {
    let mut pending = vec![root.to_path_buf()];
    let mut freed = 0;

    while let Some(directory) = pending.pop() {
        let Ok(entries) = files.read_external_dir(&directory) else {
            continue;
        };
        for path in entries {
            let Ok(metadata) = files.external_symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() && !live.contains(&path) {
                let size = metadata.len();
                if files.remove_file_if_exists(&path)? {
                    freed += size;
                }
            }
        }
    }

    Ok(freed)
}

pub fn reclaim(store: &Store, targets: &[String]) -> Result<ReclaimOutcome> {
    let files = &store.files;
    let paths = &store.paths;
    let cache_root = paths.cache();

    let wants_graph = targets.iter().any(|target| {
        matches!(
            target.as_str(),
            "orphan-versions" | "orphan-natives" | "orphan-assets" | "spare-profiles"
        )
    });

    let live = if wants_graph {
        let instances = store.db.list_instances(files)?;
        if instances.is_empty() {
            return Err(crate::error::Error::other(
                "Basalt will not call anything unused while there are no instances.",
            ));
        }
        match graph::resolve(files, paths, &instances)? {
            graph::Graph::Unavailable(reason) => {
                return Err(crate::error::Error::other(format!(
                    "Nothing was removed. {reason}"
                )))
            }
            graph::Graph::Resolved(live) => Some(live),
        }
    } else {
        None
    };

    let mut outcome = ReclaimOutcome {
        freed_bytes: 0,
        cleared: Vec::new(),
        failures: Vec::new(),
    };

    for target in targets {
        let freed = match target.as_str() {
            "cache-modpacks" => clear_tree(files, &cache_root.join("modpacks")),
            "cache-installers" => clear_tree(files, &cache_root.join("installers")),
            "cache-runtimes" => clear_tree(files, &cache_root.join("runtimes")),
            "thumbnails" => clear_tree(files, &paths.media().join("thumbnails")),
            "run-logs" => clear_files_in(files, &paths.logs().join("runs")),
            "webkit" => clear_tree(files, &paths.root.join("WebKitCache")),
            "api-cache" => {
                let bytes = store.db.api_cache_bytes().unwrap_or(0);
                store.db.clear_api_cache().map(|_| bytes)
            }
            "orphan-versions" | "spare-profiles" => {
                let live = live.as_ref().expect("graph resolved for this target");
                let names: Vec<String> = if target == "spare-profiles" {
                    live.spare_profiles.clone()
                } else {
                    directory_names(files, &paths.versions())
                        .into_iter()
                        .filter(|name| !live.versions.contains(name))
                        .collect()
                };
                let mut freed = 0;
                let mut failed = None;
                for name in names {
                    match clear_tree(files, &paths.version_dir(&name)) {
                        Ok(bytes) => freed += bytes,
                        Err(error) => failed = Some(error),
                    }
                }
                match failed {
                    Some(error) => Err(error),
                    None => Ok(freed),
                }
            }
            "orphan-natives" => {
                let live = live.as_ref().expect("graph resolved for this target");
                let mut freed = 0;
                for name in directory_names(files, &paths.natives()) {
                    if live.natives.contains(&name) {
                        continue;
                    }
                    freed += clear_tree(files, &paths.natives_dir(&name)).unwrap_or(0);
                }
                Ok(freed)
            }
            "orphan-assets" => clear_orphan_files(
                files,
                &paths.assets_objects(),
                &live
                    .as_ref()
                    .expect("graph resolved for this target")
                    .asset_objects,
            ),
            other => Err(crate::error::Error::other(format!(
                "unknown storage target: {other}"
            ))),
        };

        match freed {
            Ok(bytes) => {
                outcome.freed_bytes += bytes;
                outcome.cleared.push(target.clone());
            }
            Err(error) => {
                tracing::warn!(target = %target, error = %error, "could not reclaim");
                outcome.failures.push(format!("{target}: {error}"));
            }
        }
    }

    tracing::info!(
        freed = outcome.freed_bytes,
        cleared = outcome.cleared.len(),
        "storage reclaimed"
    );
    Ok(outcome)
}

pub fn scan(store: &Store, task: Option<&TaskHandle>) -> Result<StorageReport> {
    let files = &store.files;
    let paths = &store.paths;
    let instances = store.db.list_instances(files)?;
    let mut counted = Counted::default();
    let mut buckets = Vec::new();
    let mut reclaimable = Vec::new();
    let mut partial_bytes = 0;

    if let Some(task) = task {
        task.stage("instances");
    }
    let mut instance_entries = Vec::new();
    let mut instances_total = 0;
    for instance in &instances {
        let (bytes, children) = instance_breakdown(files, instance, &mut counted);
        instances_total += bytes;
        instance_entries.push(Entry {
            id: instance.id.clone(),
            label: instance.name.clone(),
            bytes,
            path: Some(instance.dir.clone()),
            children,
        });
    }
    instance_entries.sort_by_key(|entry| std::cmp::Reverse(entry.bytes));
    buckets.push(Entry {
        id: "instances".to_string(),
        label: "Instances".to_string(),
        bytes: instances_total,
        path: Some(paths.instances().display().to_string()),
        children: instance_entries,
    });

    if let Some(task) = task {
        task.stage("game-files");
    }
    let mut shared = Vec::new();
    let mut shared_total = 0;
    for (id, label, path) in [
        ("assets", "Assets", paths.assets()),
        ("libraries", "Libraries", paths.libraries()),
        ("versions", "Versions", paths.versions()),
        ("natives", "Natives", paths.natives()),
        ("runtimes", "Java runtimes", paths.runtimes()),
    ] {
        let found = walk(files, &path, &mut counted);
        partial_bytes += found.partials.iter().map(|(_, size)| size).sum::<u64>();
        shared_total += found.bytes;
        if found.bytes > 0 {
            shared.push(Entry::leaf(id, label, found.bytes, Some(&path)));
        }
    }
    buckets.push(Entry {
        id: "shared".to_string(),
        label: "Shared game files".to_string(),
        bytes: shared_total,
        path: None,
        children: shared,
    });

    if let Some(task) = task {
        task.stage("caches");
    }
    let cache_root = paths.cache();
    let mut caches = Vec::new();
    let mut caches_total = 0;
    for (id, label, path, offer, detail) in [
        (
            "cache-modpacks",
            "Modpack archives",
            cache_root.join("modpacks"),
            true,
            "Downloaded pack files, kept only to save fetching them twice.",
        ),
        (
            "cache-installers",
            "Loader installers",
            cache_root.join("installers"),
            true,
            "Forge and NeoForge installer jars, fetched again when needed.",
        ),
        (
            "cache-runtimes",
            "Java archives",
            cache_root.join("runtimes"),
            true,
            "Downloaded Java archives left behind after an install.",
        ),
        (
            "thumbnails",
            "Screenshot previews",
            paths.media().join("thumbnails"),
            true,
            "Rebuilt as soon as you open a screenshots tab again.",
        ),
        (
            "webkit",
            "Window cache",
            paths.root.join("WebKitCache"),
            true,
            "Icons and images the window has loaded. Cleared automatically once it passes 256 MB.",
        ),
    ] {
        let bytes = size_of(files, &path, &mut counted);
        caches_total += bytes;
        if bytes == 0 {
            continue;
        }
        caches.push(Entry::leaf(id, label, bytes, Some(&path)));
        if offer {
            reclaimable.push(Reclaimable {
                id: id.to_string(),
                label: label.to_string(),
                detail: detail.to_string(),
                bytes,
                count: 0,
                tier: Tier::Cache,
                items: Vec::new(),
            });
        }
    }

    let api_bytes = store.db.api_cache_bytes().unwrap_or(0);
    if api_bytes > 0 {
        reclaimable.push(Reclaimable {
            id: "api-cache".to_string(),
            label: "Stored search results".to_string(),
            detail: "Replies from Modrinth and CurseForge. These are also what Basalt falls back on when it cannot reach either site.".to_string(),
            bytes: api_bytes,
            count: 0,
            tier: Tier::Cache,
            items: Vec::new(),
        });
    }
    buckets.push(Entry {
        id: "caches".to_string(),
        label: "Caches".to_string(),
        bytes: caches_total,
        path: Some(cache_root.display().to_string()),
        children: caches,
    });

    if let Some(task) = task {
        task.stage("snapshots");
    }
    buckets.push(Entry::leaf(
        "snapshots",
        "Snapshots",
        size_of(files, &paths.snapshots(), &mut counted),
        Some(&paths.snapshots()),
    ));

    let run_logs = size_of(files, &paths.logs().join("runs"), &mut Counted::default());
    buckets.push(Entry::leaf(
        "logs",
        "Logs",
        size_of(files, &paths.logs(), &mut counted),
        Some(&paths.logs()),
    ));
    if run_logs > 0 {
        reclaimable.push(Reclaimable {
            id: "run-logs".to_string(),
            label: "Game output logs".to_string(),
            detail: "One file per launch, kept until you clear them.".to_string(),
            bytes: run_logs,
            count: 0,
            tier: Tier::Cache,
            items: Vec::new(),
        });
    }

    buckets.push(Entry::leaf(
        "media",
        "Banners and skins",
        size_of(files, &paths.media(), &mut counted),
        Some(&paths.media()),
    ));

    let database: u64 = ["basalt.db", "basalt.db-wal", "basalt.db-shm"]
        .iter()
        .map(|name| file_size(files, &paths.root.join(name)))
        .sum();
    buckets.push(Entry::leaf("database", "Database", database, None));

    if let Some(task) = task {
        task.stage("unreferenced");
    }
    let (unused, unresolved) = unreferenced(files, paths, &instances)?;
    reclaimable.extend(unused);

    if partial_bytes > 0 {
        buckets.push(Entry::leaf(
            "partials",
            "Unfinished downloads",
            partial_bytes,
            None,
        ));
    }

    buckets.retain(|entry| entry.bytes > 0);
    buckets.sort_by_key(|entry| std::cmp::Reverse(entry.bytes));
    reclaimable.sort_by_key(|entry| std::cmp::Reverse(entry.bytes));

    let usage = crate::sysinfo_probe::usage(paths);
    Ok(StorageReport {
        scanned_at: chrono::Utc::now().timestamp_millis(),
        root: paths.root.display().to_string(),
        total_bytes: buckets.iter().map(|entry| entry.bytes).sum(),
        free_bytes: usage.data_dir_free_mb.map(|mb| mb * 1024 * 1024),
        disk_total_bytes: usage.data_dir_total_mb.map(|mb| mb * 1024 * 1024),
        buckets,
        reclaimable,
        unresolved,
        shared_dedupe: cfg!(unix),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn report_on_the_real_data_directory() {
        let root = dirs_root();
        let paths = Paths::plain(root);
        let files = FileManager::new(paths.clone()).unwrap();
        let db = Db::open(&files).unwrap();
        let store = Store { files, paths, db };

        let report = scan(&store, None).unwrap();
        println!("total {} MB", report.total_bytes / 1024 / 1024);
        for bucket in &report.buckets {
            println!(
                "  {:<24} {:>8} MB",
                bucket.label,
                bucket.bytes / 1024 / 1024
            );
            for child in bucket.children.iter().take(4) {
                println!(
                    "      {:<20} {:>8} MB",
                    child.label,
                    child.bytes / 1024 / 1024
                );
            }
        }
        println!("unresolved: {:?}", report.unresolved);
        for entry in &report.reclaimable {
            println!(
                "  RECLAIM {:<28} {:>7} MB  {:?} {:?}",
                entry.label,
                entry.bytes / 1024 / 1024,
                entry.tier,
                entry.items
            );
        }
    }

    fn test_store() -> Store {
        let root =
            std::env::temp_dir().join(format!("basalt-reclaim-test-{}", uuid::Uuid::new_v4()));
        let paths = Paths::plain(root);
        let files = FileManager::new(paths.clone()).unwrap();
        files.ensure_base_dirs().unwrap();
        let db = Db::open(&files).unwrap();
        Store { files, paths, db }
    }

    fn test_instance(paths: &Paths, version: &str, loader: Option<(&str, &str)>) -> Instance {
        let id = uuid::Uuid::new_v4().to_string();
        Instance {
            name: "Test".to_string(),
            version_id: version.to_string(),
            created_at: chrono::Utc::now(),
            min_memory_mb: None,
            max_memory_mb: None,
            java_path: None,
            last_played_at: None,
            playtime_secs: 0,
            dir: paths.instance_dir(&id).display().to_string(),
            logo: None,
            loader: loader.map(|(name, _)| name.to_string()),
            loader_version: loader.map(|(_, version)| version.to_string()),
            launch_version_id: None,
            pack_provider: None,
            pack_project_id: None,
            pack_version_id: None,
            jvm_args: None,
            jvm_args_mode: None,
            env_vars: None,
            env_vars_mode: None,
            import_source: None,
            import_source_id: None,
            banner_id: None,
            notes: None,
            wrapper_command: None,
            pre_launch_command: None,
            post_exit_command: None,
            id,
        }
    }

    fn write_version(paths: &Paths, id: &str, inherits: Option<&str>) {
        std::fs::create_dir_all(paths.version_dir(id)).unwrap();
        let body = match inherits {
            Some(parent) => format!(
                r#"{{"id":"{id}","mainClass":"net.Main","inheritsFrom":"{parent}","libraries":[]}}"#
            ),
            None => format!(r#"{{"id":"{id}","mainClass":"net.Main","libraries":[]}}"#),
        };
        std::fs::write(paths.version_json(id), body).unwrap();
        std::fs::write(paths.version_jar(id), vec![7u8; 4096]).unwrap();
    }

    #[test]
    fn clearing_unused_versions_leaves_the_ones_an_instance_needs() {
        let store = test_store();
        write_version(&store.paths, "1.21.11", None);
        write_version(
            &store.paths,
            "fabric-loader-0.18.4-1.21.11",
            Some("1.21.11"),
        );
        write_version(&store.paths, "26.1.2", None);

        let instance = test_instance(&store.paths, "1.21.11", Some(("fabric", "0.18.4")));
        store.db.insert_instance(&instance).unwrap();

        let outcome = reclaim(&store, &["orphan-versions".to_string()]).unwrap();

        assert!(outcome.failures.is_empty());
        assert!(outcome.freed_bytes > 0);
        assert!(store.paths.version_dir("1.21.11").exists());
        assert!(store
            .paths
            .version_dir("fabric-loader-0.18.4-1.21.11")
            .exists());
        assert!(!store.paths.version_dir("26.1.2").exists());
    }

    #[test]
    fn nothing_is_cleared_while_the_reference_graph_is_broken() {
        let store = test_store();
        write_version(&store.paths, "1.21.11", None);
        write_version(&store.paths, "26.1.2", None);
        std::fs::write(store.paths.version_json("1.21.11"), b"{ truncated").unwrap();

        let instance = test_instance(&store.paths, "1.21.11", None);
        store.db.insert_instance(&instance).unwrap();

        let refused = reclaim(&store, &["orphan-versions".to_string()]);

        assert!(refused.is_err());
        assert!(store.paths.version_dir("26.1.2").exists());
    }

    #[test]
    fn nothing_is_cleared_when_there_are_no_instances_to_compare_against() {
        let store = test_store();
        write_version(&store.paths, "1.21.11", None);

        let refused = reclaim(&store, &["orphan-versions".to_string()]);

        assert!(refused.is_err());
        assert!(store.paths.version_dir("1.21.11").exists());
    }

    fn dirs_root() -> PathBuf {
        PathBuf::from(std::env::var("HOME").unwrap())
            .join(".local/share/com.megalithofficial.basalt-launcher")
    }
}
