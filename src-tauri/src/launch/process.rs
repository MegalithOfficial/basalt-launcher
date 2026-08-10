use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};
use tokio::{process::Command, sync::oneshot};

use super::identity::{kill_recovered_process, process_matches, spawned_process_start};
use crate::{
    db::{ActiveRun, Db},
    error::{Error, Result},
    files::FileManager,
    presence::Presence,
};

const MAX_LOG_LINES: usize = 6000;
const LOG_EVENT_BATCH_LINES: usize = 256;
const RECOVERY_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
const PLAYTIME_CHECKPOINT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);
const LOG_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);
#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    pub stream: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunStatus {
    pub state: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunningInfo {
    pub running_id: String,
    pub instance_id: String,
    pub pid: u32,
    pub started_at: i64,
    pub state: String,
    pub exit_code: Option<i32>,
}

pub struct RunningHandle {
    pub instance_id: String,
    pub pid: u32,
    pub started_at: i64,
    pub status: Arc<Mutex<RunStatus>>,
    pub logs: Arc<Mutex<Vec<LogLine>>>,
    control: ProcessControl,
}

enum ProcessControl {
    Attached(Option<oneshot::Sender<()>>),
    Recovered { process_started_at: u64 },
}

impl RunningHandle {
    pub fn info(&self, running_id: &str) -> RunningInfo {
        let status = self.status.lock().unwrap().clone();
        RunningInfo {
            running_id: running_id.to_string(),
            instance_id: self.instance_id.clone(),
            pid: self.pid,
            started_at: self.started_at,
            state: status.state,
            exit_code: status.exit_code,
        }
    }

    pub fn request_kill(&mut self, running_id: &str) -> bool {
        match &mut self.control {
            ProcessControl::Attached(kill_tx) => {
                kill_tx.take().is_some_and(|tx| tx.send(()).is_ok())
            }
            ProcessControl::Recovered { process_started_at } => {
                kill_recovered_process(self.pid, *process_started_at, running_id)
            }
        }
    }
}

fn push_log_lines(
    app: AppHandle,
    running_id: &str,
    stream: &'static str,
    mut lines: Vec<String>,
    logs: &Arc<Mutex<Vec<LogLine>>>,
) {
    if lines.is_empty() {
        return;
    }
    if lines.len() > MAX_LOG_LINES {
        lines.drain(0..lines.len() - MAX_LOG_LINES);
    }

    {
        let mut buffer = logs.lock().unwrap();
        buffer.extend(lines.iter().cloned().map(|line| LogLine {
            stream: stream.to_string(),
            line,
        }));
        if buffer.len() > MAX_LOG_LINES {
            let overflow = buffer.len() - MAX_LOG_LINES;
            buffer.drain(0..overflow);
        }
    }
    for batch in lines.chunks(LOG_EVENT_BATCH_LINES) {
        let _ = app.emit(
            "process:log",
            json!({ "running_id": running_id, "stream": stream, "lines": batch }),
        );
    }
}

fn spawn_log_tailer(
    app: AppHandle,
    files: FileManager,
    running_id: String,
    stream: &'static str,
    status: Arc<Mutex<RunStatus>>,
    logs: Arc<Mutex<Vec<LogLine>>>,
) {
    tauri::async_runtime::spawn(async move {
        let path = files.paths().run_log(&running_id, stream);
        let mut offset = 0u64;
        let mut pending = Vec::new();
        loop {
            if let Ok(tail) = files.read_tail_async(&path, offset).await {
                if tail.restarted {
                    pending.clear();
                }
                pending.extend_from_slice(&tail.bytes);
                offset = tail.len;

                let mut lines = Vec::new();
                while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
                    let mut line = pending.drain(..=newline).collect::<Vec<_>>();
                    line.pop();
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    lines.push(String::from_utf8_lossy(&line).into_owned());
                }
                push_log_lines(app.clone(), &running_id, stream, lines, &logs);
            }

            let running = status.lock().unwrap().state == "running";
            if !running {
                if !pending.is_empty() {
                    push_log_lines(
                        app.clone(),
                        &running_id,
                        stream,
                        vec![String::from_utf8_lossy(&pending).into_owned()],
                        &logs,
                    );
                }
                break;
            }
            tokio::time::sleep(LOG_POLL_INTERVAL).await;
        }
    });
}

