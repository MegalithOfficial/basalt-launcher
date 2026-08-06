use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, State};

use crate::{
    error::{Error, Result},
    state::AppState,
    storage::{self, ReclaimOutcome, StorageReport, Store},
    tasks::{TaskKind, TaskSpec},
};

fn remembered() -> &'static Mutex<Option<StorageReport>> {
    static REPORT: OnceLock<Mutex<Option<StorageReport>>> = OnceLock::new();
    REPORT.get_or_init(|| Mutex::new(None))
}

fn nothing_running(state: &AppState) -> Result<()> {
    let playing = state
        .running
        .lock()
        .unwrap()
        .values()
        .any(|handle| handle.status.lock().unwrap().state == "running");
    if playing {
        return Err(Error::other("Close the game before clearing storage."));
    }
    let working = state
        .tasks
        .list()
        .iter()
        .any(|task| task.state == crate::tasks::TaskState::Running);
    if working {
        return Err(Error::other(
            "Wait for the current download to finish before clearing storage.",
        ));
    }
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn scan_storage(
    app: AppHandle,
    state: State<'_, AppState>,
    force: bool,
) -> Result<StorageReport> {
    if !force {
        if let Some(report) = remembered().lock().unwrap().clone() {
            return Ok(report);
        }
    }

    let task = state.tasks.start(
        &app,
        TaskKind::StorageScan,
        TaskSpec {
            title: "Measuring storage".to_string(),
            ..Default::default()
        },
    )?;

    let store = Store::from_state(&state);
    let result = tokio::task::spawn_blocking(move || {
        let scanned = storage::scan(&store, Some(&task));
        task.finish(&scanned);
        scanned
    })
    .await
    .map_err(|error| Error::other(format!("storage scan failed: {error}")))?;

    if let Ok(report) = &result {
        *remembered().lock().unwrap() = Some(report.clone());
    }
    result
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn reclaim_storage(
    state: State<'_, AppState>,
    targets: Vec<String>,
) -> Result<ReclaimOutcome> {
    nothing_running(&state)?;
    if targets.is_empty() {
        return Err(Error::other("Nothing was selected."));
    }

    let store = Store::from_state(&state);
    let outcome = tokio::task::spawn_blocking(move || storage::reclaim(&store, &targets))
        .await
        .map_err(|error| Error::other(format!("storage cleanup failed: {error}")))??;

    *remembered().lock().unwrap() = None;
    Ok(outcome)
}
