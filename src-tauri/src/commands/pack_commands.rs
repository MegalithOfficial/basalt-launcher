use std::path::PathBuf;

use reqwest::Url;
use tauri::{AppHandle, Manager, State};

use crate::{
    config::Instance,
    error::{Error, Result},
    packs::{self, PackExport, PackFormat, PackPreview},
    state::AppState,
};

use super::find_instance;

fn validate_packwiz_url(value: &str) -> Result<()> {
    let url =
        Url::parse(value).map_err(|error| Error::other(format!("invalid packwiz URL: {error}")))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(Error::other("packwiz URLs must use HTTP or HTTPS"));
    }
    Ok(())
}

fn finish_import_in_background(app: &AppHandle, prepared: packs::PreparedImport) -> Instance {
    let instance = prepared.instance().clone();
    let handle = app.clone();
    tokio::spawn(async move {
        let state = handle.state::<AppState>();
        packs::finish_import(&handle, &state, prepared).await;
    });
    instance
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn inspect_pack_file(state: State<'_, AppState>, path: String) -> Result<PackPreview> {
    let path = PathBuf::from(path);
    if !state.files.is_external_file(&path) {
        return Err(Error::NotFound(path.display().to_string()));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("toml"))
    {
        return packs::inspect_packwiz(&state, &path.display().to_string()).await;
    }
    packs::inspect_pack(&state, &path).await
}

#[tauri::command]
#[tracing::instrument(skip(state, url), err)]
pub async fn inspect_packwiz_url(state: State<'_, AppState>, url: String) -> Result<PackPreview> {
    validate_packwiz_url(&url)?;
    packs::inspect_packwiz(&state, &url).await
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

    let prepared = if path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("toml"))
    {
        packs::prepare_packwiz_import(&app, &state, &path.display().to_string(), name).await?
    } else {
        packs::prepare_import(&app, &state, &path, name).await?
    };
    Ok(finish_import_in_background(&app, prepared))
}

#[tauri::command]
#[tracing::instrument(skip(app, state, url), err)]
pub async fn import_packwiz_url(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    name: Option<String>,
) -> Result<Instance> {
    validate_packwiz_url(&url)?;
    let prepared = packs::prepare_packwiz_import(&app, &state, &url, name).await?;
    Ok(finish_import_in_background(&app, prepared))
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
