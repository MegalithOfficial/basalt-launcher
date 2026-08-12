use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    error::{Error, Result},
    servers::{
        config, content,
        files::{FileKind, ServerEntry, ServerText},
        import::{self, ServerFolder},
        pack, players, provision, runtime, software, zippack, Server, TextProblem,
    },
    state::AppState,
    tasks::{TaskKind, TaskSpec},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerProperty {
    pub key: String,
    pub value: String,
}

fn reachable(server: &Server) -> Result<PathBuf> {
    let dir = PathBuf::from(&server.dir);
    if !dir.is_dir() {
        return Err(Error::other(format!(
            "Basalt cannot reach {}. Plug the drive back in or remove the server.",
            server.dir
        )));
    }
    Ok(dir)
}

fn cache_config(state: &AppState, server: &Server, config: &config::Config) -> Result<()> {
    state.db.cache_server_properties(
        &server.id,
        config.port,
        config.motd.as_deref(),
        config.max_players,
    )
}

fn properties_of(config: config::Config) -> Vec<ServerProperty> {
    config
        .entries
        .into_iter()
        .map(|entry| ServerProperty {
            key: entry.key,
            value: entry.value,
        })
        .collect()
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn list_servers(state: State<AppState>) -> Result<Vec<Server>> {
    state.db.list_servers(&state.paths)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub fn list_server_software() -> Vec<software::Spec> {
    software::specs()
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn list_server_flavor_versions(
    state: State<'_, AppState>,
    flavor: String,
    version_id: String,
) -> Result<Vec<String>> {
    software::find(&flavor)?
        .versions(&state.network, &version_id)
        .await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn create_server(
    state: State<AppState>,
    name: String,
    flavor: String,
    version_id: String,
    flavor_version: Option<String>,
    accept_eula: bool,
) -> Result<Server> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::other("Give the server a name first."));
    }
    let flavor = software::find(&flavor)?;
    if !flavor.native() && !accept_eula {
        return Err(Error::other(
            "The Minecraft EULA has to be accepted before a server can be created.",
        ));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let dir = state
        .paths
        .server_dir_checked(&id)
        .ok_or_else(|| Error::other("invalid server id"))?;
    state.files.ensure_dir(&dir)?;
    if !flavor.native() {
        provision::write_eula(&state.files, &dir)?;
    }

    let server = Server {
        id,
        name: name.to_string(),
        flavor,
        version_id,
        created_at: chrono::Utc::now(),
        managed: true,
        dir: dir.display().to_string(),
        available: true,
        flavor_version,
        launch_jar: None,
        launch_argfiles: Vec::new(),
        min_memory_mb: None,
        max_memory_mb: None,
        java_path: None,
        jvm_args: None,
        jvm_args_mode: None,
        stop_timeout_secs: None,
        eula_accepted_at: Some(chrono::Utc::now().timestamp()),
        installed_at: None,
        last_started_at: None,
        uptime_secs: 0,
        port: None,
        motd: None,
        max_players: None,
        notes: None,
        pack_provider: None,
        pack_project_id: None,
        pack_version_id: None,
    };
    state.db.insert_server(&server)?;
    tracing::info!(server_id = %server.id, flavor = flavor.id(), version = %server.version_id, "server created");
    Ok(server)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn inspect_server_folder(state: State<AppState>, path: String) -> Result<ServerFolder> {
    let dir = import::validate(&state.paths, &PathBuf::from(path.trim()))?;
    Ok(import::inspect(&dir))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn import_server(
    state: State<AppState>,
    path: String,
    name: String,
    flavor: String,
    version_id: String,
    flavor_version: Option<String>,
    accept_eula: bool,
) -> Result<Server> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::other("Give the server a name first."));
    }
    let dir = import::validate(&state.paths, &PathBuf::from(path.trim()))?;
    if state
        .db
        .imported_server_dirs()?
        .iter()
        .any(|known| known == &dir)
    {
        return Err(Error::other("That folder is already in Basalt."));
    }
    let folder = import::inspect(&dir);
    if !folder.eula_accepted && !accept_eula {
        return Err(Error::other(
            "The Minecraft EULA has to be accepted before this server can run.",
        ));
    }

    let server = Server {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        flavor: software::find(&flavor)?,
        version_id,
        created_at: chrono::Utc::now(),
        managed: false,
        dir: dir.display().to_string(),
        available: true,
        flavor_version,
        launch_jar: folder.launch_jar,
        launch_argfiles: folder.launch_argfiles,
        min_memory_mb: None,
        max_memory_mb: None,
        java_path: None,
        jvm_args: None,
        jvm_args_mode: None,
        stop_timeout_secs: None,
        eula_accepted_at: Some(chrono::Utc::now().timestamp()),
        installed_at: Some(chrono::Utc::now().timestamp()),
        last_started_at: None,
        uptime_secs: 0,
        port: folder.port,
        motd: None,
        max_players: None,
        notes: None,
        pack_provider: None,
        pack_project_id: None,
        pack_version_id: None,
    };
    state.db.insert_server(&server)?;
    crate::servers::adopt_imported_dirs(&state)?;
    if !folder.eula_accepted {
        provision::write_eula(&state.files, &dir)?;
    }
    tracing::info!(server_id = %server.id, dir = %server.dir, "server imported");
    Ok(server)
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn install_server(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> Result<Server> {
    let server = super::find_server(&state, &server_id)?;
    reachable(&server)?;
    let task = state.tasks.start(
        &app,
        TaskKind::ServerInstall,
        TaskSpec {
            title: server.name.clone(),
            subtitle: Some(format!("{} {}", server.flavor.label(), server.version_id)),
            server_id: Some(server.id.clone()),
            total: 1,
            ..Default::default()
        },
    )?;
    let result = provision::install(&state, &server, &task).await;
    task.finish(&result);
    let provisioned = result?;

    state.db.set_server_launch(
        &server.id,
        provisioned.launch_jar.as_deref(),
        &provisioned.launch_argfiles,
        provisioned.flavor_version.as_deref(),
        chrono::Utc::now().timestamp(),
    )?;
    super::find_server(&state, &server_id)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
#[allow(clippy::too_many_arguments)]
pub fn update_server_settings(
    state: State<AppState>,
    server_id: String,
    name: String,
    version_id: String,
    flavor_version: Option<String>,
    min_memory_mb: Option<u32>,
    max_memory_mb: Option<u32>,
    java_path: Option<String>,
    jvm_args: Option<String>,
    jvm_args_mode: Option<String>,
    stop_timeout_secs: Option<u32>,
    notes: Option<String>,
) -> Result<Server> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::other("Give the server a name first."));
    }
    let version_id = version_id.trim();
    if version_id.is_empty() {
        return Err(Error::other("Pick a Minecraft version first."));
    }
    if let (Some(min), Some(max)) = (min_memory_mb, max_memory_mb) {
        crate::config::MemoryLimits::new(min, max)?;
    }
    let current = super::find_server(&state, &server_id)?;
    let reinstall = current.version_id != version_id
        || current.flavor_version.as_deref() != flavor_version.as_deref();
    if reinstall
        && state
            .servers
            .lock()
            .unwrap()
            .get(&server_id)
            .is_some_and(runtime::ServerHandle::live)
    {
        return Err(Error::other("Stop the server before changing its version."));
    }
    state.db.update_server_settings(
        &server_id,
        name,
        version_id,
        flavor_version,
        min_memory_mb,
        max_memory_mb,
        java_path,
        jvm_args,
        jvm_args_mode,
        stop_timeout_secs,
        notes,
    )?;
    if reinstall {
        state.db.clear_server_launch(&server_id)?;
    }
    super::find_server(&state, &server_id)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn get_server_launch_command(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<String> {
    let server = super::find_server(&state, &server_id)?;
    runtime::launch_preview(&state, &server).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn accept_server_eula(state: State<AppState>, server_id: String) -> Result<Server> {
    let server = super::find_server(&state, &server_id)?;
    let dir = reachable(&server)?;
    provision::write_eula(&state.files, &dir)?;
    state
        .db
        .accept_server_eula(&server_id, chrono::Utc::now().timestamp())?;
    super::find_server(&state, &server_id)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_server(state: State<AppState>, server_id: String, delete_files: bool) -> Result<()> {
    let server = super::find_server(&state, &server_id)?;
    if state
        .servers
        .lock()
        .unwrap()
        .get(&server_id)
        .is_some_and(runtime::ServerHandle::live)
    {
        return Err(Error::other("Stop the server before deleting it."));
    }
    if delete_files {
        if !server.managed {
            return Err(Error::other(
                "Basalt does not own this folder, so it will not delete it.",
            ));
        }
        state
            .files
            .remove_managed_dir_all_if_exists(PathBuf::from(&server.dir))?;
    }
    state.db.forget_server_content(&server_id)?;
    state.db.delete_server(&server_id)?;
    runtime::forget(&state, &server_id);
    crate::servers::adopt_imported_dirs(&state)?;
    tracing::info!(server_id = %server_id, delete_files, "server removed");
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn start_server(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> Result<runtime::ServerRunningInfo> {
    let server = super::find_server(&state, &server_id)?;
    runtime::start(&app, &state, &server).await
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn stop_server(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> Result<()> {
    let server = super::find_server(&state, &server_id)?;
    runtime::stop(&app, &state, &server).await
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn restart_server(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> Result<runtime::ServerRunningInfo> {
    let server = super::find_server(&state, &server_id)?;
    if state
        .servers
        .lock()
        .unwrap()
        .get(&server_id)
        .is_some_and(runtime::ServerHandle::live)
    {
        runtime::stop(&app, &state, &server).await?;
    }
    runtime::start(&app, &state, &server).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn force_stop_server(state: State<AppState>, server_id: String) -> Result<()> {
    runtime::force_stop(&state, &server_id)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn get_server_disk_usage(state: State<AppState>, server_id: String) -> Result<u64> {
    let server = super::find_server(&state, &server_id)?;
    let dir = reachable(&server)?;
    Ok(crate::servers::files::disk_usage(&state.files, &dir))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn send_server_command(
    state: State<'_, AppState>,
    server_id: String,
    line: String,
) -> Result<()> {
    if line.trim().is_empty() {
        return Ok(());
    }
    runtime::send_command(&state, &server_id, &line).await
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub fn get_server_console(state: State<AppState>, server_id: String) -> Vec<runtime::ConsoleLine> {
    runtime::console(&state, &server_id)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub fn list_running_servers(state: State<AppState>) -> Vec<runtime::ServerRunningInfo> {
    runtime::running(&state)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn get_server_properties(
    state: State<AppState>,
    server_id: String,
) -> Result<Vec<ServerProperty>> {
    let server = super::find_server(&state, &server_id)?;
    reachable(&server)?;
    let config = config::read(&state.files, &server)?;
    cache_config(&state, &server, &config)?;
    Ok(properties_of(config))
}

#[tauri::command]
#[tracing::instrument(skip(state, changes), err)]
pub fn set_server_properties(
    state: State<AppState>,
    server_id: String,
    changes: Vec<ServerProperty>,
    removed: Vec<String>,
) -> Result<Vec<ServerProperty>> {
    let server = super::find_server(&state, &server_id)?;
    reachable(&server)?;
    let edits = changes
        .into_iter()
        .map(|property| config::Entry {
            key: property.key,
            value: property.value,
        })
        .collect::<Vec<_>>();
    let config = config::write(&state.files, &server, &edits, &removed)?;
    cache_config(&state, &server, &config)?;
    Ok(properties_of(config))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn list_server_content(
    state: State<'_, AppState>,
    server_id: String,
    reconcile: Option<bool>,
) -> Result<Vec<crate::content::ContentItem>> {
    let server = super::find_server(&state, &server_id)?;
    reachable(&server)?;
    if reconcile.unwrap_or(false) {
        crate::search::identify::reconcile(
            &state,
            crate::search::resolve::Target::Server(&server),
            "mods",
        )
        .await?;
    }
    let mut items = content::list(&state.files, &server)?;
    let mut sources: std::collections::HashMap<String, crate::db::ContentFile> = state
        .db
        .server_content_files(&server.id, "mods")?
        .into_iter()
        .map(|file| (file.file_name.clone(), file))
        .collect();
    let mut updates: std::collections::HashMap<String, crate::db::ContentUpdate> = state
        .db
        .server_content_updates(&server.id)?
        .into_iter()
        .map(|update| (update.file_name.clone(), update))
        .collect();
    for item in &mut items {
        item.source = sources.remove(&item.file_name);
        item.update = updates.remove(&item.file_name);
    }
    Ok(items)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn toggle_server_content(
    state: State<AppState>,
    server_id: String,
    file_name: String,
) -> Result<bool> {
    let server = super::find_server(&state, &server_id)?;
    reachable(&server)?;
    content::toggle(&state.files, &server, &file_name)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_server_content(
    state: State<AppState>,
    server_id: String,
    file_name: String,
) -> Result<()> {
    let server = super::find_server(&state, &server_id)?;
    reachable(&server)?;
    content::delete(&state.files, &server, &file_name)?;
    state
        .db
        .delete_server_content_file(&server.id, "mods", &file_name)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn add_server_content(
    state: State<AppState>,
    server_id: String,
    sources: Vec<String>,
) -> Result<usize> {
    let server = super::find_server(&state, &server_id)?;
    reachable(&server)?;
    content::add(&state.files, &server, &sources)
}

fn is_live(state: &AppState, server_id: &str) -> bool {
    state
        .servers
        .lock()
        .unwrap()
        .get(server_id)
        .is_some_and(runtime::ServerHandle::live)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_server_players(
    state: State<AppState>,
    server_id: String,
    list: String,
) -> Result<Vec<players::PlayerEntry>> {
    let server = super::find_server(&state, &server_id)?;
    let dir = reachable(&server)?;
    Ok(players::read(
        &state.files,
        &dir,
        players::PlayerList::parse(&list)?,
    ))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn add_server_player(
    state: State<'_, AppState>,
    server_id: String,
    list: String,
    name: String,
    reason: Option<String>,
) -> Result<()> {
    let server = super::find_server(&state, &server_id)?;
    let dir = reachable(&server)?;
    let list = players::PlayerList::parse(&list)?;

    if is_live(&state, &server_id) {
        return runtime::send_command(
            &state,
            &server_id,
            &players::command_to_add(list, name.trim(), reason.as_deref()),
        )
        .await;
    }

    let (uuid, resolved) = players::look_up(&state.network, &name).await?;
    let mut entries = players::read(&state.files, &dir, list);
    if entries
        .iter()
        .any(|entry| entry.uuid.eq_ignore_ascii_case(&uuid))
    {
        return Err(Error::other(format!("{resolved} is already on this list.")));
    }
    entries.push(players::entry_for(list, uuid, resolved, reason));
    players::write(&state.files, &dir, list, &entries)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn remove_server_player(
    state: State<'_, AppState>,
    server_id: String,
    list: String,
    name: String,
) -> Result<()> {
    let server = super::find_server(&state, &server_id)?;
    let dir = reachable(&server)?;
    let list = players::PlayerList::parse(&list)?;

    if is_live(&state, &server_id) {
        return runtime::send_command(
            &state,
            &server_id,
            &players::command_to_remove(list, name.trim()),
        )
        .await;
    }

    let mut entries = players::read(&state.files, &dir, list);
    let before = entries.len();
    entries.retain(|entry| {
        !entry.name.eq_ignore_ascii_case(name.trim())
            && !entry.uuid.eq_ignore_ascii_case(name.trim())
    });
    if entries.len() == before {
        return Err(Error::other(format!("{name} is not on this list.")));
    }
    players::write(&state.files, &dir, list, &entries)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn set_server_whitelist(
    state: State<'_, AppState>,
    server_id: String,
    enabled: bool,
) -> Result<()> {
    let server = super::find_server(&state, &server_id)?;
    reachable(&server)?;
    if server.flavor.native() {
        return Err(Error::other(format!(
            "A {} server keeps this in {}.",
            server.flavor.label(),
            server.flavor.config_file()
        )));
    }

    if is_live(&state, &server_id) {
        let line = if enabled {
            "whitelist on"
        } else {
            "whitelist off"
        };
        runtime::send_command(&state, &server_id, line).await?;
    }

    let entry = config::Entry {
        key: "white-list".to_string(),
        value: enabled.to_string(),
    };
    config::write(&state.files, &server, &[entry], &[])?;
    Ok(())
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn install_server_pack(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    provider: String,
    project_id: String,
    version_id: String,
    manual_sources: Option<Vec<crate::modpack::ManualDownloadSource>>,
) -> Result<Server> {
    let manual_sources = manual_sources.unwrap_or_default();
    let provider = crate::search::Provider::parse(&provider)?;
    let task = state.tasks.start(
        &app,
        TaskKind::ServerInstall,
        TaskSpec {
            title: "Modpack server".to_string(),
            subtitle: None,
            icon_url: None,
            instance_id: None,
            server_id: None,
            project_id: Some(project_id.clone()),
            total: 0,
            total_bytes: 0,
        },
    )?;

    match pack::install(
        &app,
        &state,
        provider,
        &project_id,
        &version_id,
        &manual_sources,
        &task,
    )
    .await
    {
        Ok(server) => {
            task.succeed();
            Ok(server)
        }
        Err(error) => {
            task.fail(error.to_string());
            Err(error)
        }
    }
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
#[allow(clippy::too_many_arguments)]
pub async fn install_server_zip(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    name: String,
    url: Option<String>,
    local_path: Option<String>,
    file_name: String,
    sha1: Option<String>,
    size: Option<u64>,
) -> Result<Server> {
    let task = state.tasks.start(
        &app,
        TaskKind::ServerInstall,
        TaskSpec {
            title: name.clone(),
            subtitle: Some("server pack".to_string()),
            icon_url: None,
            instance_id: None,
            server_id: None,
            project_id: None,
            total: 0,
            total_bytes: 0,
        },
    )?;
    let source = zippack::Source {
        url,
        local_path,
        file_name,
        sha1,
        size,
    };
    match zippack::install(&state, &name, &source, &task).await {
        Ok(server) => {
            task.succeed();
            Ok(server)
        }
        Err(error) => {
            task.fail(error.to_string());
            Err(error)
        }
    }
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn check_server_pack_update(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<Option<crate::modpack::ModpackUpgrade>> {
    let server = super::find_server(&state, &server_id)?;
    let (Some(provider), Some(project_id), Some(current)) = (
        server.pack_provider.as_deref(),
        server.pack_project_id.as_deref(),
        server.pack_version_id.as_deref(),
    ) else {
        return Ok(None);
    };
    crate::modpack::update_between(
        &state,
        crate::search::Provider::parse(provider)?,
        project_id,
        current,
        None,
    )
    .await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn plan_server_content_removal(
    state: State<AppState>,
    server_id: String,
    file_name: String,
) -> Result<crate::search::resolve::RemovalPlan> {
    let server = super::find_server(&state, &server_id)?;
    reachable(&server)?;
    Ok(crate::search::resolve::plan_removal(
        &state,
        crate::search::resolve::Target::Server(&server),
        crate::search::ContentKind::Mod,
        &file_name,
    ))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn check_server_content_updates(
    state: State<'_, AppState>,
    server_id: String,
    force: Option<bool>,
) -> Result<Vec<crate::db::ContentUpdate>> {
    let server = super::find_server(&state, &server_id)?;
    reachable(&server)?;
    let checked_at = state.db.server_updates_checked_at(&server_id)?;
    if !force.unwrap_or(false) && !crate::search::updates::is_stale(checked_at) {
        return state.db.server_content_updates(&server_id);
    }
    crate::search::updates::check_server(&state, &server).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn plan_server_content_install(
    state: State<'_, AppState>,
    server_id: String,
    provider: String,
    project_id: String,
    version_id: Option<String>,
) -> Result<crate::search::resolve::InstallPlan> {
    let server = super::find_server(&state, &server_id)?;
    reachable(&server)?;
    crate::search::resolve::plan(
        &state,
        crate::search::Provider::parse(&provider)?,
        &project_id,
        crate::search::resolve::Target::Server(&server),
        crate::search::ContentKind::Mod,
        &server.version_id,
        Some(server.flavor.id()),
        version_id.as_deref(),
        true,
    )
    .await
}

#[tauri::command]
#[tracing::instrument(skip(state, app), err)]
pub async fn install_server_content(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    provider: String,
    project_id: String,
    version_id: Option<String>,
    with_dependencies: Option<bool>,
) -> Result<Vec<crate::search::resolve::InstalledItem>> {
    let server = super::find_server(&state, &server_id)?;
    reachable(&server)?;
    let provider = crate::search::Provider::parse(&provider)?;
    let kind = crate::search::ContentKind::Mod;
    let target = crate::search::resolve::Target::Server(&server);
    let plan = crate::search::resolve::plan(
        &state,
        provider,
        &project_id,
        target,
        kind,
        &server.version_id,
        Some(server.flavor.id()),
        version_id.as_deref(),
        with_dependencies.unwrap_or(true),
    )
    .await?;
    crate::search::resolve::apply(Some(&app), &state, &plan, provider, target, kind, None).await
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_server_files(
    state: State<AppState>,
    server_id: String,
    path: String,
) -> Result<Vec<ServerEntry>> {
    let server = super::find_server(&state, &server_id)?;
    let dir = reachable(&server)?;
    crate::servers::files::entries(&state.files, &dir, &path)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn read_server_file(
    state: State<AppState>,
    server_id: String,
    path: String,
) -> Result<ServerText> {
    let server = super::find_server(&state, &server_id)?;
    let dir = reachable(&server)?;
    crate::servers::files::read_text(&state.files, &dir, &path)
}

#[tauri::command]
#[tracing::instrument(skip(state, text), err)]
pub fn write_server_file(
    state: State<AppState>,
    server_id: String,
    path: String,
    text: String,
) -> Result<Option<TextProblem>> {
    let server = super::find_server(&state, &server_id)?;
    let dir = reachable(&server)?;
    let problem = crate::servers::files::write_text(&state.files, &dir, &path, &text)?;
    if problem.is_none() && path.trim_matches('/') == server.flavor.config_file() {
        let config = config::read(&state.files, &server)?;
        cache_config(&state, &server, &config)?;
    }
    Ok(problem)
}

#[tauri::command]
#[tracing::instrument(skip_all)]
pub fn check_server_file(path: String, text: String) -> Option<TextProblem> {
    crate::servers::files::validate(FileKind::of(&path), &text)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn create_server_folder(
    state: State<AppState>,
    server_id: String,
    path: String,
    name: String,
) -> Result<String> {
    let server = super::find_server(&state, &server_id)?;
    let dir = reachable(&server)?;
    crate::servers::files::create_dir(&state.files, &dir, &path, &name)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn rename_server_entry(
    state: State<AppState>,
    server_id: String,
    path: String,
    name: String,
) -> Result<String> {
    let server = super::find_server(&state, &server_id)?;
    let dir = reachable(&server)?;
    crate::servers::files::rename(&state.files, &dir, &path, &name)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_server_entry(state: State<AppState>, server_id: String, path: String) -> Result<()> {
    let server = super::find_server(&state, &server_id)?;
    let dir = reachable(&server)?;
    crate::servers::files::delete(&state.files, &dir, &path)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn upload_server_files(
    state: State<AppState>,
    server_id: String,
    path: String,
    sources: Vec<String>,
) -> Result<usize> {
    let server = super::find_server(&state, &server_id)?;
    let dir = reachable(&server)?;
    crate::servers::files::upload(&state.files, &dir, &path, &sources)
}
