use std::path::Path;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::error::Result;
use crate::paths::Paths;

/// Global launcher settings, persisted to `launcher.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherSettings {
    /// Default JVM minimum heap in MB (per-instance can override).
    pub min_memory_mb: u32,
    /// Default JVM maximum heap in MB (per-instance can override).
    pub max_memory_mb: u32,
    /// Optional global Java executable override; `None` = auto-detect/managed.
    pub java_path: Option<String>,
    /// Max concurrent downloads during installs.
    pub concurrent_downloads: usize,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            min_memory_mb: 512,
            max_memory_mb: 2048,
            java_path: None,
            concurrent_downloads: 16,
        }
    }
}

/// A launchable profile: a Minecraft version plus its own game directory and overrides.
/// This is the unit the UI plays and the future modpack unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    /// Minecraft version id (e.g. "1.21.4").
    pub version_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Per-instance memory overrides (fall back to global settings when `None`).
    pub min_memory_mb: Option<u32>,
    pub max_memory_mb: Option<u32>,
    /// Per-instance Java override.
    pub java_path: Option<String>,
}

/// Read a JSON file into `T`, returning `T::default()` when the file does not exist.
fn load_json<T: DeserializeOwned + Default>(path: &Path) -> Result<T> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(e.into()),
    }
}

/// Atomically write `value` as pretty JSON to `path` (write-temp-then-rename).
fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load_settings(paths: &Paths) -> Result<LauncherSettings> {
    load_json(&paths.settings_file())
}

pub fn save_settings(paths: &Paths, settings: &LauncherSettings) -> Result<()> {
    save_json(&paths.settings_file(), settings)
}

pub fn load_instances(paths: &Paths) -> Result<Vec<Instance>> {
    load_json(&paths.instances_file())
}

pub fn save_instances(paths: &Paths, instances: &[Instance]) -> Result<()> {
    save_json(&paths.instances_file(), &instances.to_vec())
}
