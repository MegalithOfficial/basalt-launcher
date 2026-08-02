use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::{
    config::Instance,
    error::{Error, Result},
    files::FileManager,
    instance_ops::instance_busy,
    paths::Paths,
    state::AppState,
    tasks::{TaskHandle, TaskKind, TaskSpec},
};

use super::{
    check_cancelled, clean_name, collect_entries, create_snapshot_sync, garbage_collect,
    hex_digest, progress_for_entries, prune_automatic, read_manifest, store_guard, SnapshotFile,
    SnapshotKind, SnapshotManifest, SnapshotStore, SnapshotSummary, SourceEntry, BUFFER_SIZE,
    EXCLUDED_TOP_LEVEL,
};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct RestoreJournal {
    pub(super) schema_version: u32,
    pub(super) instance_id: String,
    pub(super) target_snapshot_id: String,
    pub(super) safety_snapshot_id: String,
    pub(super) nonce: String,
}

pub(super) fn restore_paths(
    paths: &Paths,
    instance_id: &str,
    nonce: &str,
) -> (PathBuf, PathBuf, PathBuf) {
    (
        paths.instance_dir(instance_id),
        paths
            .instances()
            .join(format!(".restore-{instance_id}-{nonce}")),
        paths
            .instances()
            .join(format!(".restore-backup-{instance_id}-{nonce}")),
    )
}

fn collect_volatile_entries(
    files: &FileManager,
    root: &Path,
    output: &mut Vec<SourceEntry>,
) -> Result<()> {
    for name in EXCLUDED_TOP_LEVEL {
        let path = root.join(name);
        if !files.exists(&path)? {
            continue;
        }
        let metadata = files.symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::other(format!(
                "Cannot preserve a symbolic link during restore: {}",
                path.display()
            )));
        }
        let relative = PathBuf::from(name);
        if metadata.is_dir() {
            output.push(SourceEntry::Directory(relative));
            collect_entries(files, root, &path, output, false)?;
        } else if metadata.is_file() {
            output.push(SourceEntry::File {
                relative,
                size: metadata.len(),
            });
        }
    }
    Ok(())
}

fn copy_plain_file(
    files: &FileManager,
    source: &Path,
    destination: &Path,
    progress: &super::workers::Progress,
    task: Option<&TaskHandle>,
) -> Result<()> {
    let mut reader = files.open(source)?;
    let mut writer = files.create(destination)?;
    let mut buffer = [0_u8; BUFFER_SIZE];
    loop {
        check_cancelled(task)?;
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        progress.bytes(read, task);
    }
    writer.sync_all()?;
    progress.file(task);
    Ok(())
}

pub(super) fn restore_blob(
    state: &SnapshotStore,
    entry: &SnapshotFile,
    destination: &Path,
    progress: &super::workers::Progress,
    task: Option<&TaskHandle>,
) -> Result<()> {
    let blob = state
        .paths
        .snapshot_blob(&entry.sha256)
        .ok_or_else(|| Error::other("snapshot contains an invalid blob hash"))?;
    if !state.files.is_file(&blob)? {
        return Err(Error::other(format!(
            "Snapshot data is missing for {}.",
            entry.path.display()
        )));
    }
    if state
        .files
        .symlink_metadata(&blob)?
        .file_type()
        .is_symlink()
    {
        return Err(Error::other(format!(
            "Snapshot blob is an unsafe symbolic link for {}.",
            entry.path.display()
        )));
    }
    let reader = state.files.open(blob)?;
    let mut decoder = zstd::stream::read::Decoder::new(reader)?;
    let mut writer = state.files.create(destination)?;
    let mut hasher = Sha256::new();
    let mut restored = 0_u64;
    let mut buffer = [0_u8; BUFFER_SIZE];
    loop {
        check_cancelled(task)?;
        let read = decoder.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        restored = restored.saturating_add(read as u64);
        if restored > entry.size {
            return Err(Error::other(format!(
                "Snapshot data expanded beyond its expected size for {}.",
                entry.path.display()
            )));
        }
        hasher.update(&buffer[..read]);
        writer.write_all(&buffer[..read])?;
        progress.bytes(read, task);
    }
    writer.sync_all()?;
    if restored != entry.size || hex_digest(&hasher.finalize()) != entry.sha256 {
        return Err(Error::other(format!(
            "Snapshot integrity check failed for {}.",
            entry.path.display()
        )));
    }
    progress.file(task);
    Ok(())
}

