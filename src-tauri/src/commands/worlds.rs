use tauri::State;

use crate::{
    error::Result,
    state::AppState,
    worlds::{self, WorldSummary},
};

use super::find_instance;

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn list_instance_worlds(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<WorldSummary>> {
    find_instance(&state, &instance_id)?;
    let files = state.files.clone();
    tokio::task::spawn_blocking(move || worlds::list(&files, &instance_id))
        .await
        .map_err(|error| {
            crate::error::Error::other(format!("world listing task failed: {error}"))
        })?
}
