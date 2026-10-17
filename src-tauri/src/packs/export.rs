use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::{
    config::Instance,
    db::ContentFile,
    error::{Error, Result},
    files::FileManager,
    state::AppState,
};

use super::{is_content_path, loader_dependency_key, PackFormat, CONTENT_DIRS};

/**
 * Top-level entries that start checked: the mods and the scripts a pack cannot
 * regenerate on first launch. Configs, packs and personal files are opt-in.
 */
const DEFAULT_SELECTED: [&str; 3] = ["mods", "kubejs", "scripts"];

/** Launcher state and caches that never belong in a pack. */
const NEVER_EXPORTED: [&str; 8] = [
    ".basalt",
    ".fabric",
    ".mixin.out",
    "logs",
    "crash-reports",
    "downloads",
    "versions",
    "__MACOSX",
];

#[derive(Debug, Clone, Serialize)]
pub struct PackExport {
    pub path: String,
    pub format: PackFormat,
    pub linked: usize,
    pub bundled: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportCandidate {
    pub path: String,
    pub directory: bool,
    pub size: u64,
    pub default_selected: bool,
    /** Linked files inside a content folder, or the link of a single file. */
    pub modrinth: usize,
    pub curseforge: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExportOptions {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub included: Vec<String>,
    #[serde(default)]
    pub excluded: Vec<String>,
}

fn first_segment(relative: &str) -> &str {
    relative.split('/').next().unwrap_or(relative)
}

fn is_exportable(relative: &str) -> bool {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    !NEVER_EXPORTED.contains(&first_segment(relative)) && name != ".DS_Store"
}

#[derive(Default)]
struct SelectionNode {
    selected: Option<bool>,
    has_included_rule: bool,
    children: BTreeMap<String, SelectionNode>,
}

/**
 * Include and exclude rules keyed by path. A path takes the nearest rule on
 * its own chain, unruled paths stay out. Directories are only walked when
 * they are selected or hold an include rule somewhere below.
 */
#[derive(Default)]
pub struct ExportSelection {
    root: SelectionNode,
}

impl ExportSelection {
    pub fn new(included: &[String], excluded: &[String]) -> Self {
        let mut selection = Self::default();
        for (paths, selected) in [(included, true), (excluded, false)] {
            for path in paths {
                let path = path.trim_matches('/');
                if path.is_empty() || path.split('/').any(|part| part == "..") {
                    continue;
                }
                selection.insert(path, selected);
            }
        }
        selection
    }

    fn insert(&mut self, path: &str, selected: bool) {
        let mut node = &mut self.root;
        for segment in path.split('/') {
            node = node.children.entry(segment.to_string()).or_default();
            if selected {
                node.has_included_rule = true;
            }
        }
        node.selected = Some(selected);
    }

    fn resolve(&self, path: &str) -> (bool, Option<&SelectionNode>) {
        let mut node = &self.root;
        let mut selected = false;
        for segment in path.split('/') {
            let Some(child) = node.children.get(segment) else {
                return (selected, None);
            };
            node = child;
            selected = node.selected.unwrap_or(selected);
        }
        (selected, Some(node))
    }

    pub fn includes(&self, path: &str) -> bool {
        self.resolve(path).0
    }

    pub fn visits(&self, path: &str) -> bool {
        let (selected, node) = self.resolve(path);
        selected || node.is_some_and(|node| node.has_included_rule)
    }
}

fn linked_counts(
    sources: &[(String, ContentFile)],
    relative: &str,
    directory: bool,
) -> (usize, usize) {
    let mut modrinth = 0;
    let mut curseforge = 0;
    for (kind, source) in sources {
        let matches = if directory {
            kind == relative
        } else {
            relative == format!("{kind}/{}", source.file_name)
        };
        if !matches {
            continue;
        }
        match source.provider.as_deref() {
            Some("modrinth") => modrinth += 1,
            Some("curseforge") => curseforge += 1,
            _ => {}
        }
    }
    (modrinth, curseforge)
}

pub fn export_candidates(
    files: &FileManager,
    instance: &Instance,
    sources: &[(String, ContentFile)],
    parent: Option<&str>,
) -> Result<Vec<ExportCandidate>> {
    let root = PathBuf::from(&instance.dir);
    let parent = parent
        .map(|p| p.trim_matches('/'))
        .filter(|p| !p.is_empty());
    if parent.is_some_and(|p| p.split('/').any(|part| part == "..") || !is_exportable(p)) {
        return Ok(Vec::new());
    }
    let directory = parent.map_or(root.clone(), |p| root.join(p));
    let mut found = Vec::new();
    for path in files.read_dir(&directory).unwrap_or_default() {
        let Some(relative) = relative_string(&root, &path) else {
            continue;
        };
        if !is_exportable(&relative) {
            continue;
        }
        let Ok(metadata) = files.symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() || !(metadata.is_dir() || metadata.is_file()) {
            continue;
        }
        let default_selected = DEFAULT_SELECTED.contains(&first_segment(&relative));
        let (modrinth, curseforge) = linked_counts(sources, &relative, metadata.is_dir());
        found.push(ExportCandidate {
            path: relative,
            directory: metadata.is_dir(),
            size: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
            default_selected,
            modrinth,
            curseforge,
        });
    }
    found.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase()))
    });
    Ok(found)
}

