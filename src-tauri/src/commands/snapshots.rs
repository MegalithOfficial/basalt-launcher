use tauri::{AppHandle, State};

use crate::{error::Result, snapshots::SnapshotSummary, state::AppState};

use super::find_instance;

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn list_instance_snapshots(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<SnapshotSummary>> {
    find_instance(&state, &instance_id)?;
    crate::snapshots::list(&state, &instance_id).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn instance_snapshot_usage(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<u64> {
    find_instance(&state, &instance_id)?;
    crate::snapshots::usage(&state, &instance_id).await
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn create_instance_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    name: Option<String>,
    excluded: Option<Vec<String>>,
) -> Result<SnapshotSummary> {
    let instance = find_instance(&state, &instance_id)?;
    crate::snapshots::create(&app, &state, instance, name, excluded.unwrap_or_default()).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn rename_instance_snapshot(
    state: State<'_, AppState>,
    instance_id: String,
    snapshot_id: String,
    name: String,
) -> Result<SnapshotSummary> {
    find_instance(&state, &instance_id)?;
    crate::snapshots::rename(&state, &instance_id, &snapshot_id, &name).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn delete_instance_snapshot(
    state: State<'_, AppState>,
    instance_id: String,
    snapshot_id: String,
) -> Result<()> {
    find_instance(&state, &instance_id)?;
    crate::snapshots::delete(&state, &instance_id, &snapshot_id).await
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn restore_instance_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    snapshot_id: String,
) -> Result<SnapshotSummary> {
    let instance = find_instance(&state, &instance_id)?;
    crate::snapshots::restore(&app, &state, instance, &snapshot_id).await
}
