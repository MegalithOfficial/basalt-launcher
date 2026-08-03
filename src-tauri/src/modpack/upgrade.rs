use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    config::Instance,
    error::{Error, Result},
    instance_ops::instance_busy,
    search::{self, ContentKind, Provider},
    snapshots,
    state::AppState,
    tasks::{TaskKind, TaskSpec},
};

use super::{
    install_pack_body, loader_from_dependencies, prepare_pack, sanitize_relative, ManualDownload,
    ManualDownloadSource, MrIndex, PreparePackOutcome,
};

const PACK_STATE_SCHEMA: u32 = 1;
const PACK_STATE_PATH: &str = ".basalt/pack-state.json";

#[derive(Debug, Serialize, Deserialize)]
struct UpgradeJournal {
    schema_version: u32,
    instance: Instance,
    content: Vec<(String, crate::db::ContentFile)>,
    nonce: String,
}

fn upgrade_paths(state: &AppState, instance_id: &str, nonce: &str) -> (PathBuf, PathBuf, PathBuf) {
    (
        state.paths.instance_dir(instance_id),
        state
            .paths
            .instances()
            .join(format!(".upgrade-{instance_id}-{nonce}")),
        state
            .paths
            .instances()
            .join(format!(".upgrade-backup-{instance_id}-{nonce}")),
    )
}

