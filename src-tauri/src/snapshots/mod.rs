use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::{
    config::Instance,
    db::{ContentFile, Db},
    error::{Error, Result},
    files::FileManager,
    instance_ops::instance_busy,
    paths::Paths,
    state::AppState,
    tasks::{TaskHandle, TaskKind, TaskSpec},
};

mod restore;
mod workers;

pub(crate) use restore::{recover_interrupted, restore};
use workers::{parallel_map, Progress};

const SNAPSHOT_SCHEMA: u32 = 2;
const AUTOMATIC_RETENTION: usize = 3;
const COMPRESSION_LEVEL: i32 = 3;
const BUFFER_SIZE: usize = 1024 * 1024;
const EXCLUDED_TOP_LEVEL: &[&str] = &["logs", "crash-reports", "screenshots", "backups"];
static STORE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone)]
struct SnapshotStore {
    files: FileManager,
    paths: Paths,
    db: Db,
}

impl SnapshotStore {
    fn from_state(state: &AppState) -> Self {
        Self {
            files: state.files.clone(),
            paths: state.paths.clone(),
            db: state.db.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotKind {
    Manual,
    Automatic,
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotSummary {
    pub id: String,
    pub name: String,
    pub kind: SnapshotKind,
    pub created_at: i64,
    pub file_count: u64,
    pub size_bytes: u64,
    pub stored_size_bytes: u64,
    pub new_size_bytes: Option<u64>,
    #[serde(default)]
    pub excluded: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotFile {
    path: PathBuf,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotManifest {
    schema_version: u32,
    id: String,
    name: String,
    kind: SnapshotKind,
    created_at: i64,
    file_count: u64,
    size_bytes: u64,
    stored_size_bytes: u64,
    #[serde(default)]
    new_size_bytes: Option<u64>,
    #[serde(default)]
    excluded: Vec<String>,
    directories: Vec<PathBuf>,
    files: Vec<SnapshotFile>,
    instance: Instance,
    content: Vec<(String, ContentFile)>,
}

impl SnapshotManifest {
    fn summary(&self) -> SnapshotSummary {
        SnapshotSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind,
            created_at: self.created_at,
            file_count: self.file_count,
            size_bytes: self.size_bytes,
            stored_size_bytes: self.stored_size_bytes,
            new_size_bytes: self.new_size_bytes,
            excluded: self.excluded.clone(),
        }
    }
}

struct StoredBlob {
    compressed_size: u64,
    created: bool,
}

struct BlobSource {
    sha256: String,
    source: PathBuf,
    size: u64,
}

#[derive(Debug)]
enum SourceEntry {
    Directory(PathBuf),
    File { relative: PathBuf, size: u64 },
}

fn progress_for_entries(entries: &[SourceEntry], task: Option<&TaskHandle>) -> Progress {
    let total_files = entries
        .iter()
        .filter(|entry| matches!(entry, SourceEntry::File { .. }))
        .count() as u64;
    let total_bytes = entries
        .iter()
        .map(|entry| match entry {
            SourceEntry::Directory(_) => 0,
            SourceEntry::File { size, .. } => *size,
        })
        .sum();
    Progress::new(total_files, total_bytes, task)
}

fn store_guard() -> Result<MutexGuard<'static, ()>> {
    STORE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| Error::other("snapshot store lock was poisoned"))
}

fn check_cancelled(task: Option<&TaskHandle>) -> Result<()> {
    if task.is_some_and(|task| task.token().is_cancelled()) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::other("Snapshot name cannot be empty."));
    }
    if name.chars().count() > 80 {
        return Err(Error::other(
            "Snapshot names cannot be longer than 80 characters.",
        ));
    }
    Ok(name.to_string())
}

pub(crate) fn ensure_no_pending_restore(state: &AppState, instance_id: &str) -> Result<()> {
    let journal = state
        .paths
        .snapshot_restore_journal_checked(instance_id)
        .ok_or_else(|| Error::other("invalid instance id"))?;
    if state.files.exists(journal)? {
        Err(Error::other(
            "This instance has an interrupted restore that must be recovered first.",
        ))
    } else {
        Ok(())
    }
}

fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn collect_entries(
    files: &FileManager,
    root: &Path,
    current: &Path,
    output: &mut Vec<SourceEntry>,
    exclude_volatile: bool,
) -> Result<()> {
    collect_entries_excluding(files, root, current, output, exclude_volatile, &[])
}

fn collect_entries_excluding(
    files: &FileManager,
    root: &Path,
    current: &Path,
    output: &mut Vec<SourceEntry>,
    exclude_volatile: bool,
    excluded: &[String],
) -> Result<()> {
    for path in files.read_dir(current)? {
        let metadata = files.symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::other(format!(
                "Cannot snapshot a symbolic link: {}",
                path.display()
            )));
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| Error::other("snapshot path escaped its source"))?
            .to_path_buf();
        if relative.components().count() == 1 {
            if let Some(name) = relative.file_name().and_then(|name| name.to_str()) {
                if exclude_volatile && EXCLUDED_TOP_LEVEL.contains(&name) {
                    continue;
                }
                if excluded.iter().any(|value| value == name) {
                    continue;
                }
            }
        }
        if metadata.is_dir() {
            output.push(SourceEntry::Directory(relative));
            collect_entries_excluding(files, root, &path, output, exclude_volatile, excluded)?;
        } else if metadata.is_file() {
            output.push(SourceEntry::File {
                relative,
                size: metadata.len(),
            });
        }
    }
    Ok(())
}

