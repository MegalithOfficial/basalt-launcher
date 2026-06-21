use tauri::State;

use crate::config::{self, Instance, LauncherSettings};
use crate::error::Result;
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
