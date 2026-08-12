use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader},
    process::{ChildStdin, Command},
    sync::{mpsc, oneshot, Mutex as AsyncMutex},
};

use crate::{
    config::MemoryLimits,
    db::ActiveServerRun,
    error::{Error, Result},
    files::FileManager,
    install, java,
    launch::{
        identity::{
            kill_recovered_process, process_matches, run_marker, spawned_process_start, Identity,
        },
        render_placeholders, split_args,
    },
    state::AppState,
};

use super::{control, properties::Properties, provision, rescan, startup, Server};

const MAX_CONSOLE_LINES: usize = 6000;
const EVENT_BATCH_LINES: usize = 256;
const MAX_FLUSH_LINES: usize = 2000;
const FLUSH_INTERVAL: Duration = Duration::from_millis(60);
const LIVENESS_INTERVAL: Duration = Duration::from_secs(2);
const TAIL_INTERVAL: Duration = Duration::from_millis(500);
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(200);
const MIN_STOP_TIMEOUT_SECS: u32 = 5;
const MAX_STOP_TIMEOUT_SECS: u32 = 600;
const DEFAULT_PORT: u16 = 25565;

pub type Registry = Arc<Mutex<HashMap<String, ServerHandle>>>;

#[derive(Debug, Clone, Serialize)]
pub struct ConsoleLine {
    pub stream: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerRunningInfo {
    pub server_id: String,
    pub running_id: String,
    pub pid: u32,
    pub started_at: i64,
    pub state: String,
    pub exit_code: Option<i32>,
    pub attached: bool,
    pub supervised: bool,
}

#[derive(Debug, Clone)]
struct RunStatus {
    state: String,
    exit_code: Option<i32>,
}

enum Control {
    Attached {
        stdin: Arc<AsyncMutex<Option<ChildStdin>>>,
        kill: Option<oneshot::Sender<()>>,
    },
    Supervised {
        writer: Arc<AsyncMutex<control::client::Writer>>,
    },
    Recovered {
        process_started_at: u64,
        identity: Identity,
    },
}

pub struct ServerHandle {
    server_id: String,
    running_id: String,
    pid: u32,
    started_at: i64,
    status: Arc<Mutex<RunStatus>>,
    console: Arc<Mutex<Vec<ConsoleLine>>>,
    control: Control,
}

impl ServerHandle {
    pub fn info(&self) -> ServerRunningInfo {
        let status = self.status.lock().unwrap().clone();
        ServerRunningInfo {
            server_id: self.server_id.clone(),
            running_id: self.running_id.clone(),
            pid: self.pid,
            started_at: self.started_at,
            state: status.state,
            exit_code: status.exit_code,
            attached: matches!(
                self.control,
                Control::Attached { .. } | Control::Supervised { .. }
            ),
            supervised: matches!(self.control, Control::Supervised { .. }),
        }
    }