fn hex_digest(digest: &[u8]) -> String {
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn hash_file(
    files: &FileManager,
    path: &Path,
    expected_size: u64,
    progress: &Progress,
    task: Option<&TaskHandle>,
) -> Result<String> {
    let mut reader = files.open(path)?;
    let mut hasher = Sha256::new();
    let mut read_size = 0_u64;
    let mut buffer = [0_u8; BUFFER_SIZE];
    loop {
        check_cancelled(task)?;
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        read_size = read_size.saturating_add(read as u64);
        progress.bytes(read, task);
    }
    if read_size != expected_size {
        return Err(Error::other(format!(
            "File changed while snapshotting: {}",
            path.display()
        )));
    }
    Ok(hex_digest(&hasher.finalize()))
}

fn compress_blob(
    state: &SnapshotStore,
    source: &Path,
    hash: &str,
    expected_size: u64,
    progress: Option<&Progress>,
    task: Option<&TaskHandle>,
) -> Result<StoredBlob> {
    let destination = state
        .paths
        .snapshot_blob(hash)
        .ok_or_else(|| Error::other("invalid snapshot blob hash"))?;
    if state.files.exists(&destination)? {
        let metadata = state.files.symlink_metadata(&destination)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(Error::other(format!(
                "Snapshot blob path is not a regular file: {}",
                destination.display()
            )));
        }
        if let Some(progress) = progress {
            progress.bytes_u64(expected_size, task);
            progress.file(task);
        }
        return Ok(StoredBlob {
            compressed_size: metadata.len(),
            created: false,
        });
    }
    let parent = destination
        .parent()
        .ok_or_else(|| Error::other("snapshot blob has no parent"))?;
    state.files.ensure_dir(parent)?;
    let temporary = parent.join(format!(".{}.zst.part", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut reader = state.files.open(source)?;
        let writer = state.files.create(&temporary)?;
        let mut encoder = zstd::stream::write::Encoder::new(writer, COMPRESSION_LEVEL)?;
        encoder.include_checksum(true)?;
        let mut hasher = Sha256::new();
        let mut read_size = 0_u64;
        let mut buffer = [0_u8; BUFFER_SIZE];
        loop {
            check_cancelled(task)?;
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            read_size = read_size.saturating_add(read as u64);
            encoder.write_all(&buffer[..read])?;
            if let Some(progress) = progress {
                progress.bytes(read, task);
            }
        }
        let writer = encoder.finish()?;
        writer.sync_all()?;
        if read_size != expected_size || hex_digest(&hasher.finalize()) != hash {
            return Err(Error::other(format!(
                "File changed while snapshotting: {}",
                source.display()
            )));
        }

        let created = if state.files.exists(&destination)? {
            let metadata = state.files.symlink_metadata(&destination)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(Error::other(format!(
                    "Snapshot blob path is not a regular file: {}",
                    destination.display()
                )));
            }
            state.files.remove_file_if_exists(&temporary)?;
            false
        } else if let Err(error) = state.files.rename(&temporary, &destination) {
            if state.files.exists(&destination)?
                && !state
                    .files
                    .symlink_metadata(&destination)?
                    .file_type()
                    .is_symlink()
                && state.files.symlink_metadata(&destination)?.is_file()
            {
                state.files.remove_file_if_exists(&temporary)?;
                false
            } else {
                return Err(error);
            }
        } else {
            true
        };
        if let Some(progress) = progress {
            progress.file(task);
        }
        Ok(StoredBlob {
            compressed_size: state.files.metadata(&destination)?.len(),
            created,
        })
    })();
    if result.is_err() {
        let _ = state.files.remove_file_if_exists(&temporary);
    }
    result
}

fn validate_manifest(manifest: &SnapshotManifest) -> Result<()> {
    if manifest.schema_version != SNAPSHOT_SCHEMA {
        return Err(Error::other(format!(
            "Unsupported snapshot format {}.",
            manifest.schema_version
        )));
    }
    if manifest.file_count != manifest.files.len() as u64
        || manifest.size_bytes != manifest.files.iter().map(|file| file.size).sum::<u64>()
    {
        return Err(Error::other("Snapshot manifest totals are invalid."));
    }
    let mut paths = HashSet::new();
    for directory in &manifest.directories {
        if !safe_relative(directory) || !paths.insert(directory.clone()) {
            return Err(Error::other(
                "Snapshot contains an invalid or duplicate path.",
            ));
        }
    }
    for file in &manifest.files {
        if !safe_relative(&file.path)
            || !paths.insert(file.path.clone())
            || !valid_hash(&file.sha256)
        {
            return Err(Error::other("Snapshot contains an invalid file entry."));
        }
    }
    Ok(())
}

