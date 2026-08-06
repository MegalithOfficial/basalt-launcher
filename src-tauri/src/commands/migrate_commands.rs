use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::{
    error::{Error, Result},
    migrate::{self, LauncherKind, LauncherSource, MigrationOutcome, MigrationScan},
    state::AppState,
    tasks::{TaskKind, TaskSpec},
};

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn detect_launchers(state: State<'_, AppState>) -> Result<Vec<LauncherSource>> {
    let files = state.files.clone();
    tokio::task::spawn_blocking(move || Ok(migrate::detect(&files)))
        .await
        .map_err(|error| Error::other(format!("launcher detection failed: {error}")))?
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn scan_launcher(
    state: State<'_, AppState>,
    kind: String,
    root: String,
) -> Result<MigrationScan> {
    let kind = LauncherKind::parse(&kind)?;
    let files = state.files.clone();
    let db = state.db.clone();
    let root = PathBuf::from(root);
    tokio::task::spawn_blocking(move || migrate::scan(&files, &db, kind, &root))
        .await
        .map_err(|error| Error::other(format!("launcher scan failed: {error}")))?
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn migrate_instances(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: String,
    root: String,
    ids: Vec<String>,
) -> Result<MigrationOutcome> {
    let kind = LauncherKind::parse(&kind)?;
    if ids.is_empty() {
        return Err(Error::other("no instances were selected"));
    }

    let task = state.tasks.start(
        &app,
        TaskKind::InstanceImport,
        TaskSpec {
            title: if ids.len() == 1 {
                "Importing instance".to_string()
            } else {
                format!("Importing {} instances", ids.len())
            },
            subtitle: Some("From ATLauncher".to_string()),
            ..Default::default()
        },
    )?;

    let files = state.files.clone();
    let db = state.db.clone();
    let root = PathBuf::from(root);

    let result = tokio::task::spawn_blocking(move || {
        let outcome = migrate::import(&files, &db, kind, &root, &ids, &task);
        task.finish(&outcome);
        outcome
    })
    .await
    .map_err(|error| Error::other(format!("migration task failed: {error}")))?;

    result
}