    pub fn live(&self) -> bool {
        matches!(
            self.status.lock().unwrap().state.as_str(),
            "running" | "stopping"
        )
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn emit_state(app: &AppHandle, info: ServerRunningInfo) {
    let _ = app.emit("server:state", info);
}

pub fn read_properties(files: &FileManager, dir: &Path) -> Properties {
    files
        .read(dir.join("server.properties"))
        .map(|bytes| Properties::parse(&bytes))
        .unwrap_or_default()
}

fn port_of(properties: &Properties, key: &str, fallback: Option<u16>) -> Option<u16> {
    properties
        .get(key)
        .and_then(|value| value.trim().parse::<u16>().ok())
        .or(fallback)
}

fn enabled(properties: &Properties, key: &str) -> bool {
    properties
        .get(key)
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
}

pub fn configured_port(properties: &Properties) -> u16 {
    port_of(properties, "server-port", Some(DEFAULT_PORT)).unwrap_or(DEFAULT_PORT)
}

pub fn server_port(files: &FileManager, server: &Server) -> u16 {
    let dir = PathBuf::from(&server.dir);
    server.flavor.port(files, &dir).unwrap_or(DEFAULT_PORT)
}

fn claimed_ports(properties: &Properties) -> Vec<(&'static str, u16)> {
    let mut ports = vec![("server-port", configured_port(properties))];
    if enabled(properties, "enable-query") {
        if let Some(port) = port_of(properties, "query.port", None) {
            ports.push(("query.port", port));
        }
    }
    if enabled(properties, "enable-rcon") {
        if let Some(port) = port_of(properties, "rcon.port", None) {
            ports.push(("rcon.port", port));
        }
    }
    ports
}

fn port_is_free(port: u16) -> bool {
    std::net::TcpListener::bind(("0.0.0.0", port)).is_ok()
}

fn jvm_template(settings: &crate::config::LauncherSettings, server: &Server) -> String {
    let base = if settings.server_jvm_args.trim().is_empty() {
        crate::config::DEFAULT_JVM_ARGS
    } else {
        settings.server_jvm_args.as_str()
    };
    let extra = server.jvm_args.as_deref().unwrap_or("").trim();
    if extra.is_empty() {
        return base.to_string();
    }
    if matches!(server.jvm_args_mode.as_deref(), Some("replace")) {
        extra.to_string()
    } else {
        format!("{base} {extra}")
    }
}

fn server_memory(
    settings: &crate::config::LauncherSettings,
    server: &Server,
) -> Result<MemoryLimits> {
    MemoryLimits::new(
        server
            .min_memory_mb
            .unwrap_or(settings.server_min_memory_mb),
        server
            .max_memory_mb
            .unwrap_or(settings.server_max_memory_mb),
    )
}

fn stop_timeout(settings: &crate::config::LauncherSettings, server: &Server) -> Duration {
    let secs = server
        .stop_timeout_secs
        .unwrap_or(settings.server_stop_timeout_secs)
        .clamp(MIN_STOP_TIMEOUT_SECS, MAX_STOP_TIMEOUT_SECS);
    Duration::from_secs(u64::from(secs))
}

fn native_binary(server: &Server) -> Result<PathBuf> {
    let name = server
        .launch_jar
        .as_deref()
        .ok_or_else(|| Error::other("Install this server before starting it."))?;
    Ok(PathBuf::from(&server.dir).join(name))
}

fn identity_of(server: &Server, running_id: &str) -> Result<Identity> {
    if server.flavor.native() {
        Ok(Identity::executable(native_binary(server)?))
    } else {
        Ok(Identity::marker(running_id))
    }
}

fn launch_arguments(
    settings: &crate::config::LauncherSettings,
    server: &Server,
    memory: MemoryLimits,
    running_id: &str,
) -> Result<Vec<String>> {
    let placeholders = HashMap::from([
        ("min_ram", memory.min_mb.to_string()),
        ("max_ram", memory.max_mb.to_string()),
        ("server_name", server.name.clone()),
        ("server_dir", server.dir.clone()),
        ("version", server.version_id.clone()),
    ]);
    let mut args = split_args(&render_placeholders(
        &jvm_template(settings, server),
        &placeholders,
    ));
    args.push(run_marker(running_id));

    if !server.launch_argfiles.is_empty() {
        for argfile in &server.launch_argfiles {
            args.push(format!("@{}", provision::argfile_for_platform(argfile)));
        }
    } else {
        let jar = server
            .launch_jar
            .as_deref()
            .ok_or_else(|| Error::other("Install this server before starting it."))?;
        args.push("-jar".to_string());
        args.push(jar.to_string());
    }

    args.extend(
        server
            .flavor
            .launch_args()
            .iter()
            .map(|argument| (*argument).to_string()),
    );
    Ok(args)
}

pub async fn launch_preview(state: &AppState, server: &Server) -> Result<String> {
    if let Some(script) = server
        .launch_script
        .as_deref()
        .filter(|_| !server.skip_launch_script)
    {
        let dir = PathBuf::from(&server.dir);
        let (shell, args) = startup::command(&dir, script);
        let mut line = quoted(&shell.display().to_string());
        for argument in args {
            line.push(' ');
            line.push_str(&quoted(&argument));
        }
        return Ok(line);
    }
    if server.flavor.native() {
        return Ok(quoted(&native_binary(server)?.display().to_string()));
    }
    let settings = state.runtime_settings()?;
    let memory = server_memory(&settings, server)?;
    let version = install::load_merged_version(state, &server.version_id).await?;
    let java = java::find_for_major(
        &state.files,
        version.required_java_major(),
        server.java_path.as_deref(),
    )
    .await
    .ok_or_else(|| {
        Error::other(format!(
            "No Java {} found for this server.",
            version.required_java_major()
        ))
    })?;
    let running_id = uuid::Uuid::new_v4().to_string();
    let args = launch_arguments(&settings, server, memory, &running_id)?;
    let mut line = quoted(&java.path);
    for argument in &args {
        line.push(' ');
        line.push_str(&quoted(argument));
    }
    Ok(line)
}

fn quoted(value: &str) -> String {
    if value.contains(char::is_whitespace) {
        format!("\"{value}\"")
    } else {
        value.to_string()
    }
}

fn push_console(
    app: &AppHandle,
    server_id: &str,
    console: &Arc<Mutex<Vec<ConsoleLine>>>,
    batch: Vec<(&'static str, String)>,
) {
    if batch.is_empty() {
        return;
    }
    {
        let mut buffer = console.lock().unwrap();
        buffer.extend(batch.iter().map(|(stream, line)| ConsoleLine {
            stream: stream.to_string(),
            line: line.clone(),
        }));
        if buffer.len() > MAX_CONSOLE_LINES {
            let overflow = buffer.len() - MAX_CONSOLE_LINES;
            buffer.drain(0..overflow);
        }
    }

    let mut start = 0;
    while start < batch.len() {
        let stream = batch[start].0;
        let mut end = start;
        while end < batch.len() && batch[end].0 == stream && end - start < EVENT_BATCH_LINES {
            end += 1;
        }
        let lines = batch[start..end]
            .iter()
            .map(|(_, line)| line.clone())
            .collect::<Vec<_>>();
        let _ = app.emit(
            "server:log",
            json!({ "server_id": server_id, "stream": stream, "lines": lines }),
        );
        start = end;
    }
}

fn spawn_reader<R>(
    stream: &'static str,
    reader: R,
    sender: mpsc::UnboundedSender<(&'static str, String)>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if sender.send((stream, line)).is_err() {
                break;
            }
        }
    });
}

fn spawn_console_flusher(
    app: AppHandle,
    server_id: String,
    console: Arc<Mutex<Vec<ConsoleLine>>>,
    mut receiver: mpsc::UnboundedReceiver<(&'static str, String)>,
) {
    tauri::async_runtime::spawn(async move {
        let mut batch: Vec<(&'static str, String)> = Vec::new();
        let mut ticker = tokio::time::interval(FLUSH_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                received = receiver.recv() => match received {
                    Some(line) => {
                        batch.push(line);
                        if batch.len() >= EVENT_BATCH_LINES {
                            push_console(&app, &server_id, &console, std::mem::take(&mut batch));
                        }
                    }
                    None => {
                        push_console(&app, &server_id, &console, std::mem::take(&mut batch));
                        break;
                    }
                },
                _ = ticker.tick() => {
                    if batch.len() > MAX_FLUSH_LINES {
                        let dropped = batch.len() - MAX_FLUSH_LINES;
                        batch.drain(0..dropped);
                        batch.insert(
                            0,
                            ("stdout", format!("[basalt] dropped {dropped} lines, the server is writing faster than the console can read")),
                        );
                    }
                    push_console(&app, &server_id, &console, std::mem::take(&mut batch));
                }
            }
        }
    });
}

#[tracing::instrument(skip_all, fields(server_id = %server.id), err)]
pub async fn start(
    app: &AppHandle,
    state: &AppState,
    server: &Server,
) -> Result<ServerRunningInfo> {
    if state
        .servers
        .lock()
        .unwrap()
        .get(&server.id)
        .is_some_and(ServerHandle::live)
    {
        return Err(Error::other("This server is already running."));
    }

    let dir = PathBuf::from(&server.dir);
    if !dir.is_dir() {
        return Err(Error::other(format!(
            "Basalt cannot reach {}. Plug the drive back in or remove the server.",
            server.dir
        )));
    }
    if !server.flavor.native() && !provision::eula_accepted(&state.files, &dir) {
        return Err(Error::other(
            "Accept the Minecraft EULA before starting this server.",
        ));
    }
    let script = server
        .launch_script
        .as_deref()
        .filter(|_| !server.skip_launch_script);
    let mut server = server.clone();
    if script.is_none() && server.launch_jar.is_none() && server.launch_argfiles.is_empty() {
        let task = state.tasks.start(
            app,
            crate::tasks::TaskKind::ServerInstall,
            crate::tasks::TaskSpec {
                title: server.name.clone(),
                subtitle: Some(format!("{} {}", server.flavor.label(), server.version_id)),
                server_id: Some(server.id.clone()),
                total: 1,
                ..Default::default()
            },
        )?;
        let result = provision::install(state, &server, &task).await;
        task.finish(&result);
        let provisioned = result?;
        state.db.set_server_launch(
            &server.id,
            provisioned.launch_jar.as_deref(),
            &provisioned.launch_argfiles,
            provisioned.flavor_version.as_deref(),
            chrono::Utc::now().timestamp(),
        )?;
        server.launch_jar = provisioned.launch_jar;
        server.launch_argfiles = provisioned.launch_argfiles;
        server.flavor_version = provisioned.flavor_version.or(server.flavor_version);
    }
    let server = &server;

    let ports = if server.flavor.native() {
        vec![("the server port", server_port(&state.files, server))]
    } else {
        claimed_ports(&read_properties(&state.files, &dir))
    };
    for (key, port) in ports {
        if !port_is_free(port) {
            return Err(Error::other(format!(
                "Port {port} is already taken, so {key} cannot be used. Change it or stop whatever is holding it."
            )));
        }
    }

    let settings = state.runtime_settings()?;
    let running_id = uuid::Uuid::new_v4().to_string();
    let (program, args) = if let Some(script) = script {
        let (shell, shell_args) = startup::command(&dir, script);
        tracing::info!(script, "starting this server through the script it ships");
        (shell, shell_args)
    } else if server.flavor.native() {
        (native_binary(server)?, Vec::new())
    } else {
        let memory = server_memory(&settings, server)?;
        let version = install::load_merged_version(state, &server.version_id).await?;
        let java = java::find_for_major(
            &state.files,
            version.required_java_major(),
            server.java_path.as_deref(),
        )
        .await
        .ok_or_else(|| {
            Error::other(format!(
                "No Java {} found for this server.",
                version.required_java_major()
            ))
        })?;
        (
            PathBuf::from(&java.path),
            launch_arguments(&settings, server, memory, &running_id)?,
        )
    };
    let started_at = now();
    let console = Arc::new(Mutex::new(Vec::new()));
    let status = Arc::new(Mutex::new(RunStatus {
        state: "running".to_string(),
        exit_code: None,
    }));

    tracing::info!(program = %program.display(), dir = %dir.display(), "starting server");
    let supervised = control::launch::start(
        &server.id,
        &state.paths.servers(),
        &dir,
        &program,
        &args,
        script.is_some(),
    )
    .await?;

    let pid;
    let control = match supervised {
        control::launch::Outcome::Supervised(session) => {
            pid = session.pid;
            let process_started_at = session.started_at;
            let (writer, reader) = session.split();

            record_run(
                state,
                server,
                &running_id,
                pid,
                process_started_at,
                started_at,
            )?;

            let (sender, receiver) = mpsc::unbounded_channel();
            spawn_console_flusher(app.clone(), server.id.clone(), console.clone(), receiver);
            let watcher = Supervisor {
                app: app.clone(),
                db: state.db.clone(),
                registry: state.servers.clone(),
                server_id: server.id.clone(),
                running_id: running_id.clone(),
                started_at,
                pid,
                status: status.clone(),
                console: console.clone(),
            };
            watcher.follow(reader, sender);

            Control::Supervised {
                writer: Arc::new(AsyncMutex::new(writer)),
            }
        }
        control::launch::Outcome::Unavailable(reason) => {
            tracing::warn!(%reason, "running this server without a supervisor");
            let mut child = Command::new(&program)
                .args(&args)
                .current_dir(&dir)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .inspect_err(
                    |error| tracing::error!(%error, "could not spawn the server process"),
                )?;

            pid = child.id().unwrap_or(0);
            let identity = identity_of(server, &running_id)?;
            let Some(process_started_at) = spawned_process_start(pid, &identity) else {
                let _ = child.start_kill();
                return Err(Error::other("Basalt could not verify the server process."));
            };

            record_run(
                state,
                server,
                &running_id,
                pid,
                process_started_at,
                started_at,
            )?;

            let (sender, receiver) = mpsc::unbounded_channel();
            if let Some(stdout) = child.stdout.take() {
                spawn_reader("stdout", stdout, sender.clone());
            }
            if let Some(stderr) = child.stderr.take() {
                spawn_reader("stderr", stderr, sender);
            }
            spawn_console_flusher(app.clone(), server.id.clone(), console.clone(), receiver);

            let stdin = Arc::new(AsyncMutex::new(child.stdin.take()));
            let (kill_tx, kill_rx) = oneshot::channel::<()>();
            let watcher = Supervisor {
                app: app.clone(),
                db: state.db.clone(),
                registry: state.servers.clone(),
                server_id: server.id.clone(),
                running_id: running_id.clone(),
                started_at,
                pid,
                status: status.clone(),
                console: console.clone(),
            };
            watcher.watch(child, kill_rx);

            Control::Attached {
                stdin,
                kill: Some(kill_tx),
            }
        }
    };

    let handle = ServerHandle {
        server_id: server.id.clone(),
        running_id,
        pid,
        started_at,
        status,
        console,
        control,
    };
    let info = handle.info();
    state
        .servers
        .lock()
        .unwrap()
        .insert(server.id.clone(), handle);
    emit_state(app, info.clone());

    if script.is_some() {
        match rescan::run(state, server) {
            Ok(found) if found.changed => {
                let _ = app.emit("server:rescanned", json!({ "server_id": server.id }));
            }
            Ok(_) => {}
            Err(error) => tracing::warn!(%error, "could not read the server folder again"),
        }
    }

    Ok(info)
}

fn record_run(
    state: &AppState,
    server: &Server,
    running_id: &str,
    pid: u32,
    process_started_at: u64,
    started_at: i64,
) -> Result<()> {
    state.db.save_active_server_run(&ActiveServerRun {
        running_id: running_id.to_string(),
        server_id: server.id.clone(),
        pid,
        process_started_at,
        started_at,
    })?;
    state.db.start_server_run(&server.id, started_at)
}

struct Supervisor {
    app: AppHandle,
    db: crate::db::Db,
    registry: Registry,
    server_id: String,
    running_id: String,
    started_at: i64,
    pid: u32,
    status: Arc<Mutex<RunStatus>>,
    console: Arc<Mutex<Vec<ConsoleLine>>>,
}

impl Supervisor {
    fn watch(self, mut child: tokio::process::Child, kill: oneshot::Receiver<()>) {
        tauri::async_runtime::spawn(async move {
            let exit = tokio::select! {
                result = child.wait() => result,
                _ = kill => {
                    let _ = child.start_kill();
                    child.wait().await
                }
            };
            self.finish(exit.ok().and_then(|status| status.code()));
        });
    }

    fn follow(
        self,
        mut reader: control::client::Reader,
        sender: mpsc::UnboundedSender<(&'static str, String)>,
    ) {
        tauri::async_runtime::spawn(async move {
            let mut code = None;
            while let Some(event) = reader.next().await {
                match event {
                    control::Event::Log { stream, line } => {
                        let stream = if stream == "stderr" {
                            "stderr"
                        } else {
                            "stdout"
                        };
                        if sender.send((stream, line)).is_err() {
                            break;
                        }
                    }
                    control::Event::Exit { code: reported } => {
                        code = reported;
                        break;
                    }
                    control::Event::Ready { .. } | control::Event::Refused { .. } => {}
                }
            }
            self.finish(code);
        });
    }

    fn finish(self, code: Option<i32>) {
        let state = if matches!(code, Some(0) | None) {
            "exited"
        } else {
            "crashed"
        };
        {
            let mut guard = self.status.lock().unwrap();
            guard.state = state.to_string();
            guard.exit_code = code;
        }

        let ended_at = now();
        if state == "crashed" {
            let tail = {
                let console = self.console.lock().unwrap();
                console
                    .iter()
                    .rev()
                    .take(12)
                    .map(|line| line.line.clone())
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            tracing::error!(
                server_id = %self.server_id,
                pid = self.pid,
                exit_code = ?code,
                "server exited abnormally:\n{tail}"
            );
        } else {
            tracing::info!(server_id = %self.server_id, pid = self.pid, exit_code = ?code, "server stopped");
        }

        if let Err(error) = self
            .db
            .add_server_uptime(&self.server_id, ended_at - self.started_at)
        {
            tracing::warn!(%error, "could not record the server uptime");
        }
        if let Err(error) = self.db.clear_active_server_run(&self.running_id) {
            tracing::warn!(%error, "could not clear the finished server run");
        }

        let info = self
            .registry
            .lock()
            .unwrap()
            .get(&self.server_id)
            .map(ServerHandle::info);
        if let Some(info) = info {
            emit_state(&self.app, info);
        }
    }
}

enum Reach {
    Pipe(Arc<AsyncMutex<Option<ChildStdin>>>),
    Supervisor(Arc<AsyncMutex<control::client::Writer>>),
}

pub async fn send_command(state: &AppState, server_id: &str, line: &str) -> Result<()> {
    let stdin = {
        let registry = state.servers.lock().unwrap();
        let handle = registry
            .get(server_id)
            .ok_or_else(|| Error::other("This server is not running."))?;
        if !handle.live() {
            return Err(Error::other("This server is not running."));
        }
        match &handle.control {
            Control::Attached { stdin, .. } => Reach::Pipe(stdin.clone()),
            Control::Supervised { writer, .. } => Reach::Supervisor(writer.clone()),
            Control::Recovered { .. } => {
                return Err(Error::other(
                    "This server was started before Basalt restarted, so commands cannot reach it. Restart it from Basalt to get the console back.",
                ))
            }
        }
    };

    let stdin = match stdin {
        Reach::Supervisor(writer) => {
            return writer
                .lock()
                .await
                .send(&control::Request::Send {
                    line: line.trim_end().to_string(),
                })
                .await;
        }
        Reach::Pipe(stdin) => stdin,
    };

    let mut guard = stdin.lock().await;
    let pipe = guard
        .as_mut()
        .ok_or_else(|| Error::other("This server is not reading commands."))?;
    pipe.write_all(line.trim_end().as_bytes()).await?;
    pipe.write_all(b"\n").await?;
    pipe.flush().await?;
    Ok(())
}

pub async fn stop(app: &AppHandle, state: &AppState, server: &Server) -> Result<()> {
    let (stdin, recovered) = {
        let mut registry = state.servers.lock().unwrap();
        let handle = registry
            .get_mut(&server.id)
            .ok_or_else(|| Error::other("This server is not running."))?;
        if !handle.live() {
            return Err(Error::other("This server is not running."));
        }
        handle.status.lock().unwrap().state = "stopping".to_string();
        match &handle.control {
            Control::Attached { stdin, .. } => (Some(Reach::Pipe(stdin.clone())), false),
            Control::Supervised { writer, .. } => (Some(Reach::Supervisor(writer.clone())), false),
            Control::Recovered { .. } => (None, true),
        }
    };

    if let Some(info) = state
        .servers
        .lock()
        .unwrap()
        .get(&server.id)
        .map(ServerHandle::info)
    {
        emit_state(app, info);
    }

    if recovered {
        return force_stop(state, &server.id);
    }

    match stdin {
        Some(Reach::Supervisor(writer)) => {
            writer.lock().await.send(&control::Request::Stop).await?;
        }
        Some(Reach::Pipe(stdin)) => {
            let mut guard = stdin.lock().await;
            if let Some(pipe) = guard.as_mut() {
                pipe.write_all(b"stop\n").await?;
                pipe.flush().await?;
            }
        }
        None => {}
    }

    let deadline = tokio::time::Instant::now() + stop_timeout(&state.runtime_settings()?, server);
    while tokio::time::Instant::now() < deadline {
        if !state
            .servers
            .lock()
            .unwrap()
            .get(&server.id)
            .is_some_and(ServerHandle::live)
        {
            return Ok(());
        }
        tokio::time::sleep(STOP_POLL_INTERVAL).await;
    }

    tracing::warn!(server_id = %server.id, "the server ignored stop, killing it");
    force_stop(state, &server.id)
}

pub fn force_stop(state: &AppState, server_id: &str) -> Result<()> {
    let mut registry = state.servers.lock().unwrap();
    let handle = registry
        .get_mut(server_id)
        .ok_or_else(|| Error::other("This server is not running."))?;
    match &mut handle.control {
        Control::Attached { kill, .. } => {
            kill.take().map(|tx| tx.send(()).is_ok());
        }
        Control::Supervised { writer, .. } => {
            let writer = writer.clone();
            tauri::async_runtime::spawn(async move {
                let _ = writer.lock().await.send(&control::Request::Kill).await;
            });
        }
        Control::Recovered {
            process_started_at,
            identity,
        } => {
            kill_recovered_process(handle.pid, *process_started_at, identity);
        }
    }
    Ok(())
}

pub fn console(state: &AppState, server_id: &str) -> Vec<ConsoleLine> {
    state
        .servers
        .lock()
        .unwrap()
        .get(server_id)
        .map(|handle| handle.console.lock().unwrap().clone())
        .unwrap_or_default()
}

pub fn running(state: &AppState) -> Vec<ServerRunningInfo> {
    state
        .servers
        .lock()
        .unwrap()
        .values()
        .map(ServerHandle::info)
        .collect()
}

pub fn forget(state: &AppState, server_id: &str) {
    state.servers.lock().unwrap().remove(server_id);
}

#[tracing::instrument(skip_all, err)]
pub fn recover(app: &AppHandle, state: &AppState) -> Result<usize> {
    let mut recovered = 0;
    for run in state.db.active_server_runs()? {
        let Some(server) = state.db.server(&state.paths, &run.server_id)? else {
            state.db.clear_active_server_run(&run.running_id)?;
            continue;
        };
        if let Some(control) = control::client::read_for(&state.paths.servers(), &server.id) {
            if control.child_pid == run.pid {
                reconnect(app, state, &server, &run, control);
                recovered += 1;
                continue;
            }
        }

        let Ok(identity) = identity_of(&server, &run.running_id) else {
            state.db.clear_active_server_run(&run.running_id)?;
            continue;
        };
        if !process_matches(run.pid, run.process_started_at, &identity) {
            state.db.clear_active_server_run(&run.running_id)?;
            continue;
        }

        let console = Arc::new(Mutex::new(Vec::new()));
        let status = Arc::new(Mutex::new(RunStatus {
            state: "running".to_string(),
            exit_code: None,
        }));
        let handle = ServerHandle {
            server_id: server.id.clone(),
            running_id: run.running_id.clone(),
            pid: run.pid,
            started_at: run.started_at,
            status: status.clone(),
            console: console.clone(),
            control: Control::Recovered {
                process_started_at: run.process_started_at,
                identity: identity.clone(),
            },
        };
        let info = handle.info();
        state
            .servers
            .lock()
            .unwrap()
            .insert(server.id.clone(), handle);

        spawn_log_tailer(
            app.clone(),
            state.files.clone(),
            server.id.clone(),
            PathBuf::from(&server.dir).join("logs").join("latest.log"),
            status.clone(),
            console,
        );
        watch_recovered(
            app.clone(),
            state.db.clone(),
            state.servers.clone(),
            run,
            identity,
        );
        emit_state(app, info);
        recovered += 1;
    }
    if recovered > 0 {
        tracing::info!(recovered, "reattached running servers");
    }
    Ok(recovered)
}

fn reconnect(
    app: &AppHandle,
    state: &AppState,
    server: &Server,
    run: &ActiveServerRun,
    control: control::Control,
) {
    let app = app.clone();
    let db = state.db.clone();
    let registry = state.servers.clone();
    let server_id = server.id.clone();
    let running_id = run.running_id.clone();
    let started_at = run.started_at;
    let pid = run.pid;
    let dir = PathBuf::from(&server.dir);
    let files = state.files.clone();

    tauri::async_runtime::spawn(async move {
        let session = match control::client::Session::connect(&control).await {
            Ok(session) => session,
            Err(error) => {
                tracing::warn!(%error, server_id = %server_id, "could not reattach to the supervisor");
                return;
            }
        };
        let (writer, reader) = session.split();

        let console = Arc::new(Mutex::new(Vec::new()));
        let status = Arc::new(Mutex::new(RunStatus {
            state: "running".to_string(),
            exit_code: None,
        }));
        let handle = ServerHandle {
            server_id: server_id.clone(),
            running_id: running_id.clone(),
            pid,
            started_at,
            status: status.clone(),
            console: console.clone(),
            control: Control::Supervised {
                writer: Arc::new(AsyncMutex::new(writer)),
            },
        };
        let info = handle.info();
        registry.lock().unwrap().insert(server_id.clone(), handle);

        backfill(&files, &dir, &console);

        let (sender, receiver) = mpsc::unbounded_channel();
        spawn_console_flusher(app.clone(), server_id.clone(), console.clone(), receiver);
        let watcher = Supervisor {
            app: app.clone(),
            db,
            registry,
            server_id,
            running_id,
            started_at,
            pid,
            status,
            console,
        };
        watcher.follow(reader, sender);
        emit_state(&app, info);
    });
}

fn backfill(files: &FileManager, dir: &Path, console: &Arc<Mutex<Vec<ConsoleLine>>>) {
    let path = dir.join("logs").join("latest.log");
    let Ok(text) = files.read(&path) else {
        return;
    };
    let text = String::from_utf8_lossy(&text);
    let lines = text
        .lines()
        .rev()
        .take(MAX_CONSOLE_LINES)
        .map(|line| ConsoleLine {
            stream: "stdout".to_string(),
            line: line.to_string(),
        })
        .collect::<Vec<_>>();
    let mut buffer = console.lock().unwrap();
    buffer.extend(lines.into_iter().rev());
}

fn spawn_log_tailer(
    app: AppHandle,
    files: FileManager,
    server_id: String,
    path: PathBuf,
    status: Arc<Mutex<RunStatus>>,
    console: Arc<Mutex<Vec<ConsoleLine>>>,
) {
    tauri::async_runtime::spawn(async move {
        let mut offset = files
            .metadata(&path)
            .map(|meta| meta.len())
            .unwrap_or_default();
        let mut pending = Vec::new();
        loop {
            if let Ok(tail) = files.read_tail_async(&path, offset).await {
                if tail.restarted {
                    pending.clear();
                }
                pending.extend_from_slice(&tail.bytes);
                offset = tail.len;

                let mut batch = Vec::new();
                while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
                    let mut line = pending.drain(..=newline).collect::<Vec<_>>();
                    line.pop();
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    batch.push(("stdout", String::from_utf8_lossy(&line).into_owned()));
                }
                push_console(&app, &server_id, &console, batch);
            }

            if status.lock().unwrap().state != "running" {
                break;
            }
            tokio::time::sleep(TAIL_INTERVAL).await;
        }
    });
}

fn watch_recovered(
    app: AppHandle,
    db: crate::db::Db,
    registry: Registry,
    run: ActiveServerRun,
    identity: Identity,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(LIVENESS_INTERVAL).await;
            if process_matches(run.pid, run.process_started_at, &identity) {
                continue;
            }
            let info = {
                let guard = registry.lock().unwrap();
                let Some(handle) = guard.get(&run.server_id) else {
                    break;
                };
                {
                    let mut status = handle.status.lock().unwrap();
                    status.state = "exited".to_string();
                }
                handle.info()
            };
            if let Err(error) = db.add_server_uptime(&run.server_id, now() - run.started_at) {
                tracing::warn!(%error, "could not record the server uptime");
            }
            if let Err(error) = db.clear_active_server_run(&run.running_id) {
                tracing::warn!(%error, "could not clear the finished server run");
            }
            emit_state(&app, info);
            break;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> Server {
        Server {
            id: "s1".into(),
            name: "Survival".into(),
            flavor: crate::servers::software::find("paper").unwrap(),
            version_id: "1.21.8".into(),
            created_at: chrono::Utc::now(),
            managed: true,
            dir: "/data/servers/s1".into(),
            available: true,
            flavor_version: None,
            launch_jar: Some("server.jar".into()),
            launch_argfiles: Vec::new(),
            min_memory_mb: Some(1024),
            max_memory_mb: Some(4096),
            java_path: None,
            jvm_args: None,
            jvm_args_mode: None,
            stop_timeout_secs: None,
            eula_accepted_at: None,
            installed_at: None,
            last_started_at: None,
            uptime_secs: 0,
            port: None,
            motd: None,
            max_players: None,
            notes: None,
            launch_script: None,
            skip_launch_script: false,
            pack_provider: None,
            pack_project_id: None,
            pack_version_id: None,
            import_source: None,
            import_source_id: None,
        }
    }

    #[test]
    fn a_jar_server_is_launched_with_the_memory_it_was_given() {
        let memory = MemoryLimits::new(1024, 4096).unwrap();
        let settings = crate::config::LauncherSettings::default();
        let args = launch_arguments(&settings, &server(), memory, "run-1").unwrap();

        assert!(args.contains(&"-Xms1024M".to_string()));
        assert!(args.contains(&"-Xmx4096M".to_string()));
        assert!(args.contains(&run_marker("run-1")));
        assert_eq!(args[args.len() - 3], "-jar");
        assert_eq!(args[args.len() - 2], "server.jar");
        assert_eq!(args[args.len() - 1], "nogui");
    }

    #[test]
    fn an_argfile_server_never_gets_a_jar_argument() {
        let mut server = server();
        server.launch_jar = None;
        server.launch_argfiles = vec![
            "user_jvm_args.txt".into(),
            format!(
                "libraries/net/neoforged/neoforge/21.8.54/{}_args.txt",
                provision::PLATFORM_PLACEHOLDER
            ),
        ];
        let args = launch_arguments(
            &crate::config::LauncherSettings::default(),
            &server,
            MemoryLimits::new(1024, 4096).unwrap(),
            "run-2",
        )
        .unwrap();

        assert!(!args.iter().any(|arg| arg == "-jar"));
        assert!(args.contains(&"@user_jvm_args.txt".to_string()));
        assert!(args
            .iter()
            .any(|arg| arg.starts_with("@libraries/")
                && !arg.contains(provision::PLATFORM_PLACEHOLDER)));
        assert_eq!(args.last().unwrap(), "nogui");
    }

    #[test]
    fn server_jvm_arguments_add_to_or_replace_the_memory_flags() {
        let memory = MemoryLimits::new(512, 1024).unwrap();

        let mut added = server();
        added.jvm_args = Some("-XX:+UseG1GC".into());
        let settings = crate::config::LauncherSettings::default();
        let args = launch_arguments(&settings, &added, memory, "run-3").unwrap();
        assert!(args.contains(&"-Xmx1024M".to_string()));
        assert!(args.contains(&"-XX:+UseG1GC".to_string()));

        let mut replaced = added.clone();
        replaced.jvm_args_mode = Some("replace".into());
        let args = launch_arguments(&settings, &replaced, memory, "run-4").unwrap();
        assert!(!args.contains(&"-Xmx1024M".to_string()));
        assert!(args.contains(&"-XX:+UseG1GC".to_string()));
    }

    #[test]
    fn the_launcher_jvm_arguments_never_reach_a_server() {
        let memory = MemoryLimits::new(512, 1024).unwrap();
        let settings = crate::config::LauncherSettings::default();
        let args = launch_arguments(&settings, &server(), memory, "run-5").unwrap();

        assert_eq!(args[0], "-Xms512M");
        assert_eq!(args[1], "-Xmx1024M");
        assert_eq!(args[2], run_marker("run-5"));
    }

    #[test]
    fn a_native_server_is_launched_from_its_binary_with_no_arguments() {
        let java = server();
        let mut pumpkin = server();
        pumpkin.flavor = crate::servers::software::find("pumpkin").unwrap();
        pumpkin.launch_jar = Some("pumpkin".into());

        assert!(pumpkin.flavor.native());
        assert!(!java.flavor.native());
        assert_eq!(
            native_binary(&pumpkin).unwrap(),
            PathBuf::from("/data/servers/s1/pumpkin")
        );
        assert!(matches!(
            identity_of(&pumpkin, "run-7").unwrap(),
            Identity::Executable(_)
        ));
        assert!(matches!(
            identity_of(&java, "run-7").unwrap(),
            Identity::Marker(_)
        ));
    }

    #[test]
    fn the_ports_come_from_the_properties_file() {
        let properties =
            Properties::parse(b"server-port=25580\nenable-rcon=true\nrcon.port=25585\n");
        assert_eq!(configured_port(&properties), 25580);
        assert_eq!(
            claimed_ports(&properties),
            vec![("server-port", 25580), ("rcon.port", 25585)]
        );

        let quiet = Properties::parse(b"enable-rcon=false\nrcon.port=25585\n");
        assert_eq!(configured_port(&quiet), DEFAULT_PORT);
        assert_eq!(claimed_ports(&quiet), vec![("server-port", DEFAULT_PORT)]);
    }

    #[test]
    fn the_stop_timeout_falls_back_to_the_setting_and_stays_in_bounds() {
        let settings = crate::config::LauncherSettings::default();
        let mut server = server();
        assert_eq!(stop_timeout(&settings, &server).as_secs(), 60);
        server.stop_timeout_secs = Some(1);
        assert_eq!(stop_timeout(&settings, &server).as_secs(), 5);
        server.stop_timeout_secs = Some(9000);
        assert_eq!(stop_timeout(&settings, &server).as_secs(), 600);
    }

    #[test]
    fn a_server_without_its_own_memory_follows_the_server_setting() {
        let settings = crate::config::LauncherSettings {
            min_memory_mb: 512,
            max_memory_mb: 2048,
            server_min_memory_mb: 2048,
            server_max_memory_mb: 8192,
            ..Default::default()
        };

        let mut server = server();
        server.min_memory_mb = None;
        server.max_memory_mb = None;
        let memory = server_memory(&settings, &server).unwrap();

        assert_eq!(memory.min_mb, 2048);
        assert_eq!(memory.max_mb, 8192);
    }

    #[test]
    fn the_server_jvm_setting_is_the_base_every_server_starts_from() {
        let settings = crate::config::LauncherSettings {
            jvm_args: "-XX:+ClientOnlyFlag".into(),
            server_jvm_args: "-Xms{{min_ram}}M -Xmx{{max_ram}}M -XX:+UseG1GC".into(),
            ..Default::default()
        };

        let args = launch_arguments(
            &settings,
            &server(),
            MemoryLimits::new(1024, 4096).unwrap(),
            "run-6",
        )
        .unwrap();

        assert!(args.contains(&"-XX:+UseG1GC".to_string()));
        assert!(args.contains(&"-Xmx4096M".to_string()));
        assert!(!args.iter().any(|arg| arg == "-XX:+ClientOnlyFlag"));
    }
}
