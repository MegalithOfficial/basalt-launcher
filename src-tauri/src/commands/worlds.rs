use std::{path::PathBuf, sync::Arc};

use tauri::{AppHandle, State};

use crate::{
    error::{Error, Result},
    state::AppState,
    tasks::{TaskKind, TaskSpec},
    worlds::{self, ImportInspection, WorldSummary},
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

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn inspect_world_source(
    state: State<'_, AppState>,
    source_path: String,
) -> Result<ImportInspection> {
    let files = state.files.clone();
    let source = PathBuf::from(source_path);
    tokio::task::spawn_blocking(move || worlds::inspect_source(&files, &source))
        .await
        .map_err(|error| {
            crate::error::Error::other(format!("world inspection task failed: {error}"))
        })?
}

#[tauri::command]
#[tracing::instrument(skip(app, state, candidate_ids), err)]
pub async fn import_worlds(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    source_path: String,
    candidate_ids: Vec<String>,
) -> Result<usize> {
    let instance = find_instance(&state, &instance_id)?;
    let running = state.running.lock().unwrap().values().any(|handle| {
        handle.instance_id == instance_id && handle.status.lock().unwrap().state == "running"
    });
    if running {
        return Err(Error::other("Stop this instance before importing worlds."));
    }
    if state.tasks.has_active(&instance_id, TaskKind::WorldImport) {
        return Err(Error::other(
            "Another world import is already running for this instance.",
        ));
    }

    let task = Arc::new(state.tasks.start(
        &app,
        TaskKind::WorldImport,
        TaskSpec {
            title: if candidate_ids.len() == 1 {
                "Importing world".to_string()
            } else {
                format!("Importing {} worlds", candidate_ids.len())
            },
            subtitle: Some(instance.name),
            instance_id: Some(instance_id.clone()),
            ..Default::default()
        },
    ));
    let files = state.files.clone();
    let source = PathBuf::from(source_path);
    let worker_task = Arc::clone(&task);
    let worker_instance_id = instance_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        worlds::import_selected(
            &files,
            &worker_instance_id,
            &source,
            &candidate_ids,
            &worker_task,
        )
    })
    .await
    .map_err(|error| Error::other(format!("world import task failed: {error}")))?;
    task.finish(&result);
    result
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn delete_instance_world(
    state: State<'_, AppState>,
    instance_id: String,
    folder_name: String,
) -> Result<()> {
    find_instance(&state, &instance_id)?;

    let running = state.running.lock().unwrap().values().any(|handle| {
        handle.instance_id == instance_id && handle.status.lock().unwrap().state == "running"
    });
    if running {
        return Err(Error::other("Stop this instance before deleting a world."));
    }

    if folder_name.trim().is_empty()
        || folder_name.contains('/')
        || folder_name.contains('\\')
        || folder_name.contains("..")
    {
        return Err(Error::other(format!("not a world folder: {folder_name}")));
    }

    let saves = state
        .files
        .paths()
        .instance_saves_dir_checked(&instance_id)
        .ok_or_else(|| Error::other("invalid instance id"))?;
    let dir = saves.join(&folder_name);
    if dir.parent() != Some(saves.as_path()) {
        return Err(Error::other(format!("not a world folder: {folder_name}")));
    }

    let files = state.files.clone();
    let removed = tokio::task::spawn_blocking(move || files.remove_managed_dir_all_if_exists(&dir))
        .await
        .map_err(|error| Error::other(format!("world delete task failed: {error}")))??;

    if !removed {
        return Err(Error::NotFound(format!("world {folder_name}")));
    }
    tracing::info!(world = %folder_name, "world deleted");
    Ok(())
}