pub fn recover_interrupted_upgrades(state: &AppState) -> Result<()> {
    let root = state.paths.modpack_upgrade_journals();
    if !state.files.exists(&root)? {
        return Ok(());
    }
    for path in state.files.read_dir(&root)? {
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        if state
            .files
            .symlink_metadata(&path)?
            .file_type()
            .is_symlink()
        {
            return Err(Error::other(format!(
                "Modpack upgrade journal is an unsafe symbolic link: {}",
                path.display()
            )));
        }
        let journal: UpgradeJournal = serde_json::from_slice(&state.files.read(&path)?)?;
        if journal.schema_version != 1
            || uuid::Uuid::parse_str(&journal.nonce).is_err()
            || state
                .paths
                .modpack_upgrade_journal(&journal.instance.id)
                .as_deref()
                != Some(path.as_path())
        {
            return Err(Error::other(format!(
                "Modpack upgrade journal is invalid: {}",
                path.display()
            )));
        }
        let (live, staging, backup) = upgrade_paths(state, &journal.instance.id, &journal.nonce);
        if state.files.exists(&backup)? {
            state.files.remove_managed_dir_all_if_exists(&live)?;
            state.files.rename(&backup, &live)?;
            state.db.restore_instance_snapshot(
                &journal.instance.id,
                &journal.instance,
                &journal.content,
            )?;
            tracing::warn!(instance_id = %journal.instance.id, "rolled back an interrupted modpack upgrade");
        }
        state.files.remove_managed_dir_all_if_exists(staging)?;
        state.files.remove_file_if_exists(path)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ManagedKind {
    Declared,
    Override,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManagedFile {
    path: String,
    sha1: Option<String>,
    kind: ManagedKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PackState {
    schema_version: u32,
    version_id: String,
    files: Vec<ManagedFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModpackUpgrade {
    pub current_version_id: String,
    pub target_version_id: String,
    pub target_name: String,
    pub version_number: String,
    pub channel: String,
    pub date: String,
    pub game_version: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ModpackUpgradeChanges {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
    pub preserved: Vec<String>,
    pub unchanged: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModpackUpgradePlan {
    pub update: ModpackUpgrade,
    pub manual_downloads: Vec<ManualDownload>,
    pub changes: Option<ModpackUpgradeChanges>,
}

fn identity(instance: &Instance) -> Result<(Provider, &str, &str)> {
    let provider = instance
        .pack_provider
        .as_deref()
        .ok_or_else(|| Error::other("This instance is not linked to a modpack project."))?;
    let project_id = instance
        .pack_project_id
        .as_deref()
        .ok_or_else(|| Error::other("This instance is missing its modpack project ID."))?;
    let version_id = instance
        .pack_version_id
        .as_deref()
        .ok_or_else(|| Error::other("This instance is missing its installed pack version."))?;
    Ok((Provider::parse(provider)?, project_id, version_id))
}

fn upgrade_from_version(
    current_version_id: &str,
    version: &search::ProjectVersion,
) -> Result<ModpackUpgrade> {
    let fetched_game = version.game_versions.first().cloned().unwrap_or_default();
    Ok(ModpackUpgrade {
        current_version_id: current_version_id.to_string(),
        target_version_id: version.id.clone(),
        target_name: version.name.clone(),
        version_number: version.version_number.clone(),
        channel: version.channel.clone(),
        date: version.date.clone(),
        game_version: fetched_game,
        loader: version.loaders.first().cloned(),
        loader_version: None,
    })
}

fn channel_rank(channel: &str) -> u8 {
    match channel {
        "release" => 0,
        "beta" => 1,
        _ => 2,
    }
}

async fn target_update(
    state: &AppState,
    instance: &Instance,
    requested: Option<&str>,
) -> Result<Option<ModpackUpgrade>> {
    let (provider, project_id, current) = identity(instance)?;
    let versions =
        search::project_versions(state, provider, project_id, ContentKind::Modpack, "", None)
            .await?;
    if let Some(target) = requested {
        if target == current {
            return Ok(None);
        }
        return versions
            .iter()
            .find(|version| version.id == target)
            .map(|version| upgrade_from_version(current, version))
            .transpose();
    }
    let latest = if let Some(installed) = versions.iter().find(|version| version.id == current) {
        let accepted_rank = channel_rank(&installed.channel);
        versions
            .iter()
            .filter(|version| channel_rank(&version.channel) <= accepted_rank)
            .max_by(|a, b| a.date.cmp(&b.date))
            .cloned()
    } else {
        search::pick_best(versions.clone())
    };
    let Some(latest) = latest else {
        return Ok(None);
    };
    if latest.id == current {
        return Ok(None);
    }
    Ok(Some(upgrade_from_version(current, &latest)?))
}

pub(crate) async fn ensure_pack_state(app: &AppHandle, state: &AppState, instance: &Instance) {
    let path = state_path(&state.paths.instance_dir(&instance.id));
    if state.files.is_file(&path).unwrap_or(false) {
        return;
    }
    let (Some(provider), Some(project), Some(version)) = (
        instance.pack_provider.as_deref(),
        instance.pack_project_id.as_deref(),
        instance.pack_version_id.as_deref(),
    ) else {
        return;
    };
    let Ok(provider) = Provider::parse(provider) else {
        return;
    };
    if let Err(error) = adopt_pack_state(app, state, instance, provider, project, version).await {
        tracing::warn!(
            instance_id = %instance.id,
            %error,
            "could not read the installed pack version, this upgrade will replace pack configs"
        );
    } else {
        tracing::info!(instance_id = %instance.id, "adopted the installed pack version as a baseline");
    }
}

pub async fn adopt_pack_state(
    app: &AppHandle,
    state: &AppState,
    instance: &Instance,
    provider: Provider,
    project_id: &str,
    version_id: &str,
) -> Result<()> {
    let prepared = match prepare_pack(app, state, provider, project_id, version_id, &[], false)
        .await?
    {
        PreparePackOutcome::Ready(prepared) => prepared,
        PreparePackOutcome::NeedsDownloads(_) => {
            return Err(Error::other(
                "This pack version needs files that CurseForge will not hand over automatically, so Basalt cannot read its baseline.",
            ))
        }
    };
    let instance_dir = state.paths.instance_dir(&instance.id);
    write_pack_state(
        state,
        &instance_dir,
        &prepared.target.id,
        &prepared.index,
        &prepared.archive_path,
    )?;
    Ok(())
}

pub async fn link_modpack(
    app: &AppHandle,
    state: &AppState,
    instance: &Instance,
    provider: Provider,
    project_id: &str,
    version_id: &str,
) -> Result<()> {
    if instance_busy(state, &instance.id) {
        return Err(Error::other(
            "Stop the instance and wait for its current task before linking a modpack.",
        ));
    }
    adopt_pack_state(app, state, instance, provider, project_id, version_id).await?;
    let state_file = read_pack_state(state, instance)?;
    let owned: Vec<String> = state_file
        .files
        .iter()
        .filter_map(|file| file.path.rsplit('/').next().map(str::to_string))
        .collect();
    state.db.link_instance_pack(
        &instance.id,
        provider.as_str(),
        project_id,
        version_id,
        &owned,
    )?;
    Ok(())
}

pub async fn check_modpack_upgrade(
    state: &AppState,
    instance: &Instance,
) -> Result<Option<ModpackUpgrade>> {
    target_update(state, instance, None).await
}

fn sha1_bytes(bytes: &[u8]) -> String {
    let mut hash = sha1_smol::Sha1::new();
    hash.update(bytes);
    hash.digest().to_string()
}

fn archive_overrides(archive_path: &Path) -> Result<Vec<ManagedFile>> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| Error::other(format!("opening modpack archive: {error}")))?;
    let mut files = BTreeMap::new();
    for prefix in ["overrides/", "client-overrides/"] {
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|error| Error::other(format!("reading modpack entry: {error}")))?;
            let name = entry.name().to_string();
            let Some(relative) = name.strip_prefix(prefix) else {
                continue;
            };
            if relative.is_empty() || name.ends_with('/') {
                continue;
            }
            let relative = sanitize_relative(relative)?;
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut bytes)?;
            files.insert(
                relative.to_string_lossy().replace('\\', "/"),
                sha1_bytes(&bytes),
            );
        }
    }
    Ok(files
        .into_iter()
        .map(|(path, sha1)| ManagedFile {
            path,
            sha1: Some(sha1),
            kind: ManagedKind::Override,
        })
        .collect())
}

fn state_from_pack(version_id: &str, index: &MrIndex, archive: &Path) -> Result<PackState> {
    let mut files = BTreeMap::<String, ManagedFile>::new();
    for file in &index.files {
        if file
            .env
            .as_ref()
            .and_then(|environment| environment.client.as_deref())
            == Some("unsupported")
        {
            continue;
        }
        let relative = sanitize_relative(&file.path)?;
        let path = relative.to_string_lossy().replace('\\', "/");
        files.insert(
            path.clone(),
            ManagedFile {
                path,
                sha1: file.hashes.sha1.clone(),
                kind: ManagedKind::Declared,
            },
        );
    }
    for file in archive_overrides(archive)? {
        files.insert(file.path.clone(), file);
    }
    Ok(PackState {
        schema_version: PACK_STATE_SCHEMA,
        version_id: version_id.to_string(),
        files: files.into_values().collect(),
    })
}

fn state_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join(PACK_STATE_PATH)
}

fn read_pack_state(state: &AppState, instance: &Instance) -> Result<PackState> {
    let path = state_path(&state.paths.instance_dir(&instance.id));
    if state.files.is_file(&path)? {
        let value: PackState = serde_json::from_slice(&state.files.read(path)?)?;
        if value.schema_version == PACK_STATE_SCHEMA {
            return Ok(value);
        }
    }

    let files = state
        .db
        .all_content_files(&instance.id)?
        .into_iter()
        .filter(|(_, file)| file.origin == "pack")
        .map(|(kind, file)| ManagedFile {
            path: format!("{kind}/{}", file.file_name),
            sha1: file.sha1,
            kind: ManagedKind::Declared,
        })
        .collect();
    Ok(PackState {
        schema_version: PACK_STATE_SCHEMA,
        version_id: instance.pack_version_id.clone().unwrap_or_default(),
        files,
    })
}

pub(super) fn write_pack_state(
    state: &AppState,
    instance_dir: &Path,
    version_id: &str,
    index: &MrIndex,
    archive: &Path,
) -> Result<()> {
    let pack_state = state_from_pack(version_id, index, archive)?;
    state.files.write_atomic(
        state_path(instance_dir),
        &serde_json::to_vec_pretty(&pack_state)?,
    )
}

fn file_sha1(state: &AppState, path: &Path) -> Result<Option<String>> {
    if !state.files.is_file(path)? {
        return Ok(None);
    }
    let mut source = state.files.open(path)?;
    let mut hasher = sha1_smol::Sha1::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(Some(hasher.digest().to_string()))
}

fn changes(
    state: &AppState,
    instance: &Instance,
    old: &PackState,
    new: &PackState,
) -> Result<ModpackUpgradeChanges> {
    let old_files: HashMap<&str, &ManagedFile> = old
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let new_files: HashMap<&str, &ManagedFile> = new
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let root = state.paths.instance_dir(&instance.id);
    let mut result = ModpackUpgradeChanges::default();

    for file in &new.files {
        match old_files.get(file.path.as_str()) {
            None => result.added.push(file.path.clone()),
            Some(old) if old.sha1 != file.sha1 || old.kind != file.kind => {
                result.changed.push(file.path.clone())
            }
            Some(_) => result.unchanged += 1,
        }
    }
    for file in &old.files {
        if !new_files.contains_key(file.path.as_str()) {
            result.removed.push(file.path.clone());
        }
        if file.kind == ManagedKind::Override
            && file_sha1(state, &root.join(&file.path))? != file.sha1
        {
            result.preserved.push(file.path.clone());
        }
    }
    for file in new
        .files
        .iter()
        .filter(|file| file.kind == ManagedKind::Override)
    {
        if !old_files.contains_key(file.path.as_str())
            && state.files.is_file(root.join(&file.path))?
        {
            result.preserved.push(file.path.clone());
        }
    }
    result.preserved.sort();
    result.preserved.dedup();
    Ok(result)
}

pub async fn plan_modpack_upgrade(
    app: &AppHandle,
    state: &AppState,
    instance: &Instance,
    target_version_id: &str,
    manual_sources: &[ManualDownloadSource],
) -> Result<ModpackUpgradePlan> {
    let mut update = target_update(state, instance, Some(target_version_id))
        .await?
        .ok_or_else(|| Error::other("That modpack version is already installed or unavailable."))?;
    let (provider, project_id, _) = identity(instance)?;
    let outcome = prepare_pack(
        app,
        state,
        provider,
        project_id,
        target_version_id,
        manual_sources,
        false,
    )
    .await?;
    match outcome {
        PreparePackOutcome::NeedsDownloads(manual_downloads) => Ok(ModpackUpgradePlan {
            update,
            manual_downloads,
            changes: None,
        }),
        PreparePackOutcome::Ready(prepared) => {
            ensure_pack_state(app, state, instance).await;
            let old = read_pack_state(state, instance)?;
            let new =
                state_from_pack(&prepared.target.id, &prepared.index, &prepared.archive_path)?;
            update.game_version = prepared
                .index
                .dependencies
                .get("minecraft")
                .cloned()
                .unwrap_or_default();
            if let Some((loader, version)) = loader_from_dependencies(&prepared.index.dependencies)?
            {
                update.loader = Some(loader);
                update.loader_version = Some(version);
            } else {
                update.loader = None;
                update.loader_version = None;
            }
            Ok(ModpackUpgradePlan {
                update,
                manual_downloads: Vec::new(),
                changes: Some(changes(state, instance, &old, &new)?),
            })
        }
    }
}

#[derive(Debug)]
struct CopyEntry {
    source: PathBuf,
    destination: PathBuf,
    size: u64,
}

fn collect_copy_entries(
    files: &crate::files::FileManager,
    source: &Path,
    destination: &Path,
    entries: &mut Vec<CopyEntry>,
) -> Result<()> {
    for path in files.read_dir(source)? {
        let metadata = files.symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::other(format!(
                "Cannot upgrade an instance containing a symbolic link: {}",
                path.display()
            )));
        }
        let target = destination.join(path.file_name().unwrap_or_default());
        if metadata.is_dir() {
            collect_copy_entries(files, &path, &target, entries)?;
        } else if metadata.is_file() {
            entries.push(CopyEntry {
                source: path,
                destination: target,
                size: metadata.len(),
            });
        }
    }
    Ok(())
}

