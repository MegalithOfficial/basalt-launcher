use tauri::{AppHandle, State};

use crate::{
    config::Instance,
    error::{Error, Result},
    install,
    java::{self, JavaStatus},
    loaders,
    meta::{
        banners,
        manifest::{self, VersionEntry},
        media::{self, VersionMedia},
    },
    search,
    state::AppState,
};

use super::find_instance;

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn list_instances(state: State<AppState>) -> Result<Vec<Instance>> {
    state.db.list_instances(&state.files)
}

#[tauri::command]
pub fn get_instance_launch_command(state: State<AppState>, instance_id: String) -> Result<String> {
    crate::cli::launch_command(&state, &instance_id)
}

#[tauri::command]
pub fn get_instance_organization(
    state: State<AppState>,
) -> Result<crate::db::InstanceOrganization> {
    state.db.instance_organization()
}

#[tauri::command]
pub fn create_instance_group(
    state: State<AppState>,
    name: String,
) -> Result<crate::db::InstanceGroup> {
    state.db.create_instance_group(&name)
}

#[tauri::command]
pub fn rename_instance_group(
    state: State<AppState>,
    group_id: String,
    name: String,
) -> Result<crate::db::InstanceGroup> {
    state.db.rename_instance_group(&group_id, &name)
}

#[tauri::command]
pub fn delete_instance_group(state: State<AppState>, group_id: String) -> Result<()> {
    state.db.delete_instance_group(&group_id)
}

#[tauri::command]
pub fn move_instance_to_group(
    state: State<AppState>,
    instance_id: String,
    group_id: Option<String>,
) -> Result<()> {
    state
        .db
        .move_instance_to_group(&instance_id, group_id.as_deref())
}

#[tauri::command]
pub fn reorder_instance_groups(state: State<AppState>, group_ids: Vec<String>) -> Result<()> {
    state.db.reorder_instance_groups(&group_ids)
}

#[tauri::command]
pub fn reorder_group_instances(
    state: State<AppState>,
    group_id: Option<String>,
    instance_ids: Vec<String>,
) -> Result<()> {
    state
        .db
        .reorder_group_instances(group_id.as_deref(), &instance_ids)
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
        import_source: None,
        import_source_id: None,
        banner_id: None,
        notes: None,
        wrapper_command: None,
        pre_launch_command: None,
        post_exit_command: None,
        jvm_args: None,
        jvm_args_mode: None,
        env_vars: None,
        env_vars_mode: None,
        pack_provider: None,
        pack_project_id: None,
        pack_version_id: None,
    };
    state
        .files
        .ensure_dir(state.paths.instance_dir(&instance.id))?;
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
#[allow(clippy::too_many_arguments)]
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
    jvm_args: Option<String>,
    jvm_args_mode: Option<String>,
    env_vars: Option<String>,
    env_vars_mode: Option<String>,
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
    if needs_reset && existing.pack_project_id.is_some() {
        return Err(Error::other(
            "This instance follows a modpack, which decides its game version and loader. Unlink it first.",
        ));
    }
    state.db.update_instance_settings(
        &instance_id,
        &name,
        min_memory_mb,
        max_memory_mb,
        java_path,
        loader,
        loader_version,
        &version_id,
        jvm_args
            .map(|value| value.trim().to_string())
            .filter(|v| !v.is_empty()),
        jvm_args_mode,
        env_vars
            .map(|value| value.trim().to_string())
            .filter(|v| !v.is_empty()),
        env_vars_mode,
        needs_reset,
    )?;
    if existing.version_id != version_id {
        state.media_cache.lock().unwrap().remove(&instance_id);
    }
    tracing::info!(needs_reset, "instance updated");
    find_instance(&state, &instance_id)
}

#[tauri::command]
#[tracing::instrument(skip(state, wrapper, pre_launch, post_exit), err)]
pub fn set_instance_launch_tools(
    state: State<AppState>,
    instance_id: String,
    wrapper: String,
    pre_launch: String,
    post_exit: String,
) -> Result<()> {
    let kept = |value: &str| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    };
    state.db.set_instance_launch_tools(
        &instance_id,
        kept(&wrapper).as_deref(),
        kept(&pre_launch).as_deref(),
        kept(&post_exit).as_deref(),
    )
}

