use tauri::{AppHandle, State};

use crate::{
    error::Result,
    launch::{
        self,
        process::{LogLine, RunningInfo},
    },
    state::AppState,
};

use super::find_instance;

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<String> {
    if state
        .tasks
        .has_active(&instance_id, crate::tasks::TaskKind::WorldImport)
    {
        return Err(crate::error::Error::other(
            "Wait for the world import to finish before launching this instance.",
        ));
    }
    let instance = find_instance(&state, &instance_id)?;
    launch::launch_instance(&app, &state, &instance).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn kill_instance(state: State<AppState>, running_id: String) -> Result<()> {
    let mut registry = state.running.lock().unwrap();
    if let Some(handle) = registry.get_mut(&running_id) {
        if handle.request_kill(&running_id) {
            tracing::info!(pid = handle.pid, "kill requested");
        }
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn list_running(state: State<AppState>) -> Result<Vec<RunningInfo>> {
    let registry = state.running.lock().unwrap();
    Ok(registry
        .iter()
        .map(|(id, handle)| handle.info(id))
        .collect())
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
