use tauri::State;

use crate::config::{self, Instance, LauncherSettings};
use crate::error::Result;
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
