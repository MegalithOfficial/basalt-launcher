use serde::Serialize;
use tauri::{AppHandle, State};

use crate::{
    config::Instance,
    content::{self, ContentItem},
    error::{Error, Result},
    search,
    state::AppState,
};

use super::find_instance;

#[tauri::command]
pub async fn list_instance_content(
    state: State<'_, AppState>,
    instance_id: String,
    kind: String,
    reconcile: Option<bool>,
) -> Result<Vec<ContentItem>> {
    find_instance(&state, &instance_id)?;
    if reconcile.unwrap_or(false) {
        let _ = search::identify::reconcile(&state, &instance_id, &kind).await;
    }

    let mut items = content::list(&state.files, &instance_id, &kind)?;
    let mut sources: std::collections::HashMap<String, crate::db::ContentFile> = state
        .db
        .content_files(&instance_id, &kind)?
        .into_iter()
        .map(|f| (f.file_name.clone(), f))
        .collect();
    let mut updates: std::collections::HashMap<String, crate::db::ContentUpdate> = state
        .db
        .content_updates(&instance_id)?
        .into_iter()
        .filter(|u| u.kind == kind)
        .map(|u| (u.file_name.clone(), u))
        .collect();

    for item in &mut items {
        item.source = sources.remove(&item.file_name);
        item.update = updates.remove(&item.file_name);
    }
    Ok(items)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn list_instance_content_bundle(
    state: State<'_, AppState>,
    instance_id: String,
    kinds: Vec<String>,
    reconcile: Option<bool>,
) -> Result<std::collections::HashMap<String, Vec<ContentItem>>> {
    find_instance(&state, &instance_id)?;
    let reconcile = reconcile.unwrap_or(false);

    let mut sources_by_kind: std::collections::HashMap<String, Vec<crate::db::ContentFile>> =
        std::collections::HashMap::new();
    for kind in &kinds {
        sources_by_kind.insert(kind.clone(), state.db.content_files(&instance_id, kind)?);
    }
    let all_updates = state.db.content_updates(&instance_id)?;

    let mut bundle = std::collections::HashMap::with_capacity(kinds.len());
    for kind in kinds {
        if reconcile {
            let _ = search::identify::reconcile(&state, &instance_id, &kind).await;
        }

        let mut items = content::list(&state.files, &instance_id, &kind)?;
        let mut sources: std::collections::HashMap<String, crate::db::ContentFile> = if reconcile {
            state.db.content_files(&instance_id, &kind)?
        } else {
            sources_by_kind.remove(&kind).unwrap_or_default()
        }
        .into_iter()
        .map(|f| (f.file_name.clone(), f))
        .collect();
        let mut updates: std::collections::HashMap<String, crate::db::ContentUpdate> = all_updates
            .iter()
            .filter(|u| u.kind == kind)
            .map(|u| (u.file_name.clone(), u.clone()))
            .collect();

        for item in &mut items {
            item.source = sources.remove(&item.file_name);
            item.update = updates.remove(&item.file_name);
        }
        bundle.insert(kind, items);
    }

    Ok(bundle)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn toggle_instance_content(
    state: State<AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
) -> Result<bool> {
    let enabled = content::toggle(&state.files, &instance_id, &kind, &file_name)?;
    tracing::info!(enabled, "content toggled");
    Ok(enabled)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_instance_content(
    state: State<AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
) -> Result<()> {
    content::delete(&state.files, &instance_id, &kind, &file_name)?;
    tracing::info!("content deleted");
    state
        .db
        .delete_content_file(&instance_id, &kind, &file_name)
}

#[tauri::command]
pub fn get_content_dependents(
    state: State<AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
) -> Result<Vec<String>> {
    let kind_enum = search::ContentKind::parse(&kind)?;
    let Some(file) = state.db.content_file(&instance_id, &kind, &file_name)? else {
        return Ok(Vec::new());
    };
    let Some(project_id) = file.project_id else {
        return Ok(Vec::new());
    };
    Ok(search::resolve::dependents_of(
        &state,
        &instance_id,
        kind_enum,
        &project_id,
    ))
}

#[tauri::command]
pub async fn add_instance_content(
    state: State<'_, AppState>,
    instance_id: String,
    kind: String,
    sources: Vec<String>,
) -> Result<usize> {
    find_instance(&state, &instance_id)?;
    let copied = content::add(&state.files, &instance_id, &kind, &sources)?;
    let _ = search::identify::reconcile(&state, &instance_id, &kind).await;
    tracing::info!(copied, offered = sources.len(), "content added from disk");
    Ok(copied)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn search_content(
    state: State<'_, AppState>,
    provider: String,
    kind: String,
    query: search::SearchQuery,
) -> Result<search::SearchPage> {
    let provider = search::Provider::parse(&provider)?;
    let kind = search::ContentKind::parse(&kind)?;
    search::search(&state, provider, kind, &query).await
}

#[tauri::command]
pub async fn get_filter_taxonomy(
    state: State<'_, AppState>,
    provider: String,
    kind: String,
    include_snapshots: Option<bool>,
) -> Result<search::FilterTaxonomy> {
    let provider = search::Provider::parse(&provider)?;
    let kind = search::ContentKind::parse(&kind)?;
    search::taxonomy(&state, provider, kind, include_snapshots.unwrap_or(false)).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn get_project_details(
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
) -> Result<search::ProjectDetails> {
    let provider = search::Provider::parse(&provider)?;
    search::project_details(&state, provider, &project_id).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn list_project_versions(
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    kind: String,
    game_version: String,
    loader: Option<String>,
) -> Result<Vec<search::ProjectVersion>> {
    let provider = search::Provider::parse(&provider)?;
    let kind = search::ContentKind::parse(&kind)?;
    search::project_versions(
        &state,
        provider,
        &project_id,
        kind,
        &game_version,
        loader.as_deref(),
    )
    .await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn resolve_projects(
    state: State<'_, AppState>,
    provider: String,
    ids: Vec<String>,
) -> Result<Vec<search::ProjectSummary>> {
    let provider = search::Provider::parse(&provider)?;
    search::resolve_projects(&state, provider, &ids).await
}

#[derive(Serialize)]
pub struct InstalledFile {
    pub version_id: Option<String>,
    pub file_name: String,
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn get_installed_project_file(
    state: State<AppState>,
    instance_id: String,
    kind: String,
    project_id: String,
) -> Result<Option<InstalledFile>> {
    let result = state
        .db
        .installed_project_file(&instance_id, &kind, &project_id)?;
    Ok(result.map(|(version_id, file_name)| InstalledFile {
        version_id,
        file_name,
    }))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn get_version_changelog(
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    version_id: String,
) -> Result<search::Changelog> {
    let provider = search::Provider::parse(&provider)?;
    search::version_changelog(&state, provider, &project_id, &version_id).await
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn plan_content_install(
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    instance_id: String,
    kind: String,
    game_version: String,
    loader: Option<String>,
    version_id: Option<String>,
) -> Result<search::resolve::InstallPlan> {
    find_instance(&state, &instance_id)?;
    let provider = search::Provider::parse(&provider)?;
    let kind = search::ContentKind::parse(&kind)?;
    search::resolve::plan(
        &state,
        provider,
        &project_id,
        &instance_id,
        kind,
        &game_version,
        loader.as_deref(),
        version_id.as_deref(),
        true,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn install_content(
    app: AppHandle,
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    instance_id: String,
    kind: String,
    game_version: String,
    loader: Option<String>,
    version_id: Option<String>,
    with_dependencies: Option<bool>,
) -> Result<Vec<search::resolve::InstalledItem>> {
    find_instance(&state, &instance_id)?;
    let provider = search::Provider::parse(&provider)?;
    let kind = search::ContentKind::parse(&kind)?;
    let plan = search::resolve::plan(
        &state,
        provider,
        &project_id,
        &instance_id,
        kind,
        &game_version,
        loader.as_deref(),
        version_id.as_deref(),
        with_dependencies.unwrap_or(true),
    )
    .await?;
    search::resolve::apply(
        Some(&app),
        &state,
        &plan,
        provider,
        &instance_id,
        kind,
        None,
    )
    .await
}

#[tauri::command]
pub async fn check_content_updates(
    state: State<'_, AppState>,
    instance_id: String,
    force: Option<bool>,
) -> Result<Vec<crate::db::ContentUpdate>> {
    let instance = find_instance(&state, &instance_id)?;
    let checked_at = state.db.updates_checked_at(&instance_id)?;
    if !force.unwrap_or(false) && !search::updates::is_stale(checked_at) {
        return state.db.content_updates(&instance_id);
    }
    search::updates::check(
        &state,
        &instance_id,
        &instance.version_id,
        instance.loader.as_deref(),
    )
    .await
}

#[tauri::command]
pub fn get_content_updates(
    state: State<AppState>,
    instance_id: String,
) -> Result<Vec<crate::db::ContentUpdate>> {
    state.db.content_updates(&instance_id)
}

#[tauri::command]
pub async fn apply_content_update(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
) -> Result<String> {
    let instance = find_instance(&state, &instance_id)?;
    let kind_enum = search::ContentKind::parse(&kind)?;
    let file = state
        .db
        .content_file(&instance_id, &kind, &file_name)?
        .ok_or_else(|| Error::other("This file is not linked to a project."))?;
    let (Some(provider), Some(project_id)) = (file.provider.clone(), file.project_id.clone())
    else {
        return Err(Error::other("This file is not linked to a project."));
    };
    let update = state
        .db
        .content_updates(&instance_id)?
        .into_iter()
        .find(|u| u.kind == kind && u.file_name == file_name)
        .ok_or_else(|| Error::other("No update is available for this file."))?;

    let provider = search::Provider::parse(&provider)?;
    let plan = search::resolve::plan(
        &state,
        provider,
        &project_id,
        &instance_id,
        kind_enum,
        &instance.version_id,
        instance.loader.as_deref(),
        Some(&update.latest_version_id),
        true,
    )
    .await?;
    let written = search::resolve::apply(
        Some(&app),
        &state,
        &plan,
        provider,
        &instance_id,
        kind_enum,
        None,
    )
    .await?;

    state
        .db
        .delete_content_file(&instance_id, &kind, &file_name)
        .ok();
    Ok(written
        .into_iter()
        .next()
        .map(|item| item.file_name)
        .unwrap_or(file_name))
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn install_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    version_id: String,
) -> Result<Instance> {
    let provider = search::Provider::parse(&provider)?;
    crate::modpack::install_modpack(&app, &state, provider, &project_id, &version_id).await
}
