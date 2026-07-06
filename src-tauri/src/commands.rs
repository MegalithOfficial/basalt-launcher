use std::time::Duration;

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, State};

use crate::auth::account::{self, Account, AccountView};
use crate::auth::microsoft::{self, PollOutcome};
use crate::config::{self, Instance, LauncherSettings};
use crate::error::{Error, Result};
use crate::install;
use crate::java::{self, JavaStatus};
use crate::launch::{self, process::{LogLine, RunningInfo}};
use crate::meta::manifest::{self, VersionEntry};
use crate::paths::Paths;
use crate::state::AppState;

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Result<LauncherSettings> {
    Ok(state.settings.lock().unwrap().clone())
}

#[tauri::command]
pub fn update_settings(state: State<AppState>, settings: LauncherSettings) -> Result<()> {
    config::save_settings(&state.paths, &settings)?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
pub fn list_instances(state: State<AppState>) -> Result<Vec<Instance>> {
    config::load_instances(&state.paths)
}

#[tauri::command]
pub fn create_instance(
    state: State<AppState>,
    name: String,
    version_id: String,
) -> Result<Instance> {
    let mut instances = config::load_instances(&state.paths)?;
    let instance = Instance {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        version_id,
        created_at: chrono::Utc::now(),
        min_memory_mb: None,
        max_memory_mb: None,
        java_path: None,
    };
    std::fs::create_dir_all(state.paths.instance_dir(&instance.id))?;
    instances.push(instance.clone());
    config::save_instances(&state.paths, &instances)?;
    Ok(instance)
}

#[tauri::command]
pub fn delete_instance(state: State<AppState>, instance_id: String) -> Result<()> {
    let mut instances = config::load_instances(&state.paths)?;
    instances.retain(|i| i.id != instance_id);
    config::save_instances(&state.paths, &instances)?;
    let dir = state.paths.instance_dir(&instance_id);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
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
        if state.paths.version_json(&id).is_file() && state.paths.version_jar(&id).is_file() {
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
    config::load_instances(&state.paths)?
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
    install::install_version(&app, &state, &instance.id, &instance.version_id).await
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
        .or_else(|| state.settings.lock().unwrap().java_path.clone());
    let found = java::find_for_major(required_major, explicit.as_deref()).await;
    let ok = found.as_ref().map_or(false, |j| j.major >= required_major);

    Ok(JavaStatus {
        required_major,
        found,
        ok,
    })
}

#[derive(Serialize)]
pub struct DeviceCodeInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub message: String,
}

async fn run_auth_flow(
    http: reqwest::Client,
    paths: Paths,
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

    let mut store = account::load(&paths)?;
    store.upsert_active(account);
    account::save(&paths, &store)?;

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
    let paths = state.paths.clone();
    let device_code = device.device_code.clone();
    let interval = device.interval;

    tokio::spawn(async move {
        match run_auth_flow(http, paths, device_code, interval).await {
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
    Ok(account::load(&state.paths)?.views())
}

#[tauri::command]
pub fn set_active_account(state: State<AppState>, account_id: String) -> Result<()> {
    let mut store = account::load(&state.paths)?;
    if store.accounts.iter().any(|a| a.id == account_id) {
        store.active_id = Some(account_id);
        account::save(&state.paths, &store)?;
    }
    Ok(())
}

#[tauri::command]
pub fn remove_account(state: State<AppState>, account_id: String) -> Result<()> {
    let mut store = account::load(&state.paths)?;
    store.accounts.retain(|a| a.id != account_id);
    if store.active_id.as_deref() == Some(account_id.as_str()) {
        store.active_id = store.accounts.first().map(|a| a.id.clone());
    }
    account::save(&state.paths, &store)?;
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