#[derive(Serialize)]
struct MrIndex {
    #[serde(rename = "formatVersion")]
    format_version: u8,
    game: &'static str,
    #[serde(rename = "versionId")]
    version_id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
    dependencies: BTreeMap<String, String>,
    files: Vec<MrFile>,
}

#[derive(Serialize)]
struct MrFile {
    path: String,
    hashes: MrHashes,
    downloads: Vec<String>,
    #[serde(rename = "fileSize")]
    file_size: u64,
}

#[derive(Serialize)]
struct MrHashes {
    sha1: String,
    sha512: String,
}

#[derive(Serialize)]
struct CfManifest {
    minecraft: CfMinecraft,
    #[serde(rename = "manifestType")]
    manifest_type: &'static str,
    #[serde(rename = "manifestVersion")]
    manifest_version: u8,
    name: String,
    version: String,
    author: String,
    files: Vec<CfFile>,
    overrides: &'static str,
}

#[derive(Serialize)]
struct CfMinecraft {
    version: String,
    #[serde(rename = "modLoaders")]
    mod_loaders: Vec<CfLoader>,
}

#[derive(Serialize)]
struct CfLoader {
    id: String,
    primary: bool,
}

#[derive(Serialize)]
struct CfFile {
    #[serde(rename = "projectID")]
    project_id: i64,
    #[serde(rename = "fileID")]
    file_id: i64,
    required: bool,
}

pub async fn export_instance(
    state: &AppState,
    instance: &Instance,
    format: PackFormat,
    destination: PathBuf,
    options: ExportOptions,
) -> Result<PackExport> {
    let files = state.files.clone();
    let root = PathBuf::from(&instance.dir);

    let mut sources = Vec::new();
    for kind in CONTENT_DIRS {
        sources.extend(state.db.content_files(&instance.id, kind)?);
    }

    let instance = instance.clone();
    let stamp = chrono::Utc::now().format("%Y.%m.%d").to_string();
    tokio::task::spawn_blocking(move || {
        write_pack(
            &files,
            &instance,
            &root,
            &sources,
            format,
            &destination,
            &stamp,
            &options,
        )
    })
    .await
    .map_err(|error| Error::other(format!("export task failed: {error}")))?
}

