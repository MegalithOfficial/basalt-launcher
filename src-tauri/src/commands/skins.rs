use tauri::State;

use crate::{
    error::Result,
    skin::{self, Appearance, SkinEntry},
    state::AppState,
};

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn get_appearance(state: State<'_, AppState>) -> Result<Appearance> {
    skin::appearance(&state).await
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn list_skins(state: State<AppState>) -> Result<Vec<SkinEntry>> {
    skin::library(&state)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn add_skin_from_file(
    state: State<AppState>,
    path: String,
    name: Option<String>,
    variant: String,
) -> Result<SkinEntry> {
    skin::add_from_file(&state, &path, name.as_deref(), &variant)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn add_skin_from_reference(
    state: State<'_, AppState>,
    reference: String,
) -> Result<SkinEntry> {
    skin::add_from_reference(&state, &reference).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_skin(state: State<AppState>, skin_id: String) -> Result<()> {
    skin::remove(&state, &skin_id)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn apply_saved_skin(
    state: State<'_, AppState>,
    skin_id: String,
    variant: Option<String>,
) -> Result<Appearance> {
    skin::apply_saved(&state, &skin_id, variant.as_deref()).await
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub async fn reset_skin(state: State<'_, AppState>) -> Result<Appearance> {
    skin::reset(&state).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn set_cape(state: State<'_, AppState>, cape_id: Option<String>) -> Result<Appearance> {
    skin::set_cape(&state, cape_id.as_deref()).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn rename_skin(state: State<AppState>, skin_id: String, name: String) -> Result<SkinEntry> {
    skin::rename(&state, &skin_id, &name)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn get_worn_skin(state: State<AppState>, uuid: String) -> Result<Option<SkinEntry>> {
    skin::worn_skin(&state, &uuid)
}