fn read_manifest(files: &FileManager, path: &Path) -> Result<SnapshotManifest> {
    if files.symlink_metadata(path)?.file_type().is_symlink() {
        return Err(Error::other(format!(
            "Snapshot manifest is an unsafe symbolic link: {}",
            path.display()
        )));
    }
    let manifest: SnapshotManifest = serde_json::from_slice(&files.read(path)?)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn create_snapshot_sync(
    state: &SnapshotStore,
    instance: &Instance,
    name: String,
    kind: SnapshotKind,
    excluded: &[String],
    task: Option<&TaskHandle>,
) -> Result<SnapshotSummary> {
    let id = uuid::Uuid::new_v4().to_string();
    let source = state.paths.instance_dir(&instance.id);
    let destination = state.paths.snapshot_dir(&instance.id, &id);
    let mut entries = Vec::new();
    collect_entries_excluding(&state.files, &source, &source, &mut entries, true, excluded)?;
    let mut directories = entries
        .iter()
        .filter_map(|entry| match entry {
            SourceEntry::Directory(relative) => Some(relative.clone()),
            SourceEntry::File { .. } => None,
        })
        .collect::<Vec<_>>();
    let mut source_files = entries
        .into_iter()
        .filter_map(|entry| match entry {
            SourceEntry::Directory(_) => None,
            SourceEntry::File { relative, size } => Some((relative, size)),
        })
        .collect::<Vec<_>>();
    directories.sort();
    source_files.sort_by(|a, b| a.0.cmp(&b.0));

    if let Some(task) = task {
        task.stage("hashing");
    }
    let hash_progress = Progress::new(
        source_files.len() as u64,
        source_files.iter().map(|(_, size)| *size).sum(),
        task,
    );
    let snapshot_files = parallel_map(&source_files, |(relative, size)| {
        check_cancelled(task)?;
        let sha256 = hash_file(
            &state.files,
            &source.join(relative),
            *size,
            &hash_progress,
            task,
        )?;
        hash_progress.file(task);
        Ok(SnapshotFile {
            path: relative.clone(),
            size: *size,
            sha256,
        })
    })?;

    let mut unique = HashMap::new();
    for file in &snapshot_files {
        unique
            .entry(file.sha256.clone())
            .or_insert_with(|| BlobSource {
                sha256: file.sha256.clone(),
                source: source.join(&file.path),
                size: file.size,
            });
    }
    let mut blob_sources = unique.into_values().collect::<Vec<_>>();
    blob_sources.sort_by(|a, b| a.sha256.cmp(&b.sha256));
    if let Some(task) = task {
        task.stage("compressing");
    }
    let compression_progress = Progress::new(
        blob_sources.len() as u64,
        blob_sources.iter().map(|blob| blob.size).sum(),
        task,
    );
    let blobs = parallel_map(&blob_sources, |blob| {
        check_cancelled(task)?;
        compress_blob(
            state,
            &blob.source,
            &blob.sha256,
            blob.size,
            Some(&compression_progress),
            task,
        )
    })?;
    let stored_size_bytes = blobs.iter().map(|blob| blob.compressed_size).sum();
    let new_size_bytes = blobs
        .iter()
        .filter(|blob| blob.created)
        .map(|blob| blob.compressed_size)
        .sum();

    let manifest = SnapshotManifest {
        schema_version: SNAPSHOT_SCHEMA,
        id,
        name,
        kind,
        created_at: chrono::Utc::now().timestamp(),
        file_count: snapshot_files.len() as u64,
        size_bytes: snapshot_files.iter().map(|file| file.size).sum(),
        stored_size_bytes,
        new_size_bytes: Some(new_size_bytes),
        excluded: excluded.to_vec(),
        directories,
        files: snapshot_files,
        instance: instance.clone(),
        content: state.db.all_content_files(&instance.id)?,
    };
    validate_manifest(&manifest)?;
    state
        .files
        .write_atomic(destination, &serde_json::to_vec_pretty(&manifest)?)?;
    Ok(manifest.summary())
}

fn list_manifests(state: &SnapshotStore, instance_id: &str) -> Result<Vec<SnapshotManifest>> {
    let parent = state.paths.instance_snapshots(instance_id);
    if !state.files.exists(&parent)? {
        return Ok(Vec::new());
    }
    let mut snapshots = Vec::new();
    for path in state.files.read_dir(parent)? {
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        if state
            .files
            .symlink_metadata(&path)?
            .file_type()
            .is_symlink()
        {
            tracing::warn!(path = %path.display(), "ignored symbolic-link snapshot manifest");
            continue;
        }
        match read_manifest(&state.files, &path) {
            Ok(manifest) if manifest.instance.id == instance_id => snapshots.push(manifest),
            Ok(_) => {
                tracing::warn!(path = %path.display(), "ignored snapshot for another instance")
            }
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "ignored unreadable snapshot")
            }
        }
    }
    snapshots.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    Ok(snapshots)
}

