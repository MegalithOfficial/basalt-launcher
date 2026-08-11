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

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub mount_point: String,
    pub name: String,
    pub free_mb: u64,
    pub total_mb: u64,
    pub removable: bool,
}

/// The address other machines on the network reach this one on. No packets are
/// sent, the socket is only asked which route it would take.
pub fn lan_address() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    let address = socket.local_addr().ok()?.ip();
    (!address.is_loopback() && !address.is_unspecified()).then(|| address.to_string())
}

pub fn disk_for(path: &std::path::Path) -> Option<DiskInfo> {
    let disks = Disks::new_with_refreshed_list();
    let disk = disks
        .list()
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())?;
    Some(DiskInfo {
        mount_point: disk.mount_point().display().to_string(),
        name: disk.name().to_string_lossy().to_string(),
        free_mb: disk.available_space() / MB,
        total_mb: disk.total_space() / MB,
        removable: disk.is_removable(),
    })
}

fn free_space_for(paths: &Paths) -> (Option<u64>, Option<u64>) {
    match disk_for(&paths.root) {
        Some(disk) => (Some(disk.free_mb), Some(disk.total_mb)),
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
        cores: system
            .physical_core_count()
            .unwrap_or_else(|| system.cpus().len()),
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