fn spawn_playtime_checkpointer(db: Db, running_id: String, status: Arc<Mutex<RunStatus>>) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(PLAYTIME_CHECKPOINT_INTERVAL).await;
            if status.lock().unwrap().state != "running" {
                break;
            }
            let checkpointed_at = chrono::Utc::now().timestamp();
            match db.checkpoint_active_run(&running_id, checkpointed_at) {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => {
                    tracing::warn!(
                        running_id,
                        error = %error,
                        "could not checkpoint active game playtime"
                    );
                }
            }
        }
    });
}

pub struct ProcessLaunch<'a> {
    pub instance_id: &'a str,
    pub running_id: &'a str,
    pub started_at: i64,
    pub program: &'a str,
    pub args: Vec<String>,
    pub cwd: &'a std::path::Path,
    pub env: Vec<(String, String)>,
    pub post_exit: Option<String>,
}

pub fn spawn_process(
    app: &AppHandle,
    registry: &Arc<Mutex<HashMap<String, RunningHandle>>>,
    files: FileManager,
    db: Db,
    presence: Arc<Presence>,
    launch: ProcessLaunch<'_>,
) -> Result<()> {
    let ProcessLaunch {
        instance_id,
        running_id,
        started_at,
        program,
        args,
        cwd,
        env,
        post_exit,
    } = launch;
    let stdout_log = files.create(files.paths().run_log(running_id, "stdout"))?;
    let stderr_log = files.create(files.paths().run_log(running_id, "stderr"))?;
    let mut command = Command::new(program);
    command
        .args(&args)
        .envs(env.iter().cloned())
        .current_dir(cwd)
        .stdout(Stdio::from(stdout_log))
        .stderr(Stdio::from(stderr_log));

    let mut child = command.spawn().inspect_err(|e| {
        tracing::error!(program, error = %e, "could not spawn game process");
    })?;
    let pid = child.id().unwrap_or(0);
    let process_started_at = spawned_process_start(pid, running_id).ok_or_else(|| {
        let _ = child.start_kill();
        Error::other("could not verify the launched game process")
    })?;
    if let Err(error) = db.save_active_run(&ActiveRun {
        running_id: running_id.to_string(),
        instance_id: instance_id.to_string(),
        pid,
        process_started_at,
        started_at,
        checkpointed_at: started_at,
    }) {
        let _ = child.start_kill();
        return Err(error);
    }
    tracing::info!(
        instance_id,
        running_id,
        pid,
        program,
        "game process started"
    );
    let status = Arc::new(Mutex::new(RunStatus {
        state: "running".to_string(),
        exit_code: None,
    }));
    let logs = Arc::new(Mutex::new(Vec::new()));
    let (kill_tx, kill_rx) = oneshot::channel::<()>();

    spawn_log_tailer(
        app.clone(),
        files.clone(),
        running_id.to_string(),
        "stdout",
        status.clone(),
        logs.clone(),
    );
    spawn_playtime_checkpointer(db.clone(), running_id.to_string(), status.clone());
    spawn_log_tailer(
        app.clone(),
        files,
        running_id.to_string(),
        "stderr",
        status.clone(),
        logs.clone(),
    );

    let sup_app = app.clone();
    let sup_logs = logs.clone();
    let sup_cwd = cwd.to_path_buf();
    let sup_env = env.clone();
    let sup_status = status.clone();
    let sup_running_id = running_id.to_string();
    let sup_instance_id = instance_id.to_string();
    tauri::async_runtime::spawn(async move {
        let exit = tokio::select! {
            result = child.wait() => result,
            _ = kill_rx => {
                let _ = child.start_kill();
                child.wait().await
            }
        };
        let code = exit.ok().and_then(|s| s.code());
        let state = if matches!(code, Some(0) | None) {
            "exited"
        } else {
            "crashed"
        };
        {
            let mut guard = sup_status.lock().unwrap();
            guard.state = state.to_string();
            guard.exit_code = code;
        }
        let ended_at = chrono::Utc::now().timestamp();
        let played_secs = ended_at - started_at;
        if state == "crashed" {
            let tail = {
                let buffer = sup_logs.lock().unwrap();
                buffer
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
                instance_id = %sup_instance_id,
                pid,
                exit_code = ?code,
                played_secs,
                "game exited abnormally:\n{tail}"
            );
        } else {
            tracing::info!(
                instance_id = %sup_instance_id,
                pid,
                exit_code = ?code,
                played_secs,
                "game exited"
            );
        }
        presence.clear();
        match db.finalize_active_run(&sup_running_id, ended_at, state == "crashed") {
            Ok(true) => {}
            Ok(false) => tracing::warn!(
                running_id = %sup_running_id,
                "active run disappeared before playtime could be finalized"
            ),
            Err(error) => tracing::warn!(
                running_id = %sup_running_id,
                error = %error,
                "could not finalize game playtime"
            ),
        }
        if let Some(command) = post_exit {
            if let Err(error) =
                super::tools::run_hook("post-exit", &command, &sup_cwd, &sup_env).await
            {
                tracing::warn!(error = %error, "the post-exit command did not finish cleanly");
            }
        }
        let _ = sup_app.emit(
            "process:state",
            RunningInfo {
                running_id: sup_running_id,
                instance_id: sup_instance_id,
                pid,
                started_at,
                state: state.to_string(),
                exit_code: code,
            },
        );
    });

    registry.lock().unwrap().insert(
        running_id.to_string(),
        RunningHandle {
            instance_id: instance_id.to_string(),
            pid,
            started_at,
            status,
            logs,
            control: ProcessControl::Attached(Some(kill_tx)),
        },
    );

    let _ = app.emit(
        "process:state",
        RunningInfo {
            running_id: running_id.to_string(),
            instance_id: instance_id.to_string(),
            pid,
            started_at,
            state: "running".to_string(),
            exit_code: None,
        },
    );

    Ok(())
}

