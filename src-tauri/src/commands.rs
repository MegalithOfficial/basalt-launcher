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
use crate::meta::manifest::{self, VersionEntry};
use crate::meta::media::{self, VersionMedia};
use crate::search;
use crate::state::AppState;

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<LauncherSettings> {
    state.db.load_settings()
}

#[tauri::command]
pub fn update_settings(state: State<AppState>, settings: LauncherSettings) -> Result<()> {
    state.db.save_settings(&settings)
}

#[tauri::command]
pub fn list_instances(state: State<AppState>) -> Result<Vec<Instance>> {
    state.db.list_instances(&state.paths)
}

#[tauri::command]
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
    };
    std::fs::create_dir_all(state.paths.instance_dir(&instance.id))?;
    state.db.insert_instance(&instance)?;
    Ok(instance)
}

#[tauri::command]
pub async fn list_loader_versions(
    state: State<'_, AppState>,
    loader: String,
    game_version: String,
) -> Result<Vec<String>> {
    let loader = loaders::Loader::parse(&loader)?;
    loaders::list_loader_versions(&state.http, loader, &game_version).await
}

#[tauri::command]
pub fn update_instance(
    state: State<AppState>,
    instance_id: String,
    name: String,
    min_memory_mb: Option<u32>,
    max_memory_mb: Option<u32>,
    java_path: Option<String>,
) -> Result<Instance> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(Error::other("Instance name cannot be empty."));
    }
    state
        .db
        .update_instance_settings(&instance_id, &name, min_memory_mb, max_memory_mb, java_path)?;
    find_instance(&state, &instance_id)
}

