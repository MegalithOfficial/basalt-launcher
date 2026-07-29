use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use serde_json::json;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tauri::{AppHandle, Emitter};
use tokio::{process::Command, sync::oneshot};

use crate::{
    db::{ActiveRun, Db},
    error::{Error, Result},
    files::FileManager,
};

const MAX_LOG_LINES: usize = 6000;
const RECOVERY_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
const LOG_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);
const RUN_MARKER_PREFIX: &str = "-Dbasalt.running_id=";

pub fn run_marker(running_id: &str) -> String {
    format!("{RUN_MARKER_PREFIX}{running_id}")
}

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

fn refresh_process(system: &mut System, pid: Pid) {
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        false,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );
}

fn command_has_marker(command: &[std::ffi::OsString], running_id: &str) -> bool {
    let expected = run_marker(running_id);
    command.iter().any(|argument| argument == expected.as_str())
}

fn identity_matches(
    actual_started_at: u64,
    command: &[std::ffi::OsString],
    expected_started_at: u64,
    running_id: &str,
) -> bool {
    actual_started_at == expected_started_at && command_has_marker(command, running_id)
}

fn process_matches(pid: u32, process_started_at: u64, running_id: &str) -> bool {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    refresh_process(&mut system, pid);
    system.process(pid).is_some_and(|process| {
        identity_matches(
            process.start_time(),
            process.cmd(),
            process_started_at,
            running_id,
        )
    })
}

fn spawned_process_start(pid: u32, running_id: &str) -> Option<u64> {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    refresh_process(&mut system, pid);
    let process = system.process(pid)?;
    command_has_marker(process.cmd(), running_id).then(|| process.start_time())
}

fn kill_recovered_process(pid: u32, process_started_at: u64, running_id: &str) -> bool {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    refresh_process(&mut system, pid);
    system.process(pid).is_some_and(|process| {
        process.start_time() == process_started_at
            && command_has_marker(process.cmd(), running_id)
            && process.kill()
    })
}

fn push_log_line(
    app: AppHandle,
    running_id: &str,
    stream: &'static str,
    line: String,
    logs: &Arc<Mutex<Vec<LogLine>>>,
) {
    {
        let mut buffer = logs.lock().unwrap();
        buffer.push(LogLine {
            stream: stream.to_string(),
            line: line.clone(),
        });
        if buffer.len() > MAX_LOG_LINES {
            let overflow = buffer.len() - MAX_LOG_LINES;
            buffer.drain(0..overflow);
        }
    }
    let _ = app.emit(
        "process:log",
        json!({ "running_id": running_id, "stream": stream, "line": line }),
    );
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
        let mut offset = 0;
        let mut pending = Vec::new();
        loop {
            if let Ok(bytes) = files.read_async(&path).await {
                if bytes.len() < offset {
                    offset = 0;
                    pending.clear();
                }
                pending.extend_from_slice(&bytes[offset..]);
                offset = bytes.len();

                while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
                    let mut line = pending.drain(..=newline).collect::<Vec<_>>();
                    line.pop();
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    push_log_line(
                        app.clone(),
                        &running_id,
                        stream,
                        String::from_utf8_lossy(&line).into_owned(),
                        &logs,
                    );
                }
            }

            let running = status.lock().unwrap().state == "running";
            if !running {
                if !pending.is_empty() {
                    push_log_line(
                        app.clone(),
                        &running_id,
                        stream,
                        String::from_utf8_lossy(&pending).into_owned(),
                        &logs,
                    );
                }
                break;
            }
            tokio::time::sleep(LOG_POLL_INTERVAL).await;
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
}

pub fn spawn_process(
    app: &AppHandle,
    registry: &Arc<Mutex<HashMap<String, RunningHandle>>>,
    files: FileManager,
    db: Db,
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
    } = launch;
    let stdout_log = files.create(files.paths().run_log(running_id, "stdout"))?;
    let stderr_log = files.create(files.paths().run_log(running_id, "stderr"))?;
    let mut command = Command::new(program);
    command
        .args(&args)
        .envs(env)
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
        if let Err(error) = db.remove_active_run(&sup_running_id) {
            tracing::warn!(error = %error, "could not remove the active run record");
        }
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
        let _ = db.record_playtime(&sup_instance_id, played_secs, ended_at);
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
            if let Err(error) = db.record_playtime(&run.instance_id, played_secs, ended_at) {
                tracing::warn!(error = %error, "could not record recovered game playtime");
            }
            if let Err(error) = db.remove_active_run(&run.running_id) {
                tracing::warn!(error = %error, "could not remove recovered active run");
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
) -> Result<usize> {
    let mut recovered = 0;
    for run in db.active_runs()? {
        if !process_matches(run.pid, run.process_started_at, &run.running_id) {
            tracing::info!(
                running_id = %run.running_id,
                pid = run.pid,
                "removing stale active run"
            );
            db.remove_active_run(&run.running_id)?;
            continue;
        }

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
        monitor_recovered_process(app.clone(), db.clone(), run, status);
        recovered += 1;
    }
    if recovered > 0 {
        tracing::info!(recovered, "recovered running game processes");
    }
    Ok(recovered)
}

#[cfg(test)]
mod recovery_tests {
    use std::ffi::OsString;

    use super::{command_has_marker, identity_matches, run_marker};

    #[test]
    fn marker_matches_only_the_expected_run() {
        let args = vec![
            OsString::from("-Xmx4G"),
            OsString::from(run_marker("run-1")),
            OsString::from("net.minecraft.client.main.Main"),
        ];
        assert!(command_has_marker(&args, "run-1"));
        assert!(!command_has_marker(&args, "run-2"));
        assert!(identity_matches(100, &args, 100, "run-1"));
        assert!(!identity_matches(101, &args, 100, "run-1"));
    }
}