fn list_sync(state: &SnapshotStore, instance_id: &str) -> Result<Vec<SnapshotSummary>> {
    Ok(list_manifests(state, instance_id)?
        .into_iter()
        .map(|manifest| manifest.summary())
        .collect())
}

fn usage_sync(state: &SnapshotStore, instance_id: &str) -> Result<u64> {
    let mut seen = HashSet::new();
    let mut total = 0_u64;
    for manifest in list_manifests(state, instance_id)? {
        for file in manifest.files {
            if !seen.insert(file.sha256.clone()) {
                continue;
            }
            let Some(blob) = state.paths.snapshot_blob(&file.sha256) else {
                continue;
            };
            if let Ok(metadata) = state.files.symlink_metadata(&blob) {
                if metadata.is_file() {
                    total = total.saturating_add(metadata.len());
                }
            }
        }
    }
    Ok(total)
}

fn all_referenced_blobs(state: &SnapshotStore) -> Result<HashSet<String>> {
    let root = state.paths.snapshot_instances();
    if !state.files.exists(&root)? {
        return Ok(HashSet::new());
    }
    let mut referenced = HashSet::new();
    for instance_dir in state.files.read_dir(root)? {
        let metadata = state.files.symlink_metadata(&instance_dir)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::other(format!(
                "Refusing to collect snapshot blobs while a symbolic link exists: {}",
                instance_dir.display()
            )));
        }
        if !metadata.is_dir() {
            continue;
        }
        for path in state.files.read_dir(instance_dir)? {
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            if state
                .files
                .symlink_metadata(&path)?
                .file_type()
                .is_symlink()
            {
                return Err(Error::other(format!(
                    "Refusing to collect snapshot blobs while a manifest is a symbolic link: {}",
                    path.display()
                )));
            }
            let manifest = read_manifest(&state.files, &path).map_err(|error| {
                Error::other(format!(
                    "Cannot collect snapshot blobs because {} is unreadable: {error}",
                    path.display()
                ))
            })?;
            referenced.extend(manifest.files.into_iter().map(|file| file.sha256));
        }
    }
    Ok(referenced)
}

fn garbage_collect(state: &SnapshotStore) -> Result<u64> {
    let referenced = all_referenced_blobs(state)?;
    let root = state.paths.snapshot_blobs();
    if !state.files.exists(&root)? {
        return Ok(0);
    }
    let mut reclaimed = 0_u64;
    for shard in state.files.read_dir(root)? {
        let shard_metadata = state.files.symlink_metadata(&shard)?;
        if shard_metadata.file_type().is_symlink() {
            return Err(Error::other(format!(
                "Refusing to collect snapshot blobs through a symbolic link: {}",
                shard.display()
            )));
        }
        if !shard_metadata.is_dir() {
            continue;
        }
        for path in state.files.read_dir(shard)? {
            let metadata = state.files.symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(Error::other(format!(
                    "Refusing to collect a symbolic-link snapshot blob: {}",
                    path.display()
                )));
            }
            if !metadata.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            let hash = name.strip_suffix(".zst");
            let stale_partial = name.starts_with('.') && name.ends_with(".part");
            if stale_partial || hash.is_some_and(|hash| !referenced.contains(hash)) {
                reclaimed = reclaimed.saturating_add(metadata.len());
                state.files.remove_file_if_exists(path)?;
            }
        }
    }
    Ok(reclaimed)
}

fn prune_automatic(state: &SnapshotStore, instance_id: &str, keep_id: Option<&str>) -> Result<()> {
    let mut automatic = list_manifests(state, instance_id)?
        .into_iter()
        .filter(|snapshot| snapshot.kind == SnapshotKind::Automatic)
        .collect::<Vec<_>>();
    if let Some(index) =
        keep_id.and_then(|keep| automatic.iter().position(|snapshot| snapshot.id == keep))
    {
        let keep = automatic.remove(index);
        automatic.insert(0, keep);
    }
    for snapshot in automatic.into_iter().skip(AUTOMATIC_RETENTION) {
        if let Some(path) = state.paths.snapshot_dir_checked(instance_id, &snapshot.id) {
            state.files.remove_file_if_exists(path)?;
        }
    }
    garbage_collect(state)?;
    Ok(())
}

pub async fn list(state: &AppState, instance_id: &str) -> Result<Vec<SnapshotSummary>> {
    let store = SnapshotStore::from_state(state);
    let instance_id = instance_id.to_string();
    tokio::task::spawn_blocking(move || {
        let _guard = store_guard()?;
        list_sync(&store, &instance_id)
    })
    .await
    .map_err(|error| Error::other(format!("snapshot listing task failed: {error}")))?
}

pub async fn usage(state: &AppState, instance_id: &str) -> Result<u64> {
    let store = SnapshotStore::from_state(state);
    let instance_id = instance_id.to_string();
    tokio::task::spawn_blocking(move || {
        let _guard = store_guard()?;
        usage_sync(&store, &instance_id)
    })
    .await
    .map_err(|error| Error::other(format!("snapshot usage task failed: {error}")))?
}

