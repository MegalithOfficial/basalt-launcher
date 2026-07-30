use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    config::LauncherSettings,
    error::{Error, Result},
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
    state.db.save_settings(&settings)?;
    state.network.reconfigure(&settings)
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
    let settings = state.db.load_settings()?;
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