fn copy_instance(
    files: &crate::files::FileManager,
    source: &Path,
    destination: &Path,
    task: &crate::tasks::TaskHandle,
) -> Result<()> {
    let mut entries = Vec::new();
    collect_copy_entries(files, source, destination, &mut entries)?;
    let total = entries.len() as u64;
    let total_bytes = entries.iter().map(|entry| entry.size).sum();
    task.set_total(total, total_bytes);
    files.ensure_dir(destination)?;
    let mut linked = 0_u64;
    for (index, entry) in entries.into_iter().enumerate() {
        if task.token().is_cancelled() {
            return Err(Error::Cancelled);
        }
        let size = entry.size;
        files.link_or_copy(entry.source, entry.destination)?;
        linked += size;
        task.progress(index as u64 + 1, total, linked, total_bytes);
    }
    Ok(())
}

fn preserved_overrides(
    state: &AppState,
    instance: &Instance,
    old: &PackState,
    new: &PackState,
) -> Result<HashSet<String>> {
    let root = state.paths.instance_dir(&instance.id);
    let old_files: HashMap<&str, &ManagedFile> = old
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    let mut preserved = HashSet::new();
    for file in old
        .files
        .iter()
        .filter(|file| file.kind == ManagedKind::Override)
    {
        if file_sha1(state, &root.join(&file.path))? != file.sha1 {
            preserved.insert(file.path.clone());
        }
    }
    for file in new
        .files
        .iter()
        .filter(|file| file.kind == ManagedKind::Override)
    {
        if !old_files.contains_key(file.path.as_str())
            && state.files.is_file(root.join(&file.path))?
        {
            preserved.insert(file.path.clone());
        }
    }
    Ok(preserved)
}

