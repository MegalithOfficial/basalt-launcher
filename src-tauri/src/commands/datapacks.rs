use tauri::{AppHandle, State};

use crate::{
    datapacks::{self, WorldPacks},
    error::Result,
    search::Provider,
    state::AppState,
};

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_instance_datapacks(
    state: State<AppState>,
    instance_id: String,
) -> Result<Vec<WorldPacks>> {
    let instance = super::find_instance(&state, &instance_id)?;
    datapacks::list(&state.files, &state.db, &instance)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn toggle_datapack(
    state: State<AppState>,
    instance_id: String,
    world: String,
    file_name: String,
) -> Result<bool> {
    super::find_instance(&state, &instance_id)?;
    datapacks::toggle(&state.files, &instance_id, &world, &file_name)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_datapack(
    state: State<AppState>,
    instance_id: String,
    world: String,
    file_name: String,
) -> Result<()> {
    super::find_instance(&state, &instance_id)?;
    datapacks::delete(&state.files, &instance_id, &world, &file_name)?;
    state
        .db
        .delete_world_datapack(&instance_id, &world, &file_name)?;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn add_datapacks(
    state: State<AppState>,
    instance_id: String,
    world: String,
    sources: Vec<String>,
) -> Result<usize> {
    super::find_instance(&state, &instance_id)?;
    datapacks::add(&state.files, &instance_id, &world, &sources)
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn install_datapack(
    app: AppHandle,
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    instance_id: String,
    world: String,
    version_id: Option<String>,
) -> Result<Vec<String>> {
    let instance = super::find_instance(&state, &instance_id)?;
    let provider = Provider::parse(&provider)?;
    datapacks::install(
        &app,
        &state,
        provider,
        &project_id,
        &instance,
        &world,
        version_id.as_deref(),
    )
    .await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn check_datapack_updates(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<usize> {
    let instance = super::find_instance(&state, &instance_id)?;
    datapacks::check_updates(&state, &instance).await
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn apply_datapack_update(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    world: String,
    file_name: String,
) -> Result<Vec<String>> {
    let instance = super::find_instance(&state, &instance_id)?;
    let recorded = state.db.world_datapacks(&instance_id, &world)?;
    let row = recorded
        .into_iter()
        .find(|entry| entry.file_name == file_name)
        .ok_or_else(|| crate::error::Error::NotFound(format!("datapack {file_name}")))?;

    let (Some(provider), Some(project_id), Some(latest)) =
        (row.provider, row.project_id, row.latest_version_id)
    else {
        return Err(crate::error::Error::other(
            "Basalt does not know where this datapack came from.",
        ));
    };

    let installed = datapacks::install(
        &app,
        &state,
        Provider::parse(&provider)?,
        &project_id,
        &instance,
        &world,
        Some(&latest),
    )
    .await?;

    if !installed.iter().any(|name| name == &file_name) {
        datapacks::delete(&state.files, &instance_id, &world, &file_name)?;
        state
            .db
            .delete_world_datapack(&instance_id, &world, &file_name)?;
    }

    Ok(installed)
}