pub(super) fn stage_restore(
    state: &SnapshotStore,
    manifest: &SnapshotManifest,
    live: &Path,
    staging: &Path,
    task: Option<&TaskHandle>,
) -> Result<()> {
    let mut volatile = Vec::new();
    collect_volatile_entries(&state.files, live, &mut volatile)?;
    let mut totals = manifest
        .files
        .iter()
        .map(|file| SourceEntry::File {
            relative: file.path.clone(),
            size: file.size,
        })
        .collect::<Vec<_>>();
    totals.extend(volatile.iter().map(|entry| match entry {
        SourceEntry::Directory(path) => SourceEntry::Directory(path.clone()),
        SourceEntry::File { relative, size } => SourceEntry::File {
            relative: relative.clone(),
            size: *size,
        },
    }));
    let progress = progress_for_entries(&totals, task);
    state.files.ensure_dir(staging)?;
    for directory in &manifest.directories {
        state.files.ensure_dir(staging.join(directory))?;
    }
    for entry in &manifest.files {
        restore_blob(state, entry, &staging.join(&entry.path), &progress, task)?;
    }
    for entry in volatile {
        match entry {
            SourceEntry::Directory(relative) => state.files.ensure_dir(staging.join(relative))?,
            SourceEntry::File { relative, .. } => copy_plain_file(
                &state.files,
                &live.join(&relative),
                &staging.join(relative),
                &progress,
                task,
            )?,
        }
    }
    Ok(())
}

fn restore_database_from_manifest(
    state: &SnapshotStore,
    instance_id: &str,
    manifest: &SnapshotManifest,
) -> Result<()> {
    state
        .db
        .restore_instance_snapshot(instance_id, &manifest.instance, &manifest.content)
}

pub(super) fn recover_journal(state: &SnapshotStore, path: &Path) -> Result<()> {
    if state.files.symlink_metadata(path)?.file_type().is_symlink() {
        return Err(Error::other(format!(
            "Restore journal is an unsafe symbolic link: {}",
            path.display()
        )));
    }
    let journal: RestoreJournal = serde_json::from_slice(&state.files.read(path)?)?;
    if journal.schema_version != 1
        || state
            .paths
            .snapshot_restore_journal_checked(&journal.instance_id)
            .as_deref()
            != Some(path)
        || state
            .paths
            .snapshot_dir_checked(&journal.instance_id, &journal.target_snapshot_id)
            .is_none()
        || state
            .paths
            .snapshot_dir_checked(&journal.instance_id, &journal.safety_snapshot_id)
            .is_none()
        || uuid::Uuid::parse_str(&journal.nonce).is_err()
    {
        return Err(Error::other(format!(
            "Restore journal is invalid: {}",
            path.display()
        )));
    }
    let target_path = state
        .paths
        .snapshot_dir(&journal.instance_id, &journal.target_snapshot_id);
    let safety_path = state
        .paths
        .snapshot_dir(&journal.instance_id, &journal.safety_snapshot_id);
    let target = read_manifest(&state.files, &target_path)?;
    let safety = read_manifest(&state.files, &safety_path)?;
    if target.instance.id != journal.instance_id || safety.instance.id != journal.instance_id {
        return Err(Error::other("Restore journal references another instance."));
    }
    let (live, staging, backup) = restore_paths(&state.paths, &journal.instance_id, &journal.nonce);
    let live_exists = state.files.exists(&live)?;
    let staging_exists = state.files.exists(&staging)?;
    let backup_exists = state.files.exists(&backup)?;

    if backup_exists && live_exists {
        if let Err(error) = restore_database_from_manifest(state, &journal.instance_id, &target) {
            let failed = state.paths.instances().join(format!(
                ".restore-failed-{}-{}",
                journal.instance_id, journal.nonce
            ));
            state.files.rename(&live, &failed)?;
            if let Err(rollback) = state.files.rename(&backup, &live) {
                let _ = state.files.rename(&failed, &live);
                return Err(Error::other(format!(
                    "could not finish recovered snapshot metadata: {error}; filesystem rollback failed: {rollback}"
                )));
            }
            restore_database_from_manifest(state, &journal.instance_id, &safety)?;
            state.files.remove_managed_dir_all_if_exists(failed)?;
        } else {
            state.files.remove_managed_dir_all_if_exists(&backup)?;
        }
        state.files.remove_managed_dir_all_if_exists(&staging)?;
    } else if backup_exists {
        state.files.rename(&backup, &live)?;
        restore_database_from_manifest(state, &journal.instance_id, &safety)?;
        state.files.remove_managed_dir_all_if_exists(&staging)?;
    } else if live_exists && staging_exists {
        state.files.remove_managed_dir_all_if_exists(&staging)?;
        restore_database_from_manifest(state, &journal.instance_id, &safety)?;
    } else if live_exists {
        restore_database_from_manifest(state, &journal.instance_id, &target)?;
    } else {
        return Err(Error::other(format!(
            "Cannot recover restore for {} because both the live instance and backup are missing.",
            journal.instance_id
        )));
    }
    state.files.remove_file_if_exists(path)?;
    tracing::info!(instance_id = %journal.instance_id, "recovered interrupted snapshot restore");
    Ok(())
}