fn clean_old_files(
    state: &AppState,
    staging: &Path,
    old: &PackState,
    new: &PackState,
    preserved: &HashSet<String>,
) -> Result<()> {
    let new_paths: HashSet<&str> = new.files.iter().map(|file| file.path.as_str()).collect();
    for file in &old.files {
        if preserved.contains(&file.path) {
            continue;
        }
        if !new_paths.contains(file.path.as_str()) {
            state
                .files
                .remove_file_if_exists(staging.join(&file.path))?;
        }
    }
    Ok(())
}

fn restore_preserved(
    state: &AppState,
    live: &Path,
    staging: &Path,
    preserved: &HashSet<String>,
) -> Result<()> {
    for path in preserved {
        let source = live.join(path);
        let destination = staging.join(path);
        if state.files.is_file(&source)? {
            let mut source = state.files.open(source)?;
            state
                .files
                .copy_reader_into_sync(&mut source, destination)?;
        } else {
            state.files.remove_file_if_exists(destination)?;
        }
    }
    Ok(())
}

pub async fn upgrade_modpack(
    app: &AppHandle,
    state: &AppState,
    instance: Instance,
    target_version_id: &str,
    manual_sources: &[ManualDownloadSource],
    snapshot_first: bool,
) -> Result<Instance> {
    if instance_busy(state, &instance.id) {
        return Err(Error::other(
            "Stop the instance and wait for its current task before upgrading the modpack.",
        ));
    }
    let update = target_update(state, &instance, Some(target_version_id))
        .await?
        .ok_or_else(|| Error::other("That modpack version is already installed or unavailable."))?;
    let (provider, project_id, _) = identity(&instance)?;
    let outcome = prepare_pack(
        app,
        state,
        provider,
        project_id,
        target_version_id,
        manual_sources,
        true,
    )
    .await?;
    let PreparePackOutcome::Ready(prepared) = outcome else {
        return Err(Error::other(
            "Download all requested CurseForge files before continuing.",
        ));
    };
    let prepared = *prepared;
    let game_version = prepared
        .index
        .dependencies
        .get("minecraft")
        .cloned()
        .ok_or_else(|| Error::other("Pack index does not declare a Minecraft version."))?;
    let loader = loader_from_dependencies(&prepared.index.dependencies)?;
    ensure_pack_state(app, state, &instance).await;
    let old_state = read_pack_state(state, &instance)?;
    let new_state = state_from_pack(&prepared.target.id, &prepared.index, &prepared.archive_path)?;
    let preserved = preserved_overrides(state, &instance, &old_state, &new_state)?;

    let task = Arc::new(state.tasks.start(
        app,
        TaskKind::ModpackUpgrade,
        TaskSpec {
            title: format!("Upgrade {}", instance.name),
            subtitle: Some(format!("{} -> {}", instance.version_id, game_version)),
            instance_id: Some(instance.id.clone()),
            project_id: instance.pack_project_id.clone(),
            ..Default::default()
        },
    ));
    let result = async {
        if snapshot_first {
            task.stage("safety-snapshot");
            let snapshot_name = format!("Before upgrade to {}", update.version_number)
                .chars()
                .take(80)
                .collect();
            snapshots::create_automatic(
                state,
                instance.clone(),
                snapshot_name,
                Vec::new(),
                Arc::clone(&task),
            )
            .await?;
        }

        let nonce = uuid::Uuid::new_v4().to_string();
        let (live, staging, backup) = upgrade_paths(state, &instance.id, &nonce);

        task.stage("copying-instance");
        let copy = {
            let task = Arc::clone(&task);
            let files = state.files.clone();
            let live = live.clone();
            let staging = staging.clone();
            tokio::task::spawn_blocking(move || copy_instance(&files, &live, &staging, &task))
                .await
                .map_err(|error| Error::other(format!("upgrade copy task failed: {error}")))?
        };
        if let Err(error) = copy {
            let _ = state.files.remove_managed_dir_all_if_exists(&staging);
            return Err(error);
        }
        clean_old_files(state, &staging, &old_state, &new_state, &preserved)?;

        let mut upgraded = instance.clone();
        upgraded.version_id = game_version;
        upgraded.loader = loader.as_ref().map(|value| value.0.clone());
        upgraded.loader_version = loader.as_ref().map(|value| value.1.clone());
        upgraded.launch_version_id = None;
        upgraded.pack_version_id = Some(prepared.target.id.clone());

        let artifacts = match install_pack_body(
            app,
            state,
            None,
            &upgraded,
            &staging,
            &prepared.archive_path,
            &prepared.index,
            &task,
        )
        .await
        {
            Ok(artifacts) => artifacts,
            Err(error) => {
                let _ = state.files.remove_managed_dir_all_if_exists(&staging);
                return Err(error);
            }
        };
        if task.token().is_cancelled() {
            let _ = state.files.remove_managed_dir_all_if_exists(&staging);
            return Err(Error::Cancelled);
        }
        restore_preserved(state, &live, &staging, &preserved)?;
        state.files.write_atomic(
            state_path(&staging),
            &serde_json::to_vec_pretty(&new_state)?,
        )?;

        let old_content = state.db.all_content_files(&instance.id)?;
        let journal_path = state
            .paths
            .modpack_upgrade_journal(&instance.id)
            .ok_or_else(|| Error::other("invalid instance id for upgrade journal"))?;
        state.files.write_atomic(
            &journal_path,
            &serde_json::to_vec_pretty(&UpgradeJournal {
                schema_version: 1,
                instance: instance.clone(),
                content: old_content.clone(),
                nonce,
            })?,
        )?;
        task.stage("activating-upgrade");
        if let Err(error) = state.files.rename(&live, &backup) {
            let _ = state.files.remove_file_if_exists(&journal_path);
            return Err(error);
        }
        if let Err(error) = state.files.rename(&staging, &live) {
            let rollback = state.files.rename(&backup, &live);
            return match rollback {
                Ok(()) => {
                    let _ = state.files.remove_file_if_exists(&journal_path);
                    Err(error)
                }
                Err(rollback) => Err(Error::other(format!(
                    "upgrade activation failed: {error}; restoring the original folder also failed: {rollback}"
                ))),
            };
        }

        upgraded.launch_version_id = Some(artifacts.launch_id.clone());
        let persist = async {
            state
                .db
                .set_modpack_version(&upgraded, &artifacts.launch_id)?;
            state.db.delete_pack_content_files(&instance.id)?;
            for (kind, _, file) in &prepared.curseforge_links {
                state.db.record_content_file(&instance.id, kind, file)?;
            }
            super::link_pack_files(state, &instance.id, &artifacts.linkable).await;
            Result::<()>::Ok(())
        }
        .await;
        if let Err(error) = persist {
            let _ = state.files.remove_managed_dir_all_if_exists(&live);
            let filesystem_rollback = state.files.rename(&backup, &live);
            let metadata_rollback = state
                .db
                .restore_instance_snapshot(&instance.id, &instance, &old_content);
            return match (filesystem_rollback, metadata_rollback) {
                (Ok(()), Ok(())) => {
                    let _ = state.files.remove_file_if_exists(&journal_path);
                    Err(error)
                }
                (filesystem, metadata) => Err(Error::other(format!(
                    "upgrade metadata failed: {error}; rollback failed: filesystem={filesystem:?}, metadata={metadata:?}"
                ))),
            };
        }
        if let Err(error) = state.files.remove_managed_dir_all_if_exists(&backup) {
            tracing::warn!(path = %backup.display(), %error, "could not remove completed upgrade backup");
        }
        state.files.remove_file_if_exists(&journal_path)?;
        for source in &prepared.consumed_sources {
            if let Err(error) = std::fs::remove_file(source) {
                tracing::warn!(path = %source.display(), %error, "could not remove consumed CurseForge download");
            }
        }
        Ok(upgraded)
    }
    .await;
    let staging_prefix = format!(".upgrade-{}-", instance.id);
    if let Ok(entries) = state.files.read_dir(state.paths.instances()) {
        for path in entries {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&staging_prefix))
            {
                let _ = state.files.remove_managed_dir_all_if_exists(path);
            }
        }
    }
    task.finish(&result);
    if result.is_ok() {
        state.media_cache.lock().unwrap().remove(&instance.id);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::Db, files::FileManager, paths::Paths};

    fn test_instance(root: &Path, id: &str, pack_version: &str) -> Instance {
        Instance {
            id: id.to_string(),
            name: "Pack".to_string(),
            version_id: "1.21.1".to_string(),
            created_at: chrono::Utc::now(),
            min_memory_mb: None,
            max_memory_mb: None,
            java_path: None,
            last_played_at: None,
            playtime_secs: 0,
            dir: root.join("instances").join(id).display().to_string(),
            logo: None,
            loader: Some("fabric".to_string()),
            loader_version: Some("0.16.0".to_string()),
            launch_version_id: Some("old-launch".to_string()),
            pack_provider: Some("modrinth".to_string()),
            pack_project_id: Some("project".to_string()),
            pack_version_id: Some(pack_version.to_string()),
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
        }
    }

    #[test]
    fn interrupted_activation_restores_the_original_folder_and_metadata() {
        let root =
            std::env::temp_dir().join(format!("basalt-pack-upgrade-test-{}", uuid::Uuid::new_v4()));
        let paths = Paths { root: root.clone() };
        let files = FileManager::new(paths.clone()).unwrap();
        files.ensure_base_dirs().unwrap();
        let db = Db::open(&files).unwrap();
        let state = AppState::new(files, db);
        let instance = test_instance(&root, "upgrade-test", "old-pack");
        state.db.insert_instance(&instance).unwrap();

        let nonce = uuid::Uuid::new_v4().to_string();
        let (live, staging, backup) = upgrade_paths(&state, &instance.id, &nonce);
        state
            .files
            .write_atomic(live.join("marker"), b"new")
            .unwrap();
        state
            .files
            .write_atomic(backup.join("marker"), b"old")
            .unwrap();
        state
            .files
            .write_atomic(staging.join("partial"), b"x")
            .unwrap();
        let mut upgraded = instance.clone();
        upgraded.pack_version_id = Some("new-pack".to_string());
        state
            .db
            .set_modpack_version(&upgraded, "new-launch")
            .unwrap();
        let journal = state.paths.modpack_upgrade_journal(&instance.id).unwrap();
        state
            .files
            .write_atomic(
                &journal,
                &serde_json::to_vec_pretty(&UpgradeJournal {
                    schema_version: 1,
                    instance: instance.clone(),
                    content: Vec::new(),
                    nonce,
                })
                .unwrap(),
            )
            .unwrap();

        recover_interrupted_upgrades(&state).unwrap();

        assert_eq!(state.files.read(live.join("marker")).unwrap(), b"old");
        assert!(!state.files.exists(staging).unwrap());
        assert!(!state.files.exists(journal).unwrap());
        let restored = state
            .db
            .list_instances(&state.files)
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == instance.id)
            .unwrap();
        assert_eq!(restored.pack_version_id.as_deref(), Some("old-pack"));
        std::fs::remove_dir_all(root).ok();
    }
}
