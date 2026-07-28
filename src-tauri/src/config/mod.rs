use serde::{Deserialize, Serialize};

pub const DEFAULT_JVM_ARGS: &str = "-Xms{{min_ram}}M -Xmx{{max_ram}}M";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherSettings {
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    pub java_path: Option<String>,
    pub concurrent_downloads: usize,
    pub curseforge_api_key: Option<String>,
    pub log_level: String,
    pub jvm_args: String,
    pub game_args: String,
    pub window_width: u32,
    pub window_height: u32,
    pub fullscreen: bool,
    pub ignore_java_checks: bool,
    pub env_vars: Vec<EnvVar>,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            min_memory_mb: 512,
            max_memory_mb: 2048,
            java_path: None,
            concurrent_downloads: 16,
            curseforge_api_key: None,
            log_level: crate::logging::DEFAULT_LEVEL.to_string(),
            jvm_args: DEFAULT_JVM_ARGS.to_string(),
            game_args: String::new(),
            window_width: 854,
            window_height: 480,
            fullscreen: false,
            ignore_java_checks: false,
            env_vars: Vec::new(),
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
    pub logo: Option<String>,
    #[serde(default)]
    pub loader: Option<String>,
    #[serde(default)]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub launch_version_id: Option<String>,
    #[serde(default)]
    pub pack_provider: Option<String>,
    #[serde(default)]
    pub pack_project_id: Option<String>,
    #[serde(default)]
    pub pack_version_id: Option<String>,
}
