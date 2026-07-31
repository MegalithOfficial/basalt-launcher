use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use crate::{
    config::Instance,
    error::{Error, Result},
    packs::{self, PackExport, PackFormat, PackPreview},
    state::AppState,
};

use super::find_instance;

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn inspect_pack_file(state: State<'_, AppState>, path: String) -> Result<PackPreview> {
    let path = PathBuf::from(path);
    if !state.files.is_external_file(&path) {
        return Err(Error::NotFound(path.display().to_string()));
    }
    packs::inspect_pack(&state, &path).await
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn import_pack_file(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    name: Option<String>,
) -> Result<Instance> {
    let path = PathBuf::from(path);
    if !state.files.is_external_file(&path) {
        return Err(Error::NotFound(path.display().to_string()));
    }

    let prepared = packs::prepare_import(&app, &state, &path, name).await?;
    let instance = prepared.instance().clone();

    let handle = app.clone();
    tokio::spawn(async move {
        let state = handle.state::<AppState>();
        packs::finish_import(&handle, &state, prepared).await;
    });

    Ok(instance)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn export_instance_pack(
    state: State<'_, AppState>,
    instance_id: String,
    format: String,
    path: String,
) -> Result<PackExport> {
    let format = PackFormat::parse(&format)?;
    let instance = find_instance(&state, &instance_id)?;
    packs::export_instance(&state, &instance, format, PathBuf::from(path)).await
}

#[tauri::command]
pub fn pack_export_name(name: String, format: String) -> Result<String> {
    let format = PackFormat::parse(&format)?;
    let stem: String = name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | ' ' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect();
    let stem = stem.trim().trim_matches('.').to_string();
    let stem = if stem.is_empty() {
        "instance".to_string()
    } else {
        stem
    };
    Ok(format!("{stem}.{}", format.extension()))
}
