use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

const MAX_LOG_LINES: usize = 6000;

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
    pub kill_tx: Option<oneshot::Sender<()>>,
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
}

fn spawn_reader<R>(app: AppHandle, running_id: String, stream: &'static str, reader: R, logs: Arc<Mutex<Vec<LogLine>>>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
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
    });
}

pub fn spawn_process(
    app: &AppHandle,
    registry: &Mutex<HashMap<String, RunningHandle>>,
    instance_id: &str,
    running_id: &str,
    started_at: i64,
    program: &str,
    args: Vec<String>,
    cwd: &std::path::Path,
) -> crate::error::Result<()> {
    let mut command = Command::new(program);
    command
        .args(&args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn()?;
    let pid = child.id().unwrap_or(0);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let status = Arc::new(Mutex::new(RunStatus {
        state: "running".to_string(),
        exit_code: None,
    }));
    let logs = Arc::new(Mutex::new(Vec::new()));
    let (kill_tx, kill_rx) = oneshot::channel::<()>();

    if let Some(stdout) = stdout {
        spawn_reader(app.clone(), running_id.to_string(), "stdout", stdout, logs.clone());
    }
    if let Some(stderr) = stderr {
        spawn_reader(app.clone(), running_id.to_string(), "stderr", stderr, logs.clone());
    }

    let sup_app = app.clone();
    let sup_status = status.clone();
    let sup_running_id = running_id.to_string();
    let sup_instance_id = instance_id.to_string();
    tokio::spawn(async move {
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
            kill_tx: Some(kill_tx),
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