fn cleaned(value: Option<&String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[allow(clippy::too_many_arguments)]
fn write_pack(
    files: &FileManager,
    instance: &Instance,
    root: &Path,
    sources: &[ContentFile],
    format: PackFormat,
    destination: &Path,
    stamp: &str,
    options: &ExportOptions,
) -> Result<PackExport> {
    let selection = ExportSelection::new(&options.included, &options.excluded);
    let name = cleaned(options.name.as_ref()).unwrap_or_else(|| instance.name.clone());
    let version = cleaned(options.version.as_ref()).unwrap_or_else(|| stamp.to_string());
    let summary = cleaned(options.description.as_ref());
    let provider = match format {
        PackFormat::Mrpack => "modrinth",
        PackFormat::Curseforge => "curseforge",
    };

    let mut index_files: Vec<MrFile> = Vec::new();
    let mut manifest_files: Vec<CfFile> = Vec::new();
    let mut bundled: Vec<(String, PathBuf)> = Vec::new();

    for path in walk(files, root, &selection) {
        let Some(relative) = relative_string(root, &path) else {
            continue;
        };
        if !is_exportable(&relative) || !selection.includes(&relative) {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if file_name.is_empty() {
            continue;
        }

        let linked = if is_content_path(Path::new(&relative)) && !file_name.ends_with(".disabled") {
            sources.iter().find(|source| {
                source.file_name == file_name && source.provider.as_deref() == Some(provider)
            })
        } else {
            None
        };

        let declared = match (format, linked) {
            (PackFormat::Mrpack, Some(source)) => mr_file(source, &relative, &path, files)
                .map(|file| index_files.push(file))
                .is_some(),
            (PackFormat::Curseforge, Some(source)) => cf_file(source)
                .map(|file| manifest_files.push(file))
                .is_some(),
            _ => false,
        };

        if !declared {
            bundled.push((relative, path));
        }
    }

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let target = std::fs::File::create(destination)?;
    let mut zip = ZipWriter::new(target);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let linked = index_files.len() + manifest_files.len();
    match format {
        PackFormat::Mrpack => {
            let mut dependencies = BTreeMap::new();
            dependencies.insert("minecraft".to_string(), instance.version_id.clone());
            if let (Some(key), Some(version)) = (
                instance.loader.as_deref().and_then(loader_dependency_key),
                instance.loader_version.clone(),
            ) {
                dependencies.insert(key.to_string(), version);
            }

            index_files.sort_by(|a, b| a.path.cmp(&b.path));
            let index = MrIndex {
                format_version: 1,
                game: "minecraft",
                version_id: version.clone(),
                name: name.clone(),
                summary: summary.clone(),
                dependencies,
                files: index_files,
            };
            write_entry(
                &mut zip,
                options,
                "modrinth.index.json",
                serde_json::to_vec_pretty(&index)?.as_slice(),
            )?;
        }
        PackFormat::Curseforge => {
            let loaders = instance
                .loader
                .as_deref()
                .zip(instance.loader_version.as_deref())
                .map(|(loader, version)| CfLoader {
                    id: format!("{loader}-{version}"),
                    primary: true,
                })
                .into_iter()
                .collect();
            manifest_files.sort_by_key(|file| (file.project_id, file.file_id));
            let manifest = CfManifest {
                minecraft: CfMinecraft {
                    version: instance.version_id.clone(),
                    mod_loaders: loaders,
                },
                manifest_type: "minecraftModpack",
                manifest_version: 1,
                name: name.clone(),
                version: version.clone(),
                author: String::new(),
                files: manifest_files,
                overrides: "overrides",
            };
            write_entry(
                &mut zip,
                options,
                "manifest.json",
                serde_json::to_vec_pretty(&manifest)?.as_slice(),
            )?;
        }
    }

    bundled.sort_by(|a, b| a.0.cmp(&b.0));
    let mut bytes = 0u64;
    for (relative, path) in &bundled {
        let Ok(mut source) = files.open(path) else {
            continue;
        };
        zip.start_file(format!("overrides/{relative}"), options)
            .map_err(|error| Error::other(format!("writing the pack: {error}")))?;
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let read = source.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            zip.write_all(&buffer[..read])?;
            bytes += read as u64;
        }
    }

    zip.finish()
        .map_err(|error| Error::other(format!("finishing the pack: {error}")))?;

    Ok(PackExport {
        path: destination.display().to_string(),
        format,
        linked,
        bundled: bundled.len(),
        bytes,
    })
}

fn write_entry(
    zip: &mut ZipWriter<std::fs::File>,
    options: SimpleFileOptions,
    name: &str,
    body: &[u8],
) -> Result<()> {
    zip.start_file(name, options)
        .map_err(|error| Error::other(format!("writing the pack: {error}")))?;
    zip.write_all(body)?;
    Ok(())
}

fn mr_file(
    source: &ContentFile,
    relative: &str,
    path: &Path,
    files: &FileManager,
) -> Option<MrFile> {
    let project_id = source.project_id.as_deref()?;
    let version_id = source.version_id.as_deref()?;
    let sha1 = source.sha1.clone()?;
    let sha512 = match source.sha512.clone() {
        Some(hash) => hash,
        None => sha512_of(files, path)?,
    };
    let size = files.metadata(path).ok()?.len();

    Some(MrFile {
        path: relative.to_string(),
        hashes: MrHashes { sha1, sha512 },
        downloads: vec![format!(
            "https://cdn.modrinth.com/data/{project_id}/versions/{version_id}/{}",
            encode_segment(&source.file_name)
        )],
        file_size: size,
    })
}

/** Jars that arrived through an import only carry sha1, the index wants both. */
fn sha512_of(files: &FileManager, path: &Path) -> Option<String> {
    use sha2::{Digest, Sha512};
    let mut source = files.open(path).ok()?;
    let mut hasher = Sha512::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = source.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn cf_file(source: &ContentFile) -> Option<CfFile> {
    Some(CfFile {
        project_id: source.project_id.as_deref()?.parse().ok()?,
        file_id: source.version_id.as_deref()?.parse().ok()?,
        required: true,
    })
}

fn encode_segment(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for byte in name.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn walk(files: &FileManager, root: &Path, selection: &ExportSelection) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut found = Vec::new();
    while let Some(directory) = pending.pop() {
        let Ok(entries) = files.read_dir(&directory) else {
            continue;
        };
        for path in entries {
            let Some(relative) = relative_string(root, &path) else {
                continue;
            };
            if !is_exportable(&relative) {
                continue;
            }
            let Ok(metadata) = files.symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                if selection.visits(&relative) {
                    pending.push(path);
                }
            } else if metadata.is_file() {
                found.push(path);
            }
        }
    }
    found
}

fn relative_string(root: &Path, path: &Path) -> Option<String> {
    Some(path.strip_prefix(root).ok()?.to_str()?.replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, io::Read, path::PathBuf};

    use super::{
        encode_segment, export_candidates, is_exportable, write_pack, ExportOptions,
        ExportSelection,
    };
    use crate::{config::Instance, files::FileManager, packs::PackFormat, paths::Paths};

    fn fake_instance() -> (PathBuf, FileManager, Instance) {
        let base = std::env::temp_dir().join(format!("basalt-export-{}", uuid::Uuid::new_v4()));
        let dir = base.join("data").join("instances").join("main");
        for file in [
            "mods/sodium.jar",
            "config/sodium.json",
            "config/private/token.txt",
            "options.txt",
            "servers.dat",
            "saves/world/level.dat",
            "logs/latest.log",
            ".basalt/state.json",
            "shaderpacks/bsl.zip",
        ] {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, file).unwrap();
        }
        let files = FileManager::new(Paths::plain(base.join("data"))).unwrap();
        let instance = Instance {
            id: "main".into(),
            name: "Main Ding".into(),
            version_id: "1.21.1".into(),
            created_at: chrono::Utc::now(),
            min_memory_mb: None,
            max_memory_mb: None,
            java_path: None,
            last_played_at: None,
            playtime_secs: 0,
            dir: dir.to_string_lossy().to_string(),
            logo: None,
            loader: Some("neoforge".into()),
            loader_version: Some("21.1.248".into()),
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
        };
        (base, files, instance)
    }

    fn entries(path: &std::path::Path) -> (BTreeSet<String>, String) {
        let file = std::fs::File::open(path).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        let names: BTreeSet<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        let mut index = String::new();
        zip.by_name("modrinth.index.json")
            .unwrap()
            .read_to_string(&mut index)
            .unwrap();
        (names, index)
    }

    #[test]
    fn candidates_list_the_instance_root_with_defaults_and_hide_launcher_state() {
        let (base, files, instance) = fake_instance();
        let sources = vec![(
            "mods".to_string(),
            crate::db::ContentFile {
                file_name: "sodium.jar".into(),
                provider: Some("modrinth".into()),
                ..Default::default()
            },
        )];
        let root = export_candidates(&files, &instance, &sources, None).unwrap();
        let by_path: std::collections::HashMap<_, _> =
            root.iter().map(|c| (c.path.as_str(), c)).collect();
        assert!(by_path["mods"].default_selected && by_path["mods"].directory);
        assert_eq!(by_path["mods"].modrinth, 1);
        assert!(!by_path["config"].default_selected);
        assert!(!by_path["options.txt"].default_selected);
        assert!(!by_path["saves"].default_selected);
        assert!(!by_path.contains_key("logs"));
        assert!(!by_path.contains_key(".basalt"));
        let nested = export_candidates(&files, &instance, &sources, Some("config")).unwrap();
        assert_eq!(
            nested.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(),
            ["config/private", "config/sodium.json"]
        );
        let mods = export_candidates(&files, &instance, &sources, Some("mods")).unwrap();
        assert_eq!(mods[0].modrinth, 1);
        assert!(export_candidates(&files, &instance, &sources, Some("../"))
            .unwrap()
            .is_empty());
        drop(files);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn only_selected_paths_land_in_the_pack_and_metadata_is_written() {
        let (base, files, instance) = fake_instance();
        let destination = base.join("out.mrpack");
        let options = ExportOptions {
            name: Some("World Basic".into()),
            version: Some("1.0.2".into()),
            description: Some("  friends pack ".into()),
            included: vec!["mods".into(), "config".into(), "shaderpacks".into()],
            excluded: vec!["config/private".into()],
        };
        let report = write_pack(
            &files,
            &instance,
            &PathBuf::from(&instance.dir),
            &[],
            PackFormat::Mrpack,
            &destination,
            "2026.09.17",
            &options,
        )
        .unwrap();
        let (names, index) = entries(&destination);
        assert_eq!(
            names.iter().map(String::as_str).collect::<Vec<_>>(),
            [
                "modrinth.index.json",
                "overrides/config/sodium.json",
                "overrides/mods/sodium.jar",
                "overrides/shaderpacks/bsl.zip",
            ]
        );
        assert_eq!(report.bundled, 3);
        assert!(index.contains("\"name\": \"World Basic\""));
        assert!(index.contains("\"versionId\": \"1.0.2\""));
        assert!(index.contains("\"summary\": \"friends pack\""));
        assert!(index.contains("\"neoforge\": \"21.1.248\""));
        drop(files);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_linked_mod_without_a_stored_sha512_is_still_listed_by_link() {
        let (base, files, instance) = fake_instance();
        let destination = base.join("linked.mrpack");
        let source = crate::db::ContentFile {
            file_name: "sodium.jar".into(),
            sha1: Some("abc".into()),
            provider: Some("modrinth".into()),
            project_id: Some("AANobbMI".into()),
            version_id: Some("v1".into()),
            ..Default::default()
        };
        let report = write_pack(
            &files,
            &instance,
            &PathBuf::from(&instance.dir),
            &[source],
            PackFormat::Mrpack,
            &destination,
            "2026.09.17",
            &ExportOptions {
                included: vec!["mods".into()],
                ..Default::default()
            },
        )
        .unwrap();
        let (names, index) = entries(&destination);
        assert_eq!(report.linked, 1);
        assert_eq!(report.bundled, 0);
        assert!(!names.contains("overrides/mods/sodium.jar"));
        assert!(index.contains("cdn.modrinth.com/data/AANobbMI/versions/v1/sodium.jar"));
        assert!(index.contains("\"sha512\": \""));
        drop(files);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn nothing_selected_means_an_index_only_pack_and_no_summary_field() {
        let (base, files, instance) = fake_instance();
        let destination = base.join("empty.mrpack");
        write_pack(
            &files,
            &instance,
            &PathBuf::from(&instance.dir),
            &[],
            PackFormat::Mrpack,
            &destination,
            "2026.09.17",
            &ExportOptions::default(),
        )
        .unwrap();
        let (names, index) = entries(&destination);
        assert_eq!(names.len(), 1);
        assert!(index.contains("\"name\": \"Main Ding\""));
        assert!(index.contains("\"versionId\": \"2026.09.17\""));
        assert!(!index.contains("summary"));
        drop(files);
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn encodes_url_unsafe_characters() {
        assert_eq!(encode_segment("sodium-0.6.jar"), "sodium-0.6.jar");
        assert_eq!(encode_segment("my mod+1.jar"), "my%20mod%2B1.jar");
    }

    #[test]
    fn a_path_takes_the_nearest_rule_and_unruled_paths_stay_out() {
        let selection = ExportSelection::new(
            &["config".into(), "saves/creative".into()],
            &["config/secret.toml".into()],
        );
        assert!(selection.includes("config/sodium.json"));
        assert!(selection.includes("config/deep/nested.cfg"));
        assert!(!selection.includes("config/secret.toml"));
        assert!(!selection.includes("options.txt"));
        assert!(!selection.includes("saves/survival/level.dat"));
        assert!(selection.includes("saves/creative/level.dat"));
    }

    #[test]
    fn directories_are_walked_only_when_something_inside_is_wanted() {
        let selection = ExportSelection::new(&["saves/creative".into()], &[]);
        assert!(selection.visits("saves"));
        assert!(selection.visits("saves/creative"));
        assert!(!selection.visits("saves/survival"));
        assert!(!selection.visits("logs"));
    }

    #[test]
    fn launcher_state_and_junk_never_export() {
        assert!(!is_exportable(".basalt/state.json"));
        assert!(!is_exportable("logs/latest.log"));
        assert!(!is_exportable("config/.DS_Store"));
        assert!(is_exportable("config/sodium.json"));
        assert!(is_exportable("saves/world/level.dat"));
    }
}