#[tauri::command]
pub async fn delete_instance(state: State<'_, AppState>, instance_id: String) -> Result<()> {
    state.db.delete_instance(&instance_id)?;
    let dir = state.paths.instance_dir(&instance_id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    media::clear_custom_banner(&state.paths, &instance_id).await;
    state.media_cache.lock().unwrap().remove(&instance_id);
    Ok(())
}

#[tauri::command]
pub async fn get_instance_media(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Option<VersionMedia>> {
    if let Some(cached) = state.media_cache.lock().unwrap().get(&instance_id) {
        return Ok(cached.clone());
    }

    let result = match media::custom_banner(&state.paths, &instance_id).await {
        Some(banner) => Some(banner),
        None => {
            let instance = find_instance(&state, &instance_id)?;
            let notes = {
                let cached = state.patch_notes.lock().unwrap().clone();
                match cached {
                    Some(notes) => notes,
                    None => {
                        let notes = media::fetch_notes(&state.http, &state.paths).await?;
                        *state.patch_notes.lock().unwrap() = Some(notes.clone());
                        notes
                    }
                }
            };
            media::media_for(&state.http, &state.paths, &notes, &instance.version_id).await
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
pub async fn set_instance_banner(
    state: State<'_, AppState>,
    instance_id: String,
    source_path: String,
) -> Result<VersionMedia> {
    find_instance(&state, &instance_id)?;
    let media = media::set_custom_banner(&state.paths, &instance_id, &source_path).await?;
    state
        .media_cache
        .lock()
        .unwrap()
        .insert(instance_id, Some(media.clone()));
    Ok(media)
}

#[tauri::command]
pub async fn clear_instance_banner(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<()> {
    media::clear_custom_banner(&state.paths, &instance_id).await;
    state.media_cache.lock().unwrap().remove(&instance_id);
    Ok(())
}

fn version_jar_exists(state: &AppState, id: &str, depth: u8) -> bool {
    if state.paths.version_jar(id).is_file() {
        return true;
    }
    if depth == 0 {
        return false;
    }
    let Ok(bytes) = std::fs::read(state.paths.version_json(id)) else {
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
pub fn list_installed_versions(state: State<AppState>) -> Result<Vec<String>> {
    let mut installed = Vec::new();
    let entries = match std::fs::read_dir(state.paths.versions()) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(installed),
        Err(e) => return Err(e.into()),
    };
    for entry in entries.flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        if state.paths.version_json(&id).is_file() && version_jar_exists(&state, &id, 3) {
            installed.push(id);
        }
    }
    Ok(installed)
}

#[tauri::command]
pub async fn list_versions(
    state: State<'_, AppState>,
    include_snapshots: bool,
) -> Result<Vec<VersionEntry>> {
    let manifest = manifest::fetch(&state.http, &state.paths).await?;
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
        .list_instances(&state.paths)?
        .into_iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| Error::NotFound(format!("instance {instance_id}")))
}

#[tauri::command]
pub async fn install_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<()> {
    let instance = find_instance(&state, &instance_id)?;
    let launch_id = match (&instance.loader, &instance.launch_version_id) {
        (Some(_), None) => {
            let id = loaders::install_loader(&app, &state, &instance).await?;
            state.db.set_launch_version(&instance.id, &id)?;
            id
        }
        (_, Some(id)) => id.clone(),
        (None, None) => instance.version_id.clone(),
    };
    install::install_version(&app, &state, &instance.id, &launch_id).await
}

#[tauri::command]
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
pub fn list_instance_content(
    state: State<AppState>,
    instance_id: String,
    kind: String,
) -> Result<Vec<ContentItem>> {
    find_instance(&state, &instance_id)?;
    content::list(&state.paths, &instance_id, &kind)
}

#[tauri::command]
pub fn toggle_instance_content(
    state: State<AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
) -> Result<bool> {
    content::toggle(&state.paths, &instance_id, &kind, &file_name)
}

#[tauri::command]
pub fn delete_instance_content(
    state: State<AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
) -> Result<()> {
    content::delete(&state.paths, &instance_id, &kind, &file_name)
}

#[tauri::command]
pub fn add_instance_content(
    state: State<AppState>,
    instance_id: String,
    kind: String,
    sources: Vec<String>,
) -> Result<usize> {
    find_instance(&state, &instance_id)?;
    content::add(&state.paths, &instance_id, &kind, &sources)
}

#[tauri::command]
pub async fn search_content(
    state: State<'_, AppState>,
    provider: String,
    kind: String,
    query: String,
    game_version: String,
    loader: Option<String>,
) -> Result<Vec<search::SearchResult>> {
    let provider = search::Provider::parse(&provider)?;
    search::search(&state, provider, &kind, &query, &game_version, loader.as_deref()).await
}

#[tauri::command]
pub async fn get_project_details(
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
) -> Result<search::ProjectDetails> {
    let provider = search::Provider::parse(&provider)?;
    search::project_details(&state, provider, &project_id).await
}

#[tauri::command]
pub async fn install_content(
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    instance_id: String,
    kind: String,
    game_version: String,
    loader: Option<String>,
) -> Result<String> {
    find_instance(&state, &instance_id)?;
    let provider = search::Provider::parse(&provider)?;
    search::install(
        &state,
        provider,
        &project_id,
        &instance_id,
        &kind,
        &game_version,
        loader.as_deref(),
    )
    .await
}

#[derive(Serialize)]
pub struct DeviceCodeInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub message: String,
}

async fn run_auth_flow(
    http: reqwest::Client,
    db: Db,
    device_code: String,
    interval: u64,
) -> Result<AccountView> {
    let mut interval = interval.max(1);
    let token = loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        match microsoft::poll_token(&http, &device_code).await? {
            PollOutcome::Pending => continue,
            PollOutcome::SlowDown => {
                interval += 5;
                continue;
            }
            PollOutcome::Token(token) => break token,
        }
    };

    let mc = microsoft::authenticate_minecraft(&http, &token.access_token).await?;
    let account = Account {
        id: mc.uuid.clone(),
        name: mc.name,
        mc_access_token: mc.access_token,
        refresh_token: token.refresh_token,
        expires_at: chrono::Utc::now().timestamp() + mc.expires_in,
    };

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
pub async fn auth_begin(app: AppHandle, state: State<'_, AppState>) -> Result<DeviceCodeInfo> {
    let device = microsoft::request_device_code(&state.http).await?;
    let info = DeviceCodeInfo {
        user_code: device.user_code.clone(),
        verification_uri: device.verification_uri.clone(),
        message: device.message.clone(),
    };

    let http = state.http.clone();
    let db = state.db.clone();
    let device_code = device.device_code.clone();
    let interval = device.interval;

    tokio::spawn(async move {
        match run_auth_flow(http, db, device_code, interval).await {
            Ok(view) => {
                let _ = app.emit("auth:state", json!({ "status": "success", "account": view }));
            }
            Err(e) => {
                let _ = app.emit("auth:state", json!({ "status": "error", "message": e.to_string() }));
            }
        }
    });

    Ok(info)
}

#[tauri::command]
pub fn list_accounts(state: State<AppState>) -> Result<Vec<AccountView>> {
    Ok(state.db.load_accounts()?.views())
}

#[tauri::command]
pub fn set_active_account(state: State<AppState>, account_id: String) -> Result<()> {
    let mut store = state.db.load_accounts()?;
    if store.accounts.iter().any(|a| a.id == account_id) {
        store.active_id = Some(account_id);
        state.db.save_accounts(&store)?;
    }
    Ok(())
}

#[tauri::command]
pub fn remove_account(state: State<AppState>, account_id: String) -> Result<()> {
    let mut store = state.db.load_accounts()?;
    store.accounts.retain(|a| a.id != account_id);
    if store.active_id.as_deref() == Some(account_id.as_str()) {
        store.active_id = store.accounts.first().map(|a| a.id.clone());
    }
    state.db.save_accounts(&store)?;
    Ok(())
}

#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<String> {
    let instance = find_instance(&state, &instance_id)?;
    launch::launch_instance(&app, &state, &instance).await
}

#[tauri::command]
pub fn kill_instance(state: State<AppState>, running_id: String) -> Result<()> {
    let mut registry = state.running.lock().unwrap();
    if let Some(handle) = registry.get_mut(&running_id) {
        if let Some(tx) = handle.kill_tx.take() {
            let _ = tx.send(());
        }
    }
    Ok(())
}

#[tauri::command]
pub fn list_running(state: State<AppState>) -> Result<Vec<RunningInfo>> {
    let registry = state.running.lock().unwrap();
    Ok(registry.iter().map(|(id, handle)| handle.info(id)).collect())
}

#[tauri::command]
pub fn get_logs(state: State<AppState>, running_id: String) -> Result<Vec<LogLine>> {
    let registry = state.running.lock().unwrap();
    Ok(registry
        .get(&running_id)
        .map(|handle| handle.logs.lock().unwrap().clone())
        .unwrap_or_default())
}

#[tauri::command]
pub fn close_running(state: State<AppState>, running_id: String) -> Result<()> {
    state.running.lock().unwrap().remove(&running_id);
    Ok(())
}