fn monitor_recovered_process(
    app: AppHandle,
    db: Db,
    presence: Arc<Presence>,
    run: ActiveRun,
    status: Arc<Mutex<RunStatus>>,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(RECOVERY_POLL_INTERVAL).await;
            if process_matches(run.pid, run.process_started_at, &run.running_id) {
                continue;
            }

            {
                let mut status = status.lock().unwrap();
                status.state = "exited".to_string();
                status.exit_code = None;
            }
            let ended_at = chrono::Utc::now().timestamp();
            let played_secs = ended_at.saturating_sub(run.started_at);
            presence.clear();
            if let Err(error) = db.finalize_active_run(&run.running_id, ended_at, false) {
                tracing::warn!(error = %error, "could not finalize recovered game playtime");
            }
            let _ = app.emit(
                "process:state",
                RunningInfo {
                    running_id: run.running_id.clone(),
                    instance_id: run.instance_id.clone(),
                    pid: run.pid,
                    started_at: run.started_at,
                    state: "exited".to_string(),
                    exit_code: None,
                },
            );
            tracing::info!(
                instance_id = %run.instance_id,
                running_id = %run.running_id,
                pid = run.pid,
                played_secs,
                "recovered game process exited"
            );
            break;
        }
    });
}

pub fn recover_processes(
    app: &AppHandle,
    registry: &Arc<Mutex<HashMap<String, RunningHandle>>>,
    files: &FileManager,
    db: &Db,
    presence: &Arc<Presence>,
) -> Result<usize> {
    let mut recovered = 0;
    let settings = db.load_settings().ok();
    let instances = db.list_instances(files).unwrap_or_default();
    for run in db.active_runs()? {
        if !process_matches(run.pid, run.process_started_at, &run.running_id) {
            let ended_at = run.checkpointed_at.max(run.started_at);
            let played_secs = ended_at.saturating_sub(run.started_at);
            tracing::info!(
                running_id = %run.running_id,
                pid = run.pid,
                played_secs,
                "finalizing interrupted game playtime from its last checkpoint"
            );
            db.finalize_active_run(&run.running_id, ended_at, false)?;
            continue;
        }
        db.checkpoint_active_run(&run.running_id, chrono::Utc::now().timestamp())?;

        let status = Arc::new(Mutex::new(RunStatus {
            state: "running".to_string(),
            exit_code: None,
        }));
        let logs = Arc::new(Mutex::new(Vec::new()));
        registry.lock().unwrap().insert(
            run.running_id.clone(),
            RunningHandle {
                instance_id: run.instance_id.clone(),
                pid: run.pid,
                started_at: run.started_at,
                status: status.clone(),
                logs: logs.clone(),
                control: ProcessControl::Recovered {
                    process_started_at: run.process_started_at,
                },
            },
        );
        spawn_log_tailer(
            app.clone(),
            files.clone(),
            run.running_id.clone(),
            "stdout",
            status.clone(),
            logs.clone(),
        );
        spawn_log_tailer(
            app.clone(),
            files.clone(),
            run.running_id.clone(),
            "stderr",
            status.clone(),
            logs,
        );
        spawn_playtime_checkpointer(db.clone(), run.running_id.clone(), status.clone());
        if let Some(settings) = settings.as_ref() {
            let instance = instances
                .iter()
                .find(|candidate| candidate.id == run.instance_id);
            if let Some(instance) = instance {
                if let Some(activity) = crate::presence::activity_for(
                    settings,
                    &instance.name,
                    &instance.version_id,
                    instance.loader.as_deref(),
                    instance.logo.as_deref(),
                    super::presence_detail(db, instance),
                    run.started_at,
                ) {
                    presence.set(activity);
                }
            }
        }
        monitor_recovered_process(app.clone(), db.clone(), presence.clone(), run, status);
        recovered += 1;
    }
    if recovered > 0 {
        tracing::info!(recovered, "recovered running game processes");
    }
    Ok(recovered)
}

