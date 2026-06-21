use tauri::{AppHandle, State};

use crate::config::{self, Instance, LauncherSettings};
use crate::error::{Error, Result};
use crate::install;
use crate::java::{self, JavaStatus};
use crate::meta::manifest::{self, VersionEntry};
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
    let found = java::detect(explicit.as_deref()).await;
    let ok = found.as_ref().map_or(false, |j| j.major == required_major);

    Ok(JavaStatus {
        required_major,
        found,
        ok,
    })
}
