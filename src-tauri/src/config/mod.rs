use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherSettings {
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    pub java_path: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub version_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub min_memory_mb: Option<u32>,
    pub max_memory_mb: Option<u32>,
    pub java_path: Option<String>,
    #[serde(default)]
    pub last_played_at: Option<i64>,
    #[serde(default)]
    pub playtime_secs: i64,
    #[serde(default)]
    pub dir: String,
    #[serde(default)]
    pub loader: Option<String>,
    #[serde(default)]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub launch_version_id: Option<String>,
}
