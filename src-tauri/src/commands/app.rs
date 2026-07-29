use serde::Serialize;
use tauri::State;

use crate::{
    config::LauncherSettings,
    error::Result,
    java, launch, logging,
    state::AppState,
    sysinfo_probe::{self, SystemStats, SystemUsage},
    update::{self, UpdateInfo},
};

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_settings(state: State<AppState>) -> Result<LauncherSettings> {
    state.db.load_settings()
}

#[derive(Serialize)]
pub struct AppInfo {
    pub version: String,
    pub build_channel: String,
    pub data_dir: String,
    pub default_jvm_args: String,
    pub jvm_placeholders: Vec<String>,
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_app_info(state: State<AppState>) -> Result<AppInfo> {
    Ok(AppInfo {
        version: crate::build_info::VERSION.to_string(),
        build_channel: crate::build_info::CHANNEL.to_string(),
        data_dir: state.paths.root.display().to_string(),
        default_jvm_args: crate::config::DEFAULT_JVM_ARGS.to_string(),
        jvm_placeholders: crate::launch::PLACEHOLDERS
            .iter()
            .map(|p| p.to_string())
            .collect(),
    })
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn list_javas(state: State<'_, AppState>) -> Result<Vec<java::JavaInfo>> {
    Ok(java::list_all(&state.files).await)
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn update_settings(state: State<AppState>, settings: LauncherSettings) -> Result<()> {
    if logging::normalize_level(&settings.log_level) != logging::current_level() {
        logging::set_level(&settings.log_level)?;
    }
    state.db.save_settings(&settings)
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn check_for_updates(state: State<'_, AppState>) -> Result<UpdateInfo> {
    update::check(&state.network).await
}

#[derive(Serialize)]
pub struct AboutLinks {
    pub repository: String,
    pub issues: String,
    pub releases: String,
}

#[tauri::command]
pub fn get_about_links() -> AboutLinks {
    AboutLinks {
        repository: update::REPO_URL.to_string(),
        issues: format!("{}/issues/new", update::REPO_URL),
        releases: format!("{}/releases", update::REPO_URL),
    }
}
#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_system_stats(state: State<AppState>) -> Result<SystemStats> {
    Ok(sysinfo_probe::collect(&state.paths))
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn preview_launch_args(
    state: State<AppState>,
    settings: LauncherSettings,
) -> Result<launch::LaunchPreview> {
    Ok(launch::preview(&state.paths, &settings))
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_system_usage(state: State<AppState>) -> Result<SystemUsage> {
    Ok(sysinfo_probe::usage(&state.paths))
}
