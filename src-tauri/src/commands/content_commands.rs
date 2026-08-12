use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::{
    config::Instance,
    content::{self, ContentItem},
    download,
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
        search::identify::reconcile(
            &state,
            search::resolve::Target::Instance(&instance_id),
            &kind,
        )
        .await?;
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
            search::identify::reconcile(
                &state,
                search::resolve::Target::Instance(&instance_id),
                &kind,
            )
            .await?;
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
pub fn plan_content_removal(
    state: State<AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
) -> Result<search::resolve::RemovalPlan> {
    let kind = search::ContentKind::parse(&kind)?;
    Ok(search::resolve::plan_removal(
        &state,
        search::resolve::Target::Instance(&instance_id),
        kind,
        &file_name,
    ))
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
        search::resolve::Target::Instance(&instance_id),
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
    if let Err(error) = search::identify::reconcile(
        &state,
        search::resolve::Target::Instance(&instance_id),
        &kind,
    )
    .await
    {
        tracing::warn!(%error, "could not identify added content");
    }
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
        search::resolve::Target::Instance(&instance_id),
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
        search::resolve::Target::Instance(&instance_id),
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
        search::resolve::Target::Instance(&instance_id),
        kind,
        None,
    )
    .await
}

#[tauri::command]
pub async fn get_server_pack_file(
    state: State<'_, AppState>,
    project_id: String,
    file_id: String,
    parent_id: String,
) -> Result<search::VersionFile> {
    search::curseforge::server_pack(&state, &project_id, &file_id, &parent_id).await
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

async fn restricted_update(
    state: &AppState,
    instance_id: &str,
    kind: &str,
    file_name: &str,
) -> Result<Option<(crate::modpack::ManualDownload, search::ProjectVersion)>> {
    let instance = find_instance(state, instance_id)?;
    let kind_enum = search::ContentKind::parse(kind)?;
    let file = state
        .db
        .content_file(instance_id, kind, file_name)?
        .ok_or_else(|| Error::other("This file is not linked to a project."))?;
    let (Some(provider), Some(project_id)) = (file.provider, file.project_id) else {
        return Err(Error::other("This file is not linked to a project."));
    };
    if provider != "curseforge" {
        return Ok(None);
    }
    let update = state
        .db
        .content_updates(instance_id)?
        .into_iter()
        .find(|update| update.kind == kind && update.file_name == file_name)
        .ok_or_else(|| Error::other("No update is available for this file."))?;
    let version = search::fetch_version(
        state,
        search::Provider::Curseforge,
        &project_id,
        kind_enum,
        &instance.version_id,
        instance.loader.as_deref(),
        Some(&update.latest_version_id),
    )
    .await?;
    let archive = version
        .primary_file()
        .cloned()
        .ok_or_else(|| Error::other("This update has no downloadable file."))?;
    if archive.url.is_some() {
        return Ok(None);
    }
    let page = search::curseforge::project_download_pages(state, std::slice::from_ref(&project_id))
        .await?
        .remove(&project_id)
        .ok_or_else(|| Error::other("CurseForge did not provide the project's download page."))?;
    let requirement = crate::modpack::ManualDownload {
        project_id,
        file_id: version.id.clone(),
        file_name: archive.file_name.clone(),
        download_page_url: format!("{}/download/{}", page.trim_end_matches('/'), version.id),
        sha1: archive.sha1.clone(),
        size: archive.size,
        instance_path: format!("{kind}/{}", archive.file_name),
        pack_archive: false,
    };
    Ok(Some((requirement, version)))
}

#[tauri::command]
pub async fn plan_content_update(
    state: State<'_, AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
) -> Result<Option<crate::modpack::ManualDownload>> {
    Ok(restricted_update(&state, &instance_id, &kind, &file_name)
        .await?
        .map(|(requirement, _)| requirement))
}

#[tauri::command]
pub async fn apply_content_update(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    kind: String,
    file_name: String,
    manual_downloads: Option<Vec<crate::modpack::ManualDownloadSource>>,
) -> Result<String> {
    if let Some((requirement, version)) =
        restricted_update(&state, &instance_id, &kind, &file_name).await?
    {
        let source = manual_downloads
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|source| {
                source.project_id == requirement.project_id && source.file_id == requirement.file_id
            })
            .ok_or_else(|| {
                Error::other("Download the requested CurseForge file before continuing.")
            })?;
        let source_path = std::path::PathBuf::from(&source.path);
        let downloads_dir = app.path().download_dir()?;
        crate::modpack::validate_manual_download(&downloads_dir, &requirement, &source_path)?;

        let kind_enum = search::ContentKind::parse(&kind)?;
        let directory = content::dir_for(&state.paths, &instance_id, kind_enum.as_str())?;
        state.files.ensure_dir(&directory)?;
        let destination = directory.join(&requirement.file_name);
        download::copy_verified(
            &state.files,
            &source_path,
            &destination,
            requirement.sha1.as_deref(),
            requirement.size,
        )
        .await?;

        let previous = state
            .db
            .content_file(&instance_id, &kind, &file_name)?
            .ok_or_else(|| Error::other("This file is not linked to a project."))?;
        let primary = version
            .primary_file()
            .ok_or_else(|| Error::other("This update has no downloadable file."))?;
        state.db.record_content_file(
            &instance_id,
            &kind,
            &crate::db::ContentFile {
                file_name: requirement.file_name.clone(),
                sha1: primary.sha1.clone(),
                sha512: primary.sha512.clone(),
                version_id: Some(version.id.clone()),
                dependencies: serde_json::to_string(&version.dependencies).ok(),
                installed_at: chrono::Utc::now().timestamp(),
                ..previous.clone()
            },
        )?;
        if file_name != requirement.file_name {
            content::delete(&state.files, &instance_id, &kind, &file_name)?;
            state
                .db
                .delete_content_file(&instance_id, &kind, &file_name)?;
        }
        if let Err(error) = std::fs::remove_file(&source_path) {
            tracing::warn!(path = %source_path.display(), %error, "could not remove consumed CurseForge download");
        }
        return Ok(requirement.file_name);
    }

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
        search::resolve::Target::Instance(&instance_id),
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
        search::resolve::Target::Instance(&instance_id),
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
    manual_downloads: Option<Vec<crate::modpack::ManualDownloadSource>>,
) -> Result<Instance> {
    let provider = search::Provider::parse(&provider)?;
    crate::modpack::install_modpack(
        &app,
        &state,
        provider,
        &project_id,
        &version_id,
        manual_downloads.as_deref().unwrap_or_default(),
    )
    .await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn plan_modpack_install(
    app: AppHandle,
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    version_id: String,
    manual_downloads: Option<Vec<crate::modpack::ManualDownloadSource>>,
) -> Result<crate::modpack::ModpackInstallPlan> {
    let provider = search::Provider::parse(&provider)?;
    crate::modpack::plan_modpack_install(
        &app,
        &state,
        provider,
        &project_id,
        &version_id,
        manual_downloads.as_deref().unwrap_or_default(),
    )
    .await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn check_modpack_upgrade(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Option<crate::modpack::ModpackUpgrade>> {
    let instance = find_instance(&state, &instance_id)?;
    crate::modpack::check_modpack_upgrade(&state, &instance).await
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn plan_modpack_upgrade(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    target_version_id: String,
    manual_downloads: Option<Vec<crate::modpack::ManualDownloadSource>>,
) -> Result<crate::modpack::ModpackUpgradePlan> {
    let instance = find_instance(&state, &instance_id)?;
    crate::modpack::plan_modpack_upgrade(
        &app,
        &state,
        &instance,
        &target_version_id,
        manual_downloads.as_deref().unwrap_or_default(),
    )
    .await
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn link_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    provider: String,
    project_id: String,
    version_id: String,
) -> Result<Instance> {
    let instance = find_instance(&state, &instance_id)?;
    let provider = crate::search::Provider::parse(&provider)?;
    crate::modpack::link_modpack(&app, &state, &instance, provider, &project_id, &version_id)
        .await?;
    find_instance(&state, &instance_id)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn unlink_modpack(state: State<'_, AppState>, instance_id: String) -> Result<Instance> {
    find_instance(&state, &instance_id)?;
    state.db.unlink_instance_pack(&instance_id)?;
    find_instance(&state, &instance_id)
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn upgrade_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    target_version_id: String,
    manual_downloads: Option<Vec<crate::modpack::ManualDownloadSource>>,
    snapshot_first: Option<bool>,
) -> Result<Instance> {
    let instance = find_instance(&state, &instance_id)?;
    crate::modpack::upgrade_modpack(
        &app,
        &state,
        instance,
        &target_version_id,
        manual_downloads.as_deref().unwrap_or_default(),
        snapshot_first.unwrap_or(true),
    )
    .await
}

#[tauri::command]
#[tracing::instrument(skip(app), err)]
pub async fn find_curseforge_download(
    app: AppHandle,
    download: crate::modpack::ManualDownload,
    started_at_ms: u64,
) -> Result<Option<String>> {
    crate::modpack::find_manual_download(&app, &download, started_at_ms).await
}