#[cfg(test)]
mod tail_tests {
    use std::io::Write;

    use crate::{files::FileManager, paths::Paths};

    #[test]
    fn a_tail_read_returns_only_what_was_appended() {
        let root = std::env::temp_dir().join(format!("basalt-tail-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let files = FileManager::new(Paths::plain(root.clone())).unwrap();
        let path = root.join("run.log");
        let runtime = tokio::runtime::Runtime::new().unwrap();

        std::fs::write(&path, b"first\nsecond\n").unwrap();
        let opening = runtime.block_on(files.read_tail_async(&path, 0)).unwrap();
        assert_eq!(opening.bytes, b"first\nsecond\n");
        assert!(!opening.restarted);

        let mut handle = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        handle.write_all(b"third\n").unwrap();
        drop(handle);

        let appended = runtime
            .block_on(files.read_tail_async(&path, opening.len))
            .unwrap();
        assert_eq!(appended.bytes, b"third\n");
        assert!(!appended.restarted);

        let idle = runtime
            .block_on(files.read_tail_async(&path, appended.len))
            .unwrap();
        assert!(idle.bytes.is_empty());

        std::fs::write(&path, b"restarted\n").unwrap();
        let rewound = runtime
            .block_on(files.read_tail_async(&path, appended.len))
            .unwrap();
        assert!(rewound.restarted);
        assert_eq!(rewound.bytes, b"restarted\n");

        let _ = std::fs::remove_dir_all(&root);
    }
}
