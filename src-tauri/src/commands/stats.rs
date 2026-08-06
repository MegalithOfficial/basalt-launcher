use tauri::State;

use crate::{db::PlayStats, error::Result, state::AppState};

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_play_stats(
    state: State<AppState>,
    days: Option<u32>,
    page: Option<u32>,
) -> Result<PlayStats> {
    state.db.play_stats(days, page)
}
