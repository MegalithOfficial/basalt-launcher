use std::time::Duration;

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, State};

use crate::auth::account::{Account, AccountView};
use crate::auth::microsoft::{self, PollOutcome};
use crate::config::{Instance, LauncherSettings};
use crate::content::{self, ContentItem};
use crate::db::Db;
use crate::error::{Error, Result};
use crate::install;
use crate::java::{self, JavaStatus};
use crate::launch::{self, process::{LogLine, RunningInfo}};
use crate::loaders;
use crate::logging::{self, LogConfig, LogRecord, LogState};
use crate::meta::manifest::{self, VersionEntry};
use crate::meta::media::{self, VersionMedia};
use crate::search;
use crate::skin::{self, Appearance, SkinEntry};
use crate::state::AppState;
use crate::sysinfo_probe::{self, SystemStats, SystemUsage};
use crate::update::{self, UpdateInfo};

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_settings(state: State<AppState>) -> Result<LauncherSettings> {
    state.db.load_settings()
}

#[derive(Serialize)]
pub struct AppInfo {
    pub version: String,
    pub build_channel: String,
    pub data_dir: String,
    pub default_jvm_args: String,
    pub jvm_placeholders: Vec<String>,
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_app_info(state: State<AppState>) -> Result<AppInfo> {
    Ok(AppInfo {
        version: crate::build_info::VERSION.to_string(),
        build_channel: crate::build_info::CHANNEL.to_string(),
        data_dir: state.paths.root.display().to_string(),
        default_jvm_args: crate::config::DEFAULT_JVM_ARGS.to_string(),
        jvm_placeholders: crate::launch::PLACEHOLDERS
            .iter()
            .map(|p| p.to_string())
            .collect(),
    })
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn list_javas() -> Result<Vec<java::JavaInfo>> {
    Ok(java::list_all().await)
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn update_settings(state: State<AppState>, settings: LauncherSettings) -> Result<()> {
    if logging::normalize_level(&settings.log_level) != logging::current_level() {
        logging::set_level(&settings.log_level)?;
    }
    state.db.save_settings(&settings)
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn list_instances(state: State<AppState>) -> Result<Vec<Instance>> {
    state.db.list_instances(&state.files)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn create_instance(
    state: State<AppState>,
    name: String,
    version_id: String,
    loader: Option<String>,
    loader_version: Option<String>,
) -> Result<Instance> {
    if let Some(loader) = loader.as_deref() {
        loaders::Loader::parse(loader)?;
        if loader_version.is_none() {
            return Err(Error::other("loader version is required"));
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    let instance = Instance {
        dir: state.paths.instance_dir(&id).display().to_string(),
        logo: None,
        id,
        name,
        version_id,
        created_at: chrono::Utc::now(),
        min_memory_mb: None,
        max_memory_mb: None,
        java_path: None,
        last_played_at: None,
        playtime_secs: 0,
        loader,
        loader_version,
        launch_version_id: None,
        pack_provider: None,
        pack_project_id: None,
        pack_version_id: None,
    };
    state.files.ensure_dir(state.paths.instance_dir(&instance.id))?;
    state.db.insert_instance(&instance)?;
    tracing::info!(
        instance_id = %instance.id,
        name = %instance.name,
        version_id = %instance.version_id,
        loader = ?instance.loader,
        "instance created"
    );
    Ok(instance)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn list_loader_versions(
    state: State<'_, AppState>,
    loader: String,
    game_version: String,
) -> Result<Vec<String>> {
    let loader = loaders::Loader::parse(&loader)?;
    loaders::list_loader_versions(&state.network, loader, &game_version).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn update_instance(
    state: State<AppState>,
    instance_id: String,
    name: String,
    min_memory_mb: Option<u32>,
    max_memory_mb: Option<u32>,
    java_path: Option<String>,
    loader: Option<String>,
    loader_version: Option<String>,
    version_id: String,
) -> Result<Instance> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(Error::other("Instance name cannot be empty."));
    }
    if version_id.trim().is_empty() {
        return Err(Error::other("Game version cannot be empty."));
    }
    if let Some(loader) = loader.as_deref() {
        loaders::Loader::parse(loader)?;
        if loader_version.is_none() {
            return Err(Error::other("loader version is required"));
        }
    }
    let existing = find_instance(&state, &instance_id)?;
    let needs_reset = existing.loader != loader
        || existing.loader_version != loader_version
        || existing.version_id != version_id;
    state.db.update_instance_settings(
        &instance_id,
        &name,
        min_memory_mb,
        max_memory_mb,
        java_path,
        loader,
        loader_version,
        &version_id,
        needs_reset,
    )?;
    if existing.version_id != version_id {
        state.media_cache.lock().unwrap().remove(&instance_id);
    }
    tracing::info!(needs_reset, "instance updated");
    find_instance(&state, &instance_id)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn delete_instance(state: State<'_, AppState>, instance_id: String) -> Result<()> {
    state.db.delete_instance(&instance_id)?;
    state.files.remove_instance_dir(&instance_id)?;
    media::clear_custom_banner(&state.files, &instance_id).await;
    state.media_cache.lock().unwrap().remove(&instance_id);
    state.db.delete_instance_content_files(&instance_id)?;
    tracing::info!("instance deleted");
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn get_instance_media(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Option<VersionMedia>> {
    if let Some(cached) = state.media_cache.lock().unwrap().get(&instance_id) {
        return Ok(cached.clone());
    }

    let result = match media::custom_banner(&state.files, &instance_id).await {
        Some(banner) => Some(banner),
        None => {
            let instance = find_instance(&state, &instance_id)?;
            let notes = {
                let cached = state.patch_notes.lock().unwrap().clone();
                match cached {
                    Some(notes) => notes,
                    None => {
                        let notes = media::fetch_notes(&state.network, &state.files).await?;
                        *state.patch_notes.lock().unwrap() = Some(notes.clone());
                        notes
                    }
                }
            };
            media::media_for(&state.network, &state.files, &notes, &instance.version_id).await
        }
    };

    state
        .media_cache
        .lock()
        .unwrap()
        .insert(instance_id, result.clone());
    Ok(result)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn set_instance_banner(
    state: State<'_, AppState>,
    instance_id: String,
    source_path: String,
) -> Result<VersionMedia> {
    find_instance(&state, &instance_id)?;
    let media = media::set_custom_banner(&state.files, &instance_id, &source_path).await?;
    state
        .media_cache
        .lock()
        .unwrap()
        .insert(instance_id, Some(media.clone()));
    Ok(media)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn clear_instance_banner(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<()> {
    media::clear_custom_banner(&state.files, &instance_id).await;
    state.media_cache.lock().unwrap().remove(&instance_id);
    Ok(())
}

#[tauri::command]
pub async fn set_instance_logo(
    state: State<'_, AppState>,
    instance_id: String,
    source_path: String,
) -> Result<String> {
    find_instance(&state, &instance_id)?;
    media::set_instance_logo(&state.files, &instance_id, &source_path).await
}

#[tauri::command]
pub async fn clear_instance_logo(state: State<'_, AppState>, instance_id: String) -> Result<()> {
    media::clear_instance_logo(&state.files, &instance_id).await;
    Ok(())
}

#[tauri::command]
pub async fn backfill_pack_logos(state: State<'_, AppState>) -> Result<Vec<Instance>> {
    let instances = state.db.list_instances(&state.files)?;

    for instance in &instances {
        if instance.logo.is_some() {
            continue;
        }
        let (Some(provider), Some(project_id)) =
            (instance.pack_provider.as_deref(), instance.pack_project_id.as_deref())
        else {
            continue;
        };
        let Ok(provider) = search::Provider::parse(provider) else {
            continue;
        };
        let icon = search::resolve_projects(&state, provider, &[project_id.to_string()])
            .await
            .ok()
            .and_then(|mut list| list.pop())
            .and_then(|summary| summary.icon_url);
        if let Some(icon) = icon {
            media::fetch_instance_logo(&state.network, &state.files, &instance.id, &icon).await;
        }
    }

    state.db.list_instances(&state.files)
}

#[tauri::command]
pub fn list_tasks(state: State<AppState>) -> Vec<crate::tasks::Task> {
    state.tasks.list()
}

#[tauri::command]
pub fn clear_finished_tasks(state: State<AppState>) {
    state.tasks.clear_finished();
}

#[tauri::command]
pub fn cancel_task(state: State<AppState>, task_id: String) -> bool {
    state.tasks.cancel(&task_id)
}

fn sweep_partials(files: &crate::files::FileManager, dir: &std::path::Path) -> usize {
    let Ok(entries) = files.read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for path in entries {
        if files.metadata(&path).is_ok_and(|metadata| metadata.is_dir()) {
            removed += sweep_partials(files, &path);
        } else if path.extension().is_some_and(|e| e == "part") {
            if files.remove_file_if_exists(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

#[tauri::command]
pub fn recover_interrupted(state: State<AppState>) -> Result<Vec<crate::db::PendingOperation>> {
    let pending = state.db.pending_operations()?;
    let removed_partials = sweep_partials(&state.files, &state.paths.root);
    if removed_partials > 0 {
        tracing::info!(removed_partials, "removed interrupted download files");
    }
    if pending.is_empty() {
        return Ok(Vec::new());
    }

    for op in &pending {
        let Some(instance_id) = op.instance_id.as_deref() else {
            continue;
        };
        if op.kind == "ModpackInstall" {
            let _ = state.db.delete_instance_content_files(instance_id);
            let _ = state.db.delete_instance(instance_id);
            let _ = state.files.remove_instance_dir(instance_id);
        }
    }

    state.db.clear_pending_operations()?;
    Ok(pending)
}

fn version_jar_exists(state: &AppState, id: &str, depth: u8) -> bool {
    if state.files.is_file(state.paths.version_jar(id)).unwrap_or(false) {
        return true;
    }
    if depth == 0 {
        return false;
    }
    let Ok(bytes) = state.files.read(state.paths.version_json(id)) else {
        return false;
    };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    let next = json
        .get("jar")
        .or_else(|| json.get("inheritsFrom"))
        .and_then(|v| v.as_str());
    match next {
        Some(next_id) if next_id != id => version_jar_exists(state, next_id, depth - 1),
        _ => false,
    }
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn list_installed_versions(state: State<AppState>) -> Result<Vec<String>> {
    let mut installed = Vec::new();
    let entries = match state.files.read_dir(state.paths.versions()) {
        Ok(entries) => entries,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(installed)
        }
        Err(error) => return Err(error),
    };
    for path in entries {
        let id = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if state
            .files
            .is_file(state.paths.version_json(&id))
            .unwrap_or(false)
            && version_jar_exists(&state, &id, 3)
        {
            installed.push(id);
        }
    }
    Ok(installed)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn list_versions(
    state: State<'_, AppState>,
    include_snapshots: bool,
) -> Result<Vec<VersionEntry>> {
    let manifest = manifest::fetch(&state.network, &state.files).await?;
    let versions = manifest
        .versions
        .into_iter()
        .filter(|v| include_snapshots || v.kind == "release")
        .collect();
    Ok(versions)
}

fn find_instance(state: &AppState, instance_id: &str) -> Result<Instance> {
    state
        .db
        .list_instances(&state.files)?
        .into_iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| Error::NotFound(format!("instance {instance_id}")))
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn install_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<()> {
    let instance = find_instance(&state, &instance_id)?;
    let task = state.tasks.start(
        &app,
        crate::tasks::TaskKind::GameInstall,
        crate::tasks::TaskSpec {
            title: instance.name.clone(),
            subtitle: Some(format!(
                "{}{}",
                instance.version_id,
                instance
                    .loader
                    .as_deref()
                    .map(|l| format!(" · {l}"))
                    .unwrap_or_default()
            )),
            instance_id: Some(instance.id.clone()),
            ..Default::default()
        },
    );

    let result = async {
        let launch_id = match (&instance.loader, &instance.launch_version_id) {
            (Some(_), None) => {
                let id = loaders::install_loader(&app, &state, &instance, &task).await?;
                state.db.set_launch_version(&instance.id, &id)?;
                id
            }
            (_, Some(id)) => id.clone(),
            (None, None) => instance.version_id.clone(),
        };
        install::install_version(&app, &state, &instance.id, &launch_id, &task).await
    }
    .await;

    task.finish(&result);
    result
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn get_java_status(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<JavaStatus> {
    let instance = find_instance(&state, &instance_id)?;
    let version = install::load_version_json(&state, &instance.version_id).await?;
    let required_major = version.required_java_major();

    let explicit = instance
        .java_path
        .clone()
        .or_else(|| state.db.load_settings().ok().and_then(|s| s.java_path));
    let found = java::find_for_major(required_major, explicit.as_deref()).await;
    let ok = found.as_ref().map_or(false, |j| j.major >= required_major);

    Ok(JavaStatus {
        required_major,
        found,
        ok,
    })
}

#[tauri::command]
pub async fn list_instance_content(
    state: State<'_, AppState>,
    instance_id: String,
    kind: String,
    reconcile: Option<bool>,
) -> Result<Vec<ContentItem>> {
    find_instance(&state, &instance_id)?;
    if reconcile.unwrap_or(false) {
        let _ = search::identify::reconcile(&state, &instance_id, &kind).await;
    }

    let mut items = content::list(&state.files, &instance_id, &kind)?;
    let mut sources: std::collections::HashMap<String, crate::db::ContentFile> = state
        .db
        .content_files(&instance_id, &kind)?
        .into_iter()
        .map(|f| (f.file_name.clone(), f))
        .collect();
    let mut updates: std::collections::HashMap<String, crate::db::ContentUpdate> = state
        .db
        .content_updates(&instance_id)?
        .into_iter()
        .filter(|u| u.kind == kind)
        .map(|u| (u.file_name.clone(), u))
        .collect();

    for item in &mut items {
        item.source = sources.remove(&item.file_name);
        item.update = updates.remove(&item.file_name);
    }
    Ok(items)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn toggle_instance_content(
    state: State<AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
) -> Result<bool> {
    let enabled = content::toggle(&state.files, &instance_id, &kind, &file_name)?;
    tracing::info!(enabled, "content toggled");
    Ok(enabled)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_instance_content(
    state: State<AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
) -> Result<()> {
    content::delete(&state.files, &instance_id, &kind, &file_name)?;
    tracing::info!("content deleted");
    state.db.delete_content_file(&instance_id, &kind, &file_name)
}

#[tauri::command]
pub fn get_content_dependents(
    state: State<AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
) -> Result<Vec<String>> {
    let kind_enum = search::ContentKind::parse(&kind)?;
    let Some(file) = state.db.content_file(&instance_id, &kind, &file_name)? else {
        return Ok(Vec::new());
    };
    let Some(project_id) = file.project_id else {
        return Ok(Vec::new());
    };
    Ok(search::resolve::dependents_of(
        &state,
        &instance_id,
        kind_enum,
        &project_id,
    ))
}

#[tauri::command]
pub async fn add_instance_content(
    state: State<'_, AppState>,
    instance_id: String,
    kind: String,
    sources: Vec<String>,
) -> Result<usize> {
    find_instance(&state, &instance_id)?;
    let copied = content::add(&state.files, &instance_id, &kind, &sources)?;
    let _ = search::identify::reconcile(&state, &instance_id, &kind).await;
    tracing::info!(copied, offered = sources.len(), "content added from disk");
    Ok(copied)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn search_content(
    state: State<'_, AppState>,
    provider: String,
    kind: String,
    query: search::SearchQuery,
) -> Result<search::SearchPage> {
    let provider = search::Provider::parse(&provider)?;
    let kind = search::ContentKind::parse(&kind)?;
    search::search(&state, provider, kind, &query).await
}

#[tauri::command]
pub async fn get_filter_taxonomy(
    state: State<'_, AppState>,
    provider: String,
    kind: String,
    include_snapshots: Option<bool>,
) -> Result<search::FilterTaxonomy> {
    let provider = search::Provider::parse(&provider)?;
    let kind = search::ContentKind::parse(&kind)?;
    search::taxonomy(&state, provider, kind, include_snapshots.unwrap_or(false)).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn get_project_details(
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
) -> Result<search::ProjectDetails> {
    let provider = search::Provider::parse(&provider)?;
    search::project_details(&state, provider, &project_id).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn list_project_versions(
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    kind: String,
    game_version: String,
    loader: Option<String>,
) -> Result<Vec<search::ProjectVersion>> {
    let provider = search::Provider::parse(&provider)?;
    let kind = search::ContentKind::parse(&kind)?;
    search::project_versions(
        &state,
        provider,
        &project_id,
        kind,
        &game_version,
        loader.as_deref(),
    )
    .await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn resolve_projects(
    state: State<'_, AppState>,
    provider: String,
    ids: Vec<String>,
) -> Result<Vec<search::ProjectSummary>> {
    let provider = search::Provider::parse(&provider)?;
    search::resolve_projects(&state, provider, &ids).await
}

#[derive(Serialize)]
pub struct InstalledFile {
    pub version_id: Option<String>,
    pub file_name: String,
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn get_installed_project_file(
    state: State<AppState>,
    instance_id: String,
    kind: String,
    project_id: String,
) -> Result<Option<InstalledFile>> {
    let result = state
        .db
        .installed_project_file(&instance_id, &kind, &project_id)?;
    Ok(result.map(|(version_id, file_name)| InstalledFile {
        version_id,
        file_name,
    }))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn get_version_changelog(
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    version_id: String,
) -> Result<search::Changelog> {
    let provider = search::Provider::parse(&provider)?;
    search::version_changelog(&state, provider, &project_id, &version_id).await
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn plan_content_install(
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    instance_id: String,
    kind: String,
    game_version: String,
    loader: Option<String>,
    version_id: Option<String>,
) -> Result<search::resolve::InstallPlan> {
    find_instance(&state, &instance_id)?;
    let provider = search::Provider::parse(&provider)?;
    let kind = search::ContentKind::parse(&kind)?;
    search::resolve::plan(
        &state,
        provider,
        &project_id,
        &instance_id,
        kind,
        &game_version,
        loader.as_deref(),
        version_id.as_deref(),
        true,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn install_content(
    app: AppHandle,
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    instance_id: String,
    kind: String,
    game_version: String,
    loader: Option<String>,
    version_id: Option<String>,
    with_dependencies: Option<bool>,
) -> Result<Vec<String>> {
    find_instance(&state, &instance_id)?;
    let provider = search::Provider::parse(&provider)?;
    let kind = search::ContentKind::parse(&kind)?;
    let plan = search::resolve::plan(
        &state,
        provider,
        &project_id,
        &instance_id,
        kind,
        &game_version,
        loader.as_deref(),
        version_id.as_deref(),
        with_dependencies.unwrap_or(true),
    )
    .await?;
    search::resolve::apply(
        Some(&app),
        &state,
        &plan,
        provider,
        &instance_id,
        kind,
        None,
    )
    .await
}

#[tauri::command]
pub async fn check_content_updates(
    state: State<'_, AppState>,
    instance_id: String,
    force: Option<bool>,
) -> Result<Vec<crate::db::ContentUpdate>> {
    let instance = find_instance(&state, &instance_id)?;
    let checked_at = state.db.updates_checked_at(&instance_id)?;
    if !force.unwrap_or(false) && !search::updates::is_stale(checked_at) {
        return state.db.content_updates(&instance_id);
    }
    search::updates::check(
        &state,
        &instance_id,
        &instance.version_id,
        instance.loader.as_deref(),
    )
    .await
}

#[tauri::command]
pub fn get_content_updates(
    state: State<AppState>,
    instance_id: String,
) -> Result<Vec<crate::db::ContentUpdate>> {
    state.db.content_updates(&instance_id)
}

#[tauri::command]
pub async fn apply_content_update(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
) -> Result<String> {
    let instance = find_instance(&state, &instance_id)?;
    let kind_enum = search::ContentKind::parse(&kind)?;
    let file = state
        .db
        .content_file(&instance_id, &kind, &file_name)?
        .ok_or_else(|| Error::other("This file is not linked to a project."))?;
    let (Some(provider), Some(project_id)) = (file.provider.clone(), file.project_id.clone())
    else {
        return Err(Error::other("This file is not linked to a project."));
    };
    let update = state
        .db
        .content_updates(&instance_id)?
        .into_iter()
        .find(|u| u.kind == kind && u.file_name == file_name)
        .ok_or_else(|| Error::other("No update is available for this file."))?;

    let provider = search::Provider::parse(&provider)?;
    let plan = search::resolve::plan(
        &state,
        provider,
        &project_id,
        &instance_id,
        kind_enum,
        &instance.version_id,
        instance.loader.as_deref(),
        Some(&update.latest_version_id),
        true,
    )
    .await?;
    let written = search::resolve::apply(
        Some(&app),
        &state,
        &plan,
        provider,
        &instance_id,
        kind_enum,
        None,
    )
    .await?;

    state
        .db
        .delete_content_file(&instance_id, &kind, &file_name)
        .ok();
    Ok(written.into_iter().next().unwrap_or(file_name))
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn install_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    version_id: String,
) -> Result<Instance> {
    let provider = search::Provider::parse(&provider)?;
    crate::modpack::install_modpack(&app, &state, provider, &project_id, &version_id).await
}

#[derive(Serialize)]
pub struct DeviceCodeInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub message: String,
}

async fn run_auth_flow(
    network: std::sync::Arc<crate::network::NetworkManager>,
    db: Db,
    device_code: String,
    interval: u64,
) -> Result<AccountView> {
    let mut interval = interval.max(1);
    let token = loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        match microsoft::poll_token(&network, &device_code).await? {
            PollOutcome::Pending => continue,
            PollOutcome::SlowDown => {
                interval += 5;
                continue;
            }
            PollOutcome::Token(token) => break token,
        }
    };

    let mc = microsoft::authenticate_minecraft(&network, &token.access_token).await?;
    let account = Account {
        id: mc.uuid.clone(),
        name: mc.name,
        mc_access_token: mc.access_token,
        refresh_token: token.refresh_token,
        expires_at: chrono::Utc::now().timestamp() + mc.expires_in,
    };

    tracing::info!(account = %account.name, uuid = %account.id, "microsoft sign-in completed");
    let mut store = db.load_accounts()?;
    store.upsert_active(account);
    db.save_accounts(&store)?;

    store
        .views()
        .into_iter()
        .find(|v| v.id == mc.uuid)
        .ok_or_else(|| Error::other("account vanished after save"))
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn auth_begin(app: AppHandle, state: State<'_, AppState>) -> Result<DeviceCodeInfo> {
    let device = microsoft::request_device_code(&state.network).await?;
    tracing::info!(
        verification_uri = %device.verification_uri,
        interval = device.interval,
        "device code issued"
    );
    let info = DeviceCodeInfo {
        user_code: device.user_code.clone(),
        verification_uri: device.verification_uri.clone(),
        message: device.message.clone(),
    };

    let network = state.network.clone();
    let db = state.db.clone();
    let device_code = device.device_code.clone();
    let interval = device.interval;

    tokio::spawn(async move {
        match run_auth_flow(network, db, device_code, interval).await {
            Ok(view) => {
                let _ = app.emit("auth:state", json!({ "status": "success", "account": view }));
            }
            Err(e) => {
                tracing::error!(error = %e, "microsoft sign-in failed");
                let _ = app.emit("auth:state", json!({ "status": "error", "message": e.to_string() }));
            }
        }
    });

    Ok(info)
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn list_accounts(state: State<AppState>) -> Result<Vec<AccountView>> {
    Ok(state.db.load_accounts()?.views())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn set_active_account(state: State<AppState>, account_id: String) -> Result<()> {
    let mut store = state.db.load_accounts()?;
    if store.accounts.iter().any(|a| a.id == account_id) {
        store.active_id = Some(account_id);
        state.db.save_accounts(&store)?;
        tracing::info!("active account changed");
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn remove_account(state: State<AppState>, account_id: String) -> Result<()> {
    let mut store = state.db.load_accounts()?;
    store.accounts.retain(|a| a.id != account_id);
    if store.active_id.as_deref() == Some(account_id.as_str()) {
        store.active_id = store.accounts.first().map(|a| a.id.clone());
    }
    state.db.save_accounts(&store)?;
    tracing::info!(remaining = store.accounts.len(), "account removed");
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<String> {
    let instance = find_instance(&state, &instance_id)?;
    launch::launch_instance(&app, &state, &instance).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn kill_instance(state: State<AppState>, running_id: String) -> Result<()> {
    let mut registry = state.running.lock().unwrap();
    if let Some(handle) = registry.get_mut(&running_id) {
        if let Some(tx) = handle.kill_tx.take() {
            tracing::info!(pid = handle.pid, "kill requested");
            let _ = tx.send(());
        }
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn list_running(state: State<AppState>) -> Result<Vec<RunningInfo>> {
    let registry = state.running.lock().unwrap();
    Ok(registry.iter().map(|(id, handle)| handle.info(id)).collect())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn get_logs(state: State<AppState>, running_id: String) -> Result<Vec<LogLine>> {
    let registry = state.running.lock().unwrap();
    Ok(registry
        .get(&running_id)
        .map(|handle| handle.logs.lock().unwrap().clone())
        .unwrap_or_default())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn close_running(state: State<AppState>, running_id: String) -> Result<()> {
    state.running.lock().unwrap().remove(&running_id);
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(logs), err)]
pub fn get_log_records(logs: State<LogState>, limit: Option<usize>) -> Result<Vec<LogRecord>> {
    Ok(logs.buffer.snapshot(limit.unwrap_or(2000)))
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn clear_log_records(logs: State<LogState>) -> Result<()> {
    logs.buffer.clear();
    tracing::info!("log view cleared");
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_log_config(state: State<AppState>) -> Result<LogConfig> {
    Ok(logging::config(&state.paths))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn set_log_level(state: State<AppState>, level: String) -> Result<LogConfig> {
    logging::set_level(&level)?;
    let mut settings = state.db.load_settings()?;
    settings.log_level = logging::normalize_level(&level).to_string();
    state.db.save_settings(&settings)?;
    Ok(logging::config(&state.paths))
}

#[tauri::command]
pub fn frontend_log(
    level: String,
    scope: String,
    message: String,
    data: Option<String>,
) -> Result<()> {
    logging::record_frontend(&level, &scope, &message, data.as_deref());
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn check_for_updates(state: State<'_, AppState>) -> Result<UpdateInfo> {
    update::check(&state.network).await
}

#[derive(Serialize)]
pub struct AboutLinks {
    pub repository: String,
    pub issues: String,
    pub releases: String,
}

#[tauri::command]
pub fn get_about_links() -> AboutLinks {
    AboutLinks {
        repository: update::REPO_URL.to_string(),
        issues: format!("{}/issues/new", update::REPO_URL),
        releases: format!("{}/releases", update::REPO_URL),
    }
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn get_appearance(state: State<'_, AppState>) -> Result<Appearance> {
    skin::appearance(&state).await
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn list_skins(state: State<AppState>) -> Result<Vec<SkinEntry>> {
    skin::library(&state)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn add_skin_from_file(
    state: State<AppState>,
    path: String,
    name: Option<String>,
    variant: String,
) -> Result<SkinEntry> {
    skin::add_from_file(&state, &path, name.as_deref(), &variant)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn add_skin_from_reference(
    state: State<'_, AppState>,
    reference: String,
) -> Result<SkinEntry> {
    skin::add_from_reference(&state, &reference).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_skin(state: State<AppState>, skin_id: String) -> Result<()> {
    skin::remove(&state, &skin_id)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn apply_saved_skin(
    state: State<'_, AppState>,
    skin_id: String,
    variant: Option<String>,
) -> Result<Appearance> {
    skin::apply_saved(&state, &skin_id, variant.as_deref()).await
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn reset_skin(state: State<'_, AppState>) -> Result<Appearance> {
    skin::reset(&state).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn set_cape(state: State<'_, AppState>, cape_id: Option<String>) -> Result<Appearance> {
    skin::set_cape(&state, cape_id.as_deref()).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn rename_skin(state: State<AppState>, skin_id: String, name: String) -> Result<SkinEntry> {
    skin::rename(&state, &skin_id, &name)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn get_worn_skin(state: State<AppState>, uuid: String) -> Result<Option<SkinEntry>> {
    skin::worn_skin(&state, &uuid)
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_system_stats(state: State<AppState>) -> Result<SystemStats> {
    Ok(sysinfo_probe::collect(&state.paths))
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn preview_launch_args(
    state: State<AppState>,
    settings: LauncherSettings,
) -> Result<launch::LaunchPreview> {
    Ok(launch::preview(&state.paths, &settings))
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_system_usage(state: State<AppState>) -> Result<SystemUsage> {
    Ok(sysinfo_probe::usage(&state.paths))
}
