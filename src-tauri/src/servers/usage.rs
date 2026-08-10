use std::time::Duration;

use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter, Manager};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
const IDLE_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize)]
pub struct ServerUsage {
    pub server_id: String,
    pub cpu_percent: f32,
    pub cpu_percent_normalized: f32,
    pub memory_mb: u64,
}

pub fn start_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut system = System::new();
        let cores = system.physical_core_count().unwrap_or(1).max(1) as f32;
        loop {
            let running = {
                let state = app.state::<crate::state::AppState>();
                super::runtime::running(&state)
                    .into_iter()
                    .filter(|info| info.state == "running" || info.state == "stopping")
                    .map(|info| (info.server_id, Pid::from_u32(info.pid)))
                    .collect::<Vec<_>>()
            };
            if running.is_empty() {
                tokio::time::sleep(IDLE_INTERVAL).await;
                continue;
            }

            let pids = running.iter().map(|(_, pid)| *pid).collect::<Vec<_>>();
            system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&pids),
                true,
                ProcessRefreshKind::nothing().with_cpu().with_memory(),
            );

            let samples = running
                .into_iter()
                .filter_map(|(server_id, pid)| {
                    let process = system.process(pid)?;
                    let cpu_percent = process.cpu_usage();
                    Some(ServerUsage {
                        server_id,
                        cpu_percent,
                        cpu_percent_normalized: cpu_percent / cores,
                        memory_mb: process.memory() / (1024 * 1024),
                    })
                })
                .collect::<Vec<_>>();
            if !samples.is_empty() {
                let _ = app.emit("server:usage", samples);
            }
            tokio::time::sleep(SAMPLE_INTERVAL).await;
        }
    });
}