#[tauri::command]
#[tracing::instrument(skip(state, notes), err)]
pub fn set_instance_notes(
    state: State<AppState>,
    instance_id: String,
    notes: String,
) -> Result<()> {
    let trimmed = notes.trim();
    state
        .db
        .set_instance_notes(&instance_id, (!trimmed.is_empty()).then_some(trimmed))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn delete_instance(state: State<'_, AppState>, instance_id: String) -> Result<()> {
    crate::snapshots::ensure_no_pending_restore(&state, &instance_id)?;
    state.db.delete_instance(&instance_id)?;
    state.files.remove_instance_dir(&instance_id)?;
    crate::snapshots::delete_instance_data(&state, &instance_id).await?;
    media::clear_custom_banner(&state.files, &instance_id).await;
    state.media_cache.lock().unwrap().remove(&instance_id);
    state.db.delete_instance_content_files(&instance_id)?;
    tracing::info!("instance deleted");
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn repair_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<crate::instance_ops::RepairReport> {
    let instance = find_instance(&state, &instance_id)?;
    crate::instance_ops::repair(&app, &state, instance).await
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn duplicate_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Instance> {
    let instance = find_instance(&state, &instance_id)?;
    crate::instance_ops::duplicate(&app, &state, instance).await
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

    let result = match banners::media_for_instance(&state.files, &state.db, &instance_id) {
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
    let entry = banners::import(&state.files, &state.db, &source_path).await?;
    state
        .db
        .set_instance_banner_id(&instance_id, Some(&entry.id))?;
    let media = banners::media_for_instance(&state.files, &state.db, &instance_id)
        .ok_or_else(|| crate::error::Error::other("the banner vanished after import"))?;
    state
        .media_cache
        .lock()
        .unwrap()
        .insert(instance_id, Some(media.clone()));
    Ok(media)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn list_banner_library(state: State<'_, AppState>) -> Result<Vec<banners::BannerEntry>> {
    banners::list(&state.files, &state.db)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn add_banner_to_library(
    state: State<'_, AppState>,
    source_path: String,
) -> Result<banners::BannerEntry> {
    banners::import(&state.files, &state.db, &source_path).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn delete_banner(state: State<'_, AppState>, banner_id: String) -> Result<()> {
    let affected = state.db.banner_users(&banner_id).unwrap_or_default();
    banners::remove(&state.files, &state.db, &banner_id).await?;
    if !affected.is_empty() {
        state.media_cache.lock().unwrap().clear();
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn apply_banner(
    state: State<'_, AppState>,
    instance_id: String,
    banner_id: String,
) -> Result<VersionMedia> {
    find_instance(&state, &instance_id)?;
    state
        .db
        .set_instance_banner_id(&instance_id, Some(&banner_id))?;
    let media = banners::media_for_instance(&state.files, &state.db, &instance_id)
        .ok_or_else(|| crate::error::Error::other("that banner is no longer in the library"))?;
    state
        .media_cache
        .lock()
        .unwrap()
        .insert(instance_id, Some(media.clone()));
    Ok(media)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn clear_instance_banner(state: State<'_, AppState>, instance_id: String) -> Result<()> {
    state.db.set_instance_banner_id(&instance_id, None)?;
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
    let _ = banners::import(&state.files, &state.db, &source_path).await;
    media::set_instance_logo(&state.files, &instance_id, &source_path).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn apply_logo(
    state: State<'_, AppState>,
    instance_id: String,
    banner_id: String,
) -> Result<String> {
    find_instance(&state, &instance_id)?;
    let record = state
        .db
        .banner(&banner_id)?
        .ok_or_else(|| Error::other("that image is no longer in the library"))?;
    if record.kind != "image" {
        return Err(Error::other("A logo has to be an image."));
    }
    let path = banners::library_path(&state.files, &record);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png")
        .to_string();
    let bytes = state.files.read_async(&path).await?;
    media::write_logo(&state.files, &instance_id, &extension, &bytes).await
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
        let (Some(provider), Some(project_id)) = (
            instance.pack_provider.as_deref(),
            instance.pack_project_id.as_deref(),
        ) else {
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

fn version_jar_exists(state: &AppState, id: &str, depth: u8) -> bool {
    if state
        .files
        .is_file(state.paths.version_jar(id))
        .unwrap_or(false)
    {
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
        let id = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
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
    )?;

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
    let found = java::find_for_major(&state.files, required_major, explicit.as_deref()).await;
    let ok = found.as_ref().is_some_and(|j| j.major >= required_major);

    Ok(JavaStatus {
        required_major,
        found,
        ok,
    })
}