fn cleanup_orphan_restore_staging(state: &SnapshotStore) -> Result<()> {
    for path in state.files.read_dir(state.paths.instances())? {
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !name.starts_with(".restore-") || name.starts_with(".restore-backup-") {
            continue;
        }
        let metadata = state.files.symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            tracing::warn!(path = %path.display(), "left symbolic-link restore staging untouched");
        } else if metadata.is_dir() {
            state.files.remove_managed_dir_all_if_exists(path)?;
        }
    }
    Ok(())
}

pub(crate) fn recover_interrupted(state: &AppState) -> Result<()> {
    let _guard = store_guard()?;
    let store = SnapshotStore::from_state(state);
    let root = store.paths.snapshot_restore_journals();
    let mut first_error = None;
    if store.files.exists(&root)? {
        for path in store.files.read_dir(&root)? {
            if path.extension().and_then(|value| value.to_str()) == Some("json") {
                if let Err(error) = recover_journal(&store, &path) {
                    tracing::error!(path = %path.display(), %error, "could not recover snapshot restore");
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
    }
    if let Err(error) = garbage_collect(&store) {
        tracing::warn!(%error, "could not collect snapshot blobs during startup recovery");
    }
    match first_error {
        Some(error) => Err(error),
        None => cleanup_orphan_restore_staging(&store),
    }
}

pub(crate) async fn restore(
    app: &AppHandle,
    state: &AppState,
    instance: Instance,
    snapshot_id: &str,
) -> Result<SnapshotSummary> {
    super::ensure_no_pending_restore(state, &instance.id)?;
    if instance_busy(state, &instance.id) {
        return Err(Error::other(
            "Stop the instance and wait for its current task before restoring a snapshot.",
        ));
    }
    let snapshot_path = state
        .paths
        .snapshot_dir_checked(&instance.id, snapshot_id)
        .ok_or_else(|| Error::other("invalid snapshot id"))?;
    let snapshot_id = snapshot_id.to_string();
    let task = Arc::new(state.tasks.start(
        app,
        TaskKind::SnapshotRestore,
        TaskSpec {
            title: format!("Restore {}", instance.name),
            instance_id: Some(instance.id.clone()),
            ..Default::default()
        },
    ));
    let instance_id = instance.id.clone();
    let store = SnapshotStore::from_state(state);
    let result = match tokio::task::spawn_blocking({
        let task = Arc::clone(&task);
        move || {
            let _guard = store_guard()?;
            let manifest = read_manifest(&store.files, &snapshot_path)?;
            if manifest.instance.id != instance.id || manifest.id != snapshot_id {
                return Err(Error::other("Snapshot identity does not match its location."));
            }

            let journal_path = store
                .paths
                .snapshot_restore_journal_checked(&instance.id)
                .ok_or_else(|| Error::other("invalid instance id for restore journal"))?;
            if store.files.exists(&journal_path)? {
                return Err(Error::other(
                    "This instance has an interrupted restore that must be recovered first.",
                ));
            }

            task.stage("safety-snapshot");
            let safety_name = format!("Before restoring {}", manifest.name)
                .chars()
                .take(80)
                .collect::<String>();
            let safety = match create_snapshot_sync(
                &store,
                &instance,
                clean_name(&safety_name)?,
                SnapshotKind::Automatic,
                Some(&task),
            ) {
                Ok(safety) => safety,
                Err(error) => {
                    if let Err(cleanup_error) = garbage_collect(&store) {
                        tracing::warn!(%cleanup_error, "could not clean incomplete safety snapshot blobs");
                    }
                    return Err(error);
                }
            };

            task.stage("verifying-and-staging");
            let nonce = uuid::Uuid::new_v4().to_string();
            let (live, staging, backup) = restore_paths(&store.paths, &instance.id, &nonce);
            if let Err(error) = stage_restore(&store, &manifest, &live, &staging, Some(&task)) {
                let _ = store.files.remove_managed_dir_all_if_exists(&staging);
                return Err(error);
            }
            if let Err(error) = check_cancelled(Some(&task)) {
                let _ = store.files.remove_managed_dir_all_if_exists(&staging);
                return Err(error);
            }

            let journal = RestoreJournal {
                schema_version: 1,
                instance_id: instance.id.clone(),
                target_snapshot_id: manifest.id.clone(),
                safety_snapshot_id: safety.id,
                nonce,
            };
            let journal_bytes = serde_json::to_vec_pretty(&journal)?;
            if let Err(error) = store.files.write_atomic(&journal_path, &journal_bytes) {
                let _ = store.files.remove_managed_dir_all_if_exists(&staging);
                return Err(error);
            }

            task.stage("activating-restore");
            if let Err(error) = store.files.rename(&live, &backup) {
                let _ = store.files.remove_managed_dir_all_if_exists(&staging);
                let _ = store.files.remove_file_if_exists(&journal_path);
                return Err(error);
            }
            if let Err(error) = store.files.rename(&staging, &live) {
                let rollback = store.files.rename(&backup, &live);
                let _ = store.files.remove_managed_dir_all_if_exists(&staging);
                return match rollback {
                    Ok(()) => {
                        let _ = store.files.remove_file_if_exists(&journal_path);
                        Err(error)
                    }
                    Err(rollback) => Err(Error::other(format!(
                        "restore activation failed: {error}; restoring the original folder also failed: {rollback}"
                    ))),
                };
            }
            if let Err(error) = store.db.restore_instance_snapshot(
                &instance.id,
                &manifest.instance,
                &manifest.content,
            ) {
                let _ = store.files.remove_managed_dir_all_if_exists(&live);
                let rollback = store.files.rename(&backup, &live);
                return match rollback {
                    Ok(()) => {
                        let _ = store.files.remove_file_if_exists(&journal_path);
                        Err(error)
                    }
                    Err(rollback) => Err(Error::other(format!(
                        "snapshot metadata failed: {error}; restoring the original folder also failed: {rollback}"
                    ))),
                };
            }
            match store.files.remove_managed_dir_all_if_exists(&backup) {
                Ok(_) => {
                    store.files.remove_file_if_exists(&journal_path)?;
                    if let Err(error) = prune_automatic(&store, &instance.id) {
                        tracing::warn!(%error, "could not prune automatic snapshots");
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "could not remove restore backup; startup recovery will retry");
                }
            }
            Ok(manifest.summary())
        }
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(Error::other(format!("snapshot restore task failed: {error}"))),
    };
    task.finish(&result);
    if result.is_ok() {
        state.media_cache.lock().unwrap().remove(&instance_id);
    }
    result
}