pub async fn create(
    app: &AppHandle,
    state: &AppState,
    instance: Instance,
    name: Option<String>,
    excluded: Vec<String>,
) -> Result<SnapshotSummary> {
    ensure_no_pending_restore(state, &instance.id)?;
    if instance_busy(state, &instance.id) {
        return Err(Error::other(
            "Stop the instance and wait for its current task before creating a snapshot.",
        ));
    }
    let name = match name {
        Some(name) => clean_name(&name)?,
        None => format!("Snapshot {}", chrono::Local::now().format("%Y-%m-%d %H:%M")),
    };
    let task = Arc::new(state.tasks.start(
        app,
        TaskKind::SnapshotCreate,
        TaskSpec {
            title: format!("Snapshot {}", instance.name),
            subtitle: Some(name.clone()),
            instance_id: Some(instance.id.clone()),
            ..Default::default()
        },
    )?);
    task.stage("indexing");
    let store = SnapshotStore::from_state(state);
    let result = match tokio::task::spawn_blocking({
        let task = Arc::clone(&task);
        move || {
            let _guard = store_guard()?;
            let result = create_snapshot_sync(
                &store,
                &instance,
                name,
                SnapshotKind::Manual,
                &excluded,
                Some(&task),
            );
            if result.is_err() {
                if let Err(error) = garbage_collect(&store) {
                    tracing::warn!(%error, "could not clean incomplete snapshot blobs");
                }
            }
            result
        }
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(Error::other(format!("snapshot task failed: {error}"))),
    };
    task.finish(&result);
    result
}

pub(crate) async fn create_automatic(
    state: &AppState,
    instance: Instance,
    name: String,
    excluded: Vec<String>,
    task: Arc<TaskHandle>,
) -> Result<SnapshotSummary> {
    ensure_no_pending_restore(state, &instance.id)?;
    let name = clean_name(&name)?;
    let store = SnapshotStore::from_state(state);
    tokio::task::spawn_blocking(move || {
        let _guard = store_guard()?;
        let result = create_snapshot_sync(
            &store,
            &instance,
            name,
            SnapshotKind::Automatic,
            &excluded,
            Some(&task),
        );
        if result.is_err() {
            let _ = garbage_collect(&store);
        } else if let Err(error) = prune_automatic(
            &store,
            &instance.id,
            result.as_ref().ok().map(|snapshot| snapshot.id.as_str()),
        ) {
            tracing::warn!(%error, "could not prune automatic snapshots");
        }
        result
    })
    .await
    .map_err(|error| Error::other(format!("automatic snapshot task failed: {error}")))?
}

pub async fn rename(
    state: &AppState,
    instance_id: &str,
    snapshot_id: &str,
    name: &str,
) -> Result<SnapshotSummary> {
    let name = clean_name(name)?;
    let path = state
        .paths
        .snapshot_dir_checked(instance_id, snapshot_id)
        .ok_or_else(|| Error::other("invalid snapshot id"))?;
    let store = SnapshotStore::from_state(state);
    let instance_id = instance_id.to_string();
    let snapshot_id = snapshot_id.to_string();
    tokio::task::spawn_blocking(move || {
        let _guard = store_guard()?;
        let mut manifest = read_manifest(&store.files, &path)?;
        if manifest.instance.id != instance_id || manifest.id != snapshot_id {
            return Err(Error::other(
                "Snapshot identity does not match its location.",
            ));
        }
        manifest.name = name;
        store
            .files
            .write_atomic(path, &serde_json::to_vec_pretty(&manifest)?)?;
        Ok(manifest.summary())
    })
    .await
    .map_err(|error| Error::other(format!("snapshot rename task failed: {error}")))?
}

pub async fn delete(state: &AppState, instance_id: &str, snapshot_id: &str) -> Result<()> {
    ensure_no_pending_restore(state, instance_id)?;
    let path = state
        .paths
        .snapshot_dir_checked(instance_id, snapshot_id)
        .ok_or_else(|| Error::other("invalid snapshot id"))?;
    let store = SnapshotStore::from_state(state);
    let instance_id = instance_id.to_string();
    let snapshot_id = snapshot_id.to_string();
    tokio::task::spawn_blocking(move || {
        let _guard = store_guard()?;
        let manifest = read_manifest(&store.files, &path)?;
        if manifest.instance.id != instance_id || manifest.id != snapshot_id {
            return Err(Error::other(
                "Snapshot identity does not match its location.",
            ));
        }
        store.files.remove_file_if_exists(path)?;
        if let Err(error) = garbage_collect(&store) {
            tracing::warn!(%error, "could not collect unreferenced snapshot blobs");
        }
        Ok(())
    })
    .await
    .map_err(|error| Error::other(format!("snapshot deletion task failed: {error}")))?
}

pub async fn delete_instance_data(state: &AppState, instance_id: &str) -> Result<()> {
    let store = SnapshotStore::from_state(state);
    let instance_id = instance_id.to_string();
    tokio::task::spawn_blocking(move || {
        let _guard = store_guard()?;
        store
            .files
            .remove_managed_dir_all_if_exists(store.paths.instance_snapshots(&instance_id))?;
        if let Err(error) = garbage_collect(&store) {
            tracing::warn!(%error, "could not collect deleted instance snapshot blobs");
        }
        Ok(())
    })
    .await
    .map_err(|error| Error::other(format!("snapshot cleanup task failed: {error}")))?
}

#[cfg(test)]
mod tests {
    use super::restore::{
        recover_journal, restore_blob, restore_paths, stage_restore, RestoreJournal,
    };
    use super::*;

    fn test_store() -> (SnapshotStore, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("basalt-snapshot-test-{}", uuid::Uuid::new_v4()));
        let paths = Paths { root: root.clone() };
        let files = FileManager::new(paths.clone()).unwrap();
        files.ensure_base_dirs().unwrap();
        let db = Db::open(&files).unwrap();
        (SnapshotStore { files, paths, db }, root)
    }

    fn test_instance(store: &SnapshotStore, id: &str) -> Instance {
        let instance = Instance {
            id: id.to_string(),
            name: "Test instance".to_string(),
            version_id: "1.21.1".to_string(),
            created_at: chrono::Utc::now(),
            min_memory_mb: None,
            max_memory_mb: None,
            java_path: None,
            last_played_at: None,
            playtime_secs: 0,
            dir: store.paths.instance_dir(id).display().to_string(),
            logo: None,
            loader: None,
            loader_version: None,
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
        store.db.insert_instance(&instance).unwrap();
        instance
    }

    fn prepare_interrupted_restore(
        store: &SnapshotStore,
        instance: &Instance,
    ) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let live = store.paths.instance_dir(&instance.id);
        store
            .files
            .write_atomic(live.join("options.txt"), b"old snapshot state")
            .unwrap();
        let mut old_metadata = instance.clone();
        old_metadata.version_id = "old-version".into();
        let target = create_snapshot_sync(
            store,
            &old_metadata,
            "Target".into(),
            SnapshotKind::Manual,
            &[],
            None,
        )
        .unwrap();

        store
            .files
            .write_atomic(live.join("options.txt"), b"current state")
            .unwrap();
        let safety = create_snapshot_sync(
            store,
            instance,
            "Safety".into(),
            SnapshotKind::Automatic,
            &[],
            None,
        )
        .unwrap();
        let target_manifest = read_manifest(
            &store.files,
            &store.paths.snapshot_dir(&instance.id, &target.id),
        )
        .unwrap();
        let nonce = uuid::Uuid::new_v4().to_string();
        let (live, staging, backup) = restore_paths(&store.paths, &instance.id, &nonce);
        stage_restore(store, &target_manifest, &live, &staging, None).unwrap();
        let journal_path = store
            .paths
            .snapshot_restore_journal_checked(&instance.id)
            .unwrap();
        store
            .files
            .write_atomic(
                &journal_path,
                &serde_json::to_vec_pretty(&RestoreJournal {
                    schema_version: 1,
                    instance_id: instance.id.clone(),
                    target_snapshot_id: target.id,
                    safety_snapshot_id: safety.id,
                    nonce,
                })
                .unwrap(),
            )
            .unwrap();
        (live, staging, backup, journal_path)
    }

    #[test]
    fn snapshot_names_are_trimmed_and_bounded() {
        assert_eq!(clean_name("  Before update  ").unwrap(), "Before update");
        assert!(clean_name(" ").is_err());
        assert!(clean_name(&"x".repeat(81)).is_err());
    }

    #[test]
    fn volatile_directories_are_explicit() {
        assert!(EXCLUDED_TOP_LEVEL.contains(&"logs"));
        assert!(EXCLUDED_TOP_LEVEL.contains(&"screenshots"));
        assert!(!EXCLUDED_TOP_LEVEL.contains(&"saves"));
        assert!(!EXCLUDED_TOP_LEVEL.contains(&"mods"));
    }

    #[test]
    fn an_excluded_directory_stays_out_of_the_snapshot_and_survives_a_restore() {
        let (store, _root) = test_store();
        let instance = test_instance(&store, "scoped");
        let live = store.paths.instance_dir(&instance.id);
        store
            .files
            .write_atomic(live.join("saves/world/level.dat"), b"original world")
            .unwrap();
        store
            .files
            .write_atomic(live.join("mods/example.jar"), b"original mod")
            .unwrap();

        let summary = create_snapshot_sync(
            &store,
            &instance,
            "Mods only".into(),
            SnapshotKind::Manual,
            &["saves".to_string()],
            None,
        )
        .unwrap();
        assert_eq!(summary.excluded, vec!["saves".to_string()]);

        let manifest = read_manifest(
            &store.files,
            &store.paths.snapshot_dir(&instance.id, &summary.id),
        )
        .unwrap();
        assert!(manifest
            .files
            .iter()
            .all(|file| !file.path.starts_with("saves")));
        assert!(manifest
            .files
            .iter()
            .any(|file| file.path == Path::new("mods/example.jar")));

        store
            .files
            .write_atomic(live.join("saves/world/level.dat"), b"newer world")
            .unwrap();
        store
            .files
            .write_atomic(live.join("mods/example.jar"), b"newer mod")
            .unwrap();

        let staging = store.paths.instances().join(".scoped-staging");
        restore::stage_restore(&store, &manifest, &live, &staging, None).unwrap();

        assert_eq!(
            store.files.read(staging.join("mods/example.jar")).unwrap(),
            b"original mod"
        );
        assert_eq!(
            store
                .files
                .read(staging.join("saves/world/level.dat"))
                .unwrap(),
            b"newer world"
        );
    }

    #[test]
    fn snapshot_collection_keeps_gameplay_data_and_skips_volatile_files() {
        let (store, root) = test_store();
        let instance = store.paths.instance_dir("test");
        store
            .files
            .write_atomic(instance.join("saves/world/level.dat"), b"world")
            .unwrap();
        store
            .files
            .write_atomic(instance.join("mods/example.jar"), b"mod")
            .unwrap();
        store
            .files
            .write_atomic(instance.join("logs/latest.log"), b"log")
            .unwrap();
        store
            .files
            .write_atomic(instance.join("screenshots/view.png"), b"image")
            .unwrap();

        let mut entries = Vec::new();
        collect_entries(&store.files, &instance, &instance, &mut entries, true).unwrap();
        let paths = entries
            .iter()
            .filter_map(|entry| match entry {
                SourceEntry::File { relative, .. } => Some(relative.clone()),
                SourceEntry::Directory(_) => None,
            })
            .collect::<Vec<_>>();
        assert!(paths.contains(&PathBuf::from("saves/world/level.dat")));
        assert!(paths.contains(&PathBuf::from("mods/example.jar")));
        assert!(!paths.contains(&PathBuf::from("logs/latest.log")));
        assert!(!paths.contains(&PathBuf::from("screenshots/view.png")));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn identical_files_share_one_compressed_blob_and_restore_verifies_it() {
        let (store, root) = test_store();
        let source = store.paths.instance_dir("source");
        let original = b"the same content repeated across snapshots".repeat(128);
        store
            .files
            .write_atomic(source.join("config/first.txt"), &original)
            .unwrap();
        store
            .files
            .write_atomic(source.join("mods/second.jar"), &original)
            .unwrap();
        let mut entries = Vec::new();
        collect_entries(&store.files, &source, &source, &mut entries, true).unwrap();
        let progress = progress_for_entries(&entries, None);
        let hashes = entries
            .iter()
            .filter_map(|entry| match entry {
                SourceEntry::File { relative, size } => Some(
                    hash_file(&store.files, &source.join(relative), *size, &progress, None)
                        .unwrap(),
                ),
                SourceEntry::Directory(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(hashes[0], hashes[1]);
        compress_blob(
            &store,
            &source.join("config/first.txt"),
            &hashes[0],
            original.len() as u64,
            None,
            None,
        )
        .unwrap();
        compress_blob(
            &store,
            &source.join("mods/second.jar"),
            &hashes[1],
            original.len() as u64,
            None,
            None,
        )
        .unwrap();
        let shard = store.paths.snapshot_blobs().join(&hashes[0][..2]);
        assert_eq!(store.files.read_dir(shard).unwrap().len(), 1);

        let target = store.paths.instance_dir("restored").join("first.txt");
        let entry = SnapshotFile {
            path: PathBuf::from("first.txt"),
            size: original.len() as u64,
            sha256: hashes[0].clone(),
        };
        let totals = vec![SourceEntry::File {
            relative: entry.path.clone(),
            size: entry.size,
        }];
        let progress = progress_for_entries(&totals, None);
        restore_blob(&store, &entry, &target, &progress, None).unwrap();
        assert_eq!(store.files.read(target).unwrap(), original);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn corrupted_blob_is_rejected_during_restore() {
        let (store, root) = test_store();
        let original = b"protected state";
        let hash = hex_digest(&Sha256::digest(original));
        let blob = store.paths.snapshot_blob(&hash).unwrap();
        store.files.write_atomic(&blob, b"not zstd data").unwrap();
        let entry = SnapshotFile {
            path: PathBuf::from("options.txt"),
            size: original.len() as u64,
            sha256: hash,
        };
        let totals = vec![SourceEntry::File {
            relative: entry.path.clone(),
            size: entry.size,
        }];
        let progress = progress_for_entries(&totals, None);
        assert!(restore_blob(
            &store,
            &entry,
            &store.paths.instance_dir("restore").join("options.txt"),
            &progress,
            None,
        )
        .is_err());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn snapshots_reuse_blobs_and_collection_waits_for_the_last_reference() {
        let (store, root) = test_store();
        let instance = test_instance(&store, "deduplicated");
        let path = store.paths.instance_dir(&instance.id).join("options.txt");
        store
            .files
            .write_atomic(&path, &b"shared snapshot state".repeat(256))
            .unwrap();

        let first = create_snapshot_sync(
            &store,
            &instance,
            "First".into(),
            SnapshotKind::Manual,
            &[],
            None,
        )
        .unwrap();
        let second = create_snapshot_sync(
            &store,
            &instance,
            "Second".into(),
            SnapshotKind::Manual,
            &[],
            None,
        )
        .unwrap();
        assert!(first.new_size_bytes.unwrap() > 0);
        assert_eq!(first.new_size_bytes, Some(first.stored_size_bytes));
        assert_eq!(second.new_size_bytes, Some(0));
        assert_eq!(second.stored_size_bytes, first.stored_size_bytes);
        let first_manifest = read_manifest(
            &store.files,
            &store.paths.snapshot_dir(&instance.id, &first.id),
        )
        .unwrap();
        let blob = store
            .paths
            .snapshot_blob(&first_manifest.files[0].sha256)
            .unwrap();
        assert!(store.files.is_file(&blob).unwrap());
        assert_eq!(
            first_manifest.files[0].sha256,
            read_manifest(
                &store.files,
                &store.paths.snapshot_dir(&instance.id, &second.id),
            )
            .unwrap()
            .files[0]
                .sha256
        );

        store
            .files
            .remove_file_if_exists(store.paths.snapshot_dir(&instance.id, &first.id))
            .unwrap();
        garbage_collect(&store).unwrap();
        assert!(store.files.is_file(&blob).unwrap());

        store
            .files
            .remove_file_if_exists(store.paths.snapshot_dir(&instance.id, &second.id))
            .unwrap();
        garbage_collect(&store).unwrap();
        assert!(!store.files.exists(&blob).unwrap());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn garbage_collection_is_conservative_when_a_manifest_is_corrupt() {
        let (store, root) = test_store();
        let hash = "a".repeat(64);
        let blob = store.paths.snapshot_blob(&hash).unwrap();
        store
            .files
            .write_atomic(&blob, b"potentially referenced")
            .unwrap();
        store
            .files
            .write_atomic(
                store.paths.snapshot_dir("instance", "broken"),
                b"not valid json",
            )
            .unwrap();

        assert!(garbage_collect(&store).is_err());
        assert!(store.files.is_file(blob).unwrap());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn automatic_retention_keeps_three_manifests_without_dropping_shared_data() {
        let (store, root) = test_store();
        let instance = test_instance(&store, "retention");
        store
            .files
            .write_atomic(
                store.paths.instance_dir(&instance.id).join("options.txt"),
                b"state",
            )
            .unwrap();
        for index in 0..5 {
            create_snapshot_sync(
                &store,
                &instance,
                format!("Automatic {index}"),
                SnapshotKind::Automatic,
                &[],
                None,
            )
            .unwrap();
        }

        prune_automatic(&store, &instance.id, None).unwrap();
        let manifests = list_manifests(&store, &instance.id).unwrap();
        assert_eq!(manifests.len(), AUTOMATIC_RETENTION);
        let blob = store
            .paths
            .snapshot_blob(&manifests[0].files[0].sha256)
            .unwrap();
        assert!(store.files.is_file(blob).unwrap());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn recovery_rolls_back_when_activation_never_completed() {
        let (store, root) = test_store();
        let mut instance = test_instance(&store, "rollback");
        instance.version_id = "current-version".into();
        store.db.insert_instance(&instance).unwrap();
        let (live, staging, backup, journal) = prepare_interrupted_restore(&store, &instance);
        store.files.rename(&live, &backup).unwrap();

        recover_journal(&store, &journal).unwrap();

        assert_eq!(
            store.files.read(live.join("options.txt")).unwrap(),
            b"current state"
        );
        assert!(!store.files.exists(staging).unwrap());
        assert!(!store.files.exists(backup).unwrap());
        assert!(!store.files.exists(journal).unwrap());
        assert_eq!(
            store
                .db
                .list_instances(&store.files)
                .unwrap()
                .into_iter()
                .find(|value| value.id == instance.id)
                .unwrap()
                .version_id,
            "current-version"
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn recovery_finishes_an_activated_restore_and_its_metadata() {
        let (store, root) = test_store();
        let mut instance = test_instance(&store, "finish");
        instance.version_id = "current-version".into();
        store.db.insert_instance(&instance).unwrap();
        let (live, staging, backup, journal) = prepare_interrupted_restore(&store, &instance);
        store.files.rename(&live, &backup).unwrap();
        store.files.rename(&staging, &live).unwrap();

        recover_journal(&store, &journal).unwrap();

        assert_eq!(
            store.files.read(live.join("options.txt")).unwrap(),
            b"old snapshot state"
        );
        assert!(!store.files.exists(backup).unwrap());
        assert!(!store.files.exists(journal).unwrap());
        assert_eq!(
            store
                .db
                .list_instances(&store.files)
                .unwrap()
                .into_iter()
                .find(|value| value.id == instance.id)
                .unwrap()
                .version_id,
            "old-version"
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn manifest_paths_cannot_escape_restore_staging() {
        assert!(!safe_relative(Path::new("../escape")));
        assert!(!safe_relative(Path::new("/absolute")));
        assert!(safe_relative(Path::new("saves/world/level.dat")));
    }
}
