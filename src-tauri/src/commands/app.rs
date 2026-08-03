use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    config::LauncherSettings,
    error::{Error, Result},
    java, launch, logging,
    state::AppState,
    sysinfo_probe::{self, SystemStats, SystemUsage},
    update::{self, AppUpdateStatus, UpdateInfo},
};

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_settings(state: State<AppState>) -> Result<LauncherSettings> {
    state.db.load_settings_view(&state.credentials)
}

#[derive(Serialize)]
pub struct AppInfo {
    pub version: String,
    pub build_channel: String,
    pub data_dir: String,
    pub default_jvm_args: String,
    pub jvm_placeholders: Vec<String>,
    pub arch: String,
    pub install_source: update::InstallSource,
    pub bundled_curseforge_key: bool,
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_app_info(state: State<AppState>) -> Result<AppInfo> {
    Ok(AppInfo {
        version: crate::build_info::display_version(),
        build_channel: crate::build_info::CHANNEL.to_string(),
        data_dir: state.paths.root.display().to_string(),
        default_jvm_args: crate::config::DEFAULT_JVM_ARGS.to_string(),
        jvm_placeholders: crate::launch::PLACEHOLDERS
            .iter()
            .map(|p| p.to_string())
            .collect(),
        arch: std::env::consts::ARCH.to_string(),
        install_source: update::install_source(),
        bundled_curseforge_key: crate::build_info::bundled_curseforge_key().is_some(),
    })
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn list_javas(state: State<'_, AppState>) -> Result<Vec<java::JavaInfo>> {
    Ok(java::list_all(&state.files).await)
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn install_java_runtime(
    app: AppHandle,
    state: State<'_, AppState>,
    major: u32,
    instance_id: Option<String>,
) -> Result<java::JavaInfo> {
    if !(8..=99).contains(&major) {
        return Err(Error::other("Java major version must be between 8 and 99."));
    }
    let platform = format!("{} · {}", std::env::consts::OS, std::env::consts::ARCH);
    let task = state.tasks.start(
        &app,
        crate::tasks::TaskKind::JavaInstall,
        crate::tasks::TaskSpec {
            title: format!("Java {major}"),
            subtitle: Some(format!("Eclipse Temurin · {platform}")),
            ..Default::default()
        },
    );
    let result = async {
        let info = java::managed::install(&state.network, &state.files, major, &task).await?;
        if let Some(instance_id) = instance_id.as_deref() {
            state.db.set_instance_java_path(instance_id, &info.path)?;
        }
        Ok(info)
    }
    .await;
    task.finish(&result);
    result
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn update_settings(state: State<AppState>, settings: LauncherSettings) -> Result<()> {
    if logging::normalize_level(&settings.log_level) != logging::current_level() {
        logging::set_level(&settings.log_level)?;
    }
    if state
        .db
        .load_settings()
        .map(|current| current.pack_content_updates != settings.pack_content_updates)
        .unwrap_or(false)
    {
        state.db.clear_all_content_updates()?;
    }
    let runtime = state
        .db
        .save_settings_secure(&state.credentials, &settings)?;
    state.network.reconfigure(&runtime)
}

#[derive(Debug, serde::Serialize)]
pub struct NetworkProbe {
    pub ok: bool,
    pub status: Option<u16>,
    pub millis: u64,
    pub via_proxy: bool,
    pub error: Option<String>,
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn test_network(state: State<'_, AppState>, url: Option<String>) -> Result<NetworkProbe> {
    let settings = state.runtime_settings()?;
    let target = url
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://api.modrinth.com/v2/tag/loader".to_string());
    let via_proxy = matches!(settings.proxy_mode.as_str(), "http" | "socks5");

    let started = std::time::Instant::now();
    let result = state.network.get(&target).send().await;
    let millis = started.elapsed().as_millis() as u64;

    Ok(match result {
        Ok(response) => NetworkProbe {
            ok: response.status().is_success(),
            status: Some(response.status().as_u16()),
            millis,
            via_proxy,
            error: (!response.status().is_success())
                .then(|| format!("server answered {}", response.status())),
        },
        Err(error) => NetworkProbe {
            ok: false,
            status: None,
            millis,
            via_proxy,
            error: Some(error.to_string()),
        },
    })
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub fn reset_launcher(app: AppHandle, state: State<AppState>, deep: bool) -> Result<()> {
    let running = state
        .running
        .lock()
        .unwrap()
        .values()
        .any(|handle| handle.status.lock().unwrap().state == "running");
    if running {
        return Err(Error::other("Close the game before resetting Basalt."));
    }

    let paths = state.files.paths().clone();
    let mut removed: Vec<PathBuf> = vec![
        paths.instances(),
        paths.root.join("media"),
        paths.skins(),
        paths.logs(),
        paths.root.join("basalt.db"),
        paths.root.join("basalt.db-wal"),
        paths.root.join("basalt.db-shm"),
    ];
    if deep {
        removed.extend([
            paths.versions(),
            paths.libraries(),
            paths.assets(),
            paths.natives(),
            paths.runtimes(),
        ]);
    }

    for path in removed {
        let outcome = if path.is_dir() {
            std::fs::remove_dir_all(&path).map(|_| ())
        } else {
            std::fs::remove_file(&path).or_else(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(error)
                }
            })
        };
        if let Err(error) = outcome {
            tracing::warn!(path = %path.display(), error = %error, "could not remove during reset");
        }
    }

    tracing::warn!(deep, "launcher reset, restarting");
    app.restart()
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn check_for_updates(app: AppHandle, state: State<'_, AppState>) -> Result<UpdateInfo> {
    update::check_and_record(&app, &state.network, &state.updates).await
}

#[tauri::command]
pub fn get_app_update_status(state: State<'_, AppState>) -> AppUpdateStatus {
    state.updates.status()
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn dismiss_app_update(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
) -> Result<AppUpdateStatus> {
    let status = state.updates.dismiss(&version)?;
    update::emit_status(&app, &status);
    Ok(status)
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn download_app_update(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppUpdateStatus> {
    update::download(app, &state).await
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn install_app_update(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    update::install_ready(app, &state)
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
const MAX_INSPECTED_PATHS: usize = 256;

#[derive(Serialize)]
pub struct PathKind {
    pub path: String,
    pub directory: bool,
    pub usable: bool,
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn inspect_paths(paths: Vec<String>) -> Result<Vec<PathKind>> {
    if paths.len() > MAX_INSPECTED_PATHS {
        return Err(Error::other(format!(
            "That is more than {MAX_INSPECTED_PATHS} paths at once."
        )));
    }

    Ok(paths
        .into_iter()
        .map(|path| {
            let metadata = (!path.trim().is_empty())
                .then(|| std::fs::symlink_metadata(&path).ok())
                .flatten()
                .filter(|meta| !meta.file_type().is_symlink());
            PathKind {
                directory: metadata.as_ref().is_some_and(|meta| meta.is_dir()),
                usable: metadata.is_some_and(|meta| meta.is_dir() || meta.is_file()),
                path,
            }
        })
        .collect())
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
