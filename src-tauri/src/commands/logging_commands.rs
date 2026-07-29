use super::*;

#[tauri::command]
#[tracing::instrument(skip(logs), err)]
pub fn get_log_records(logs: State<LogState>, limit: Option<usize>) -> Result<Vec<LogRecord>> {
    Ok(logs.buffer.snapshot(limit.unwrap_or(2000)))
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn clear_log_records(logs: State<LogState>) -> Result<()> {
    logs.buffer.clear();
    tracing::info!("log view cleared");
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_log_config(state: State<AppState>) -> Result<LogConfig> {
    Ok(logging::config(&state.files))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn set_log_level(state: State<AppState>, level: String) -> Result<LogConfig> {
    logging::set_level(&level)?;
    let mut settings = state.db.load_settings()?;
    settings.log_level = logging::normalize_level(&level).to_string();
    state.db.save_settings(&settings)?;
    Ok(logging::config(&state.files))
}

#[tauri::command]
pub fn frontend_log(
    level: String,
    scope: String,
    message: String,
    data: Option<String>,
) -> Result<()> {
    logging::record_frontend(&level, &scope, &message, data.as_deref());
    Ok(())
}
