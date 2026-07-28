use serde::Serialize;
use sysinfo::{Disks, System};

use crate::paths::Paths;

const MB: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct SystemStats {
    pub os: String,
    pub kernel: Option<String>,
    pub cpu: String,
    pub cores: usize,
    pub total_memory_mb: u64,
    pub available_memory_mb: u64,
    pub data_dir_free_mb: Option<u64>,
    pub data_dir_total_mb: Option<u64>,
}

fn free_space_for(paths: &Paths) -> (Option<u64>, Option<u64>) {
    let root = &paths.root;
    let disks = Disks::new_with_refreshed_list();
    let best = disks
        .list()
        .iter()
        .filter(|disk| root.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len());

    match best {
        Some(disk) => (
            Some(disk.available_space() / MB),
            Some(disk.total_space() / MB),
        ),
        None => (None, None),
    }
}

#[tracing::instrument(skip_all)]
pub fn collect(paths: &Paths) -> SystemStats {
    let mut system = System::new();
    system.refresh_memory();
    system.refresh_cpu_all();

    let cpu = system
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|brand| !brand.is_empty())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let os = match (System::name(), System::os_version()) {
        (Some(name), Some(version)) => format!("{name} {version}"),
        (Some(name), None) => name,
        _ => std::env::consts::OS.to_string(),
    };

    let (data_dir_free_mb, data_dir_total_mb) = free_space_for(paths);

    SystemStats {
        os,
        kernel: System::kernel_version(),
        cpu,
        cores: System::physical_core_count().unwrap_or_else(|| system.cpus().len()),
        total_memory_mb: system.total_memory() / MB,
        available_memory_mb: system.available_memory() / MB,
        data_dir_free_mb,
        data_dir_total_mb,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemUsage {
    pub total_memory_mb: u64,
    pub available_memory_mb: u64,
    pub data_dir_free_mb: Option<u64>,
    pub data_dir_total_mb: Option<u64>,
}

#[tracing::instrument(skip_all)]
pub fn usage(paths: &Paths) -> SystemUsage {
    let mut system = System::new();
    system.refresh_memory();
    let (data_dir_free_mb, data_dir_total_mb) = free_space_for(paths);

    SystemUsage {
        total_memory_mb: system.total_memory() / MB,
        available_memory_mb: system.available_memory() / MB,
        data_dir_free_mb,
        data_dir_total_mb,
    }
}
