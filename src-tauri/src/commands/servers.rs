use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    error::{Error, Result},
    servers::{
        files::{FileKind, ServerEntry, ServerText},
        import::{self, ServerFolder},
        properties::Properties,
        provision, runtime, Server, ServerFlavor, TextProblem,
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

fn cache_properties(state: &AppState, server: &Server, properties: &Properties) -> Result<()> {
    state.db.cache_server_properties(
        &server.id,
        properties
            .get("server-port")
            .and_then(|value| value.trim().parse().ok()),
        properties.get("motd"),
        properties
            .get("max-players")
            .and_then(|value| value.trim().parse().ok()),
    )
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn list_servers(state: State<AppState>) -> Result<Vec<Server>> {
    state.db.list_servers(&state.paths)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn list_server_flavor_versions(
    state: State<'_, AppState>,
    flavor: String,
    version_id: String,
) -> Result<Vec<String>> {
    provision::list_flavor_versions(&state.network, ServerFlavor::parse(&flavor)?, &version_id)
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
    if !accept_eula {
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
    provision::write_eula(&state.files, &dir)?;

    let server = Server {
        id,
        name: name.to_string(),
        flavor: ServerFlavor::parse(&flavor)?,
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
    };
    state.db.insert_server(&server)?;
    tracing::info!(server_id = %server.id, flavor = %flavor, version = %server.version_id, "server created");
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
        flavor: ServerFlavor::parse(&flavor)?,
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
            subtitle: Some(format!("{} {}", server.flavor.as_str(), server.version_id)),
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
    let dir = reachable(&server)?;
    let properties = runtime::read_properties(&state.files, &dir);
    cache_properties(&state, &server, &properties)?;
    Ok(properties
        .entries()
        .into_iter()
        .map(|(key, value)| ServerProperty {
            key: key.to_string(),
            value: value.to_string(),
        })
        .collect())
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
    let dir = reachable(&server)?;
    let mut properties = runtime::read_properties(&state.files, &dir);
    for property in &changes {
        properties.set(property.key.trim(), &property.value);
    }
    for key in &removed {
        properties.remove(key.trim());
    }
    state
        .files
        .write_atomic(dir.join("server.properties"), &properties.render())?;
    cache_properties(&state, &server, &properties)?;
    get_server_properties(state, server_id)
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
    if problem.is_none() && path.trim_matches('/') == "server.properties" {
        let properties = runtime::read_properties(&state.files, &dir);
        cache_properties(&state, &server, &properties)?;
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
