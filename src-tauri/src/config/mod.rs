use serde::{Deserialize, Serialize};

mod memory;

pub use memory::{MemoryLimits, DEFAULT_MAX_MEMORY_MB, DEFAULT_MIN_MEMORY_MB};

pub const DEFAULT_JVM_ARGS: &str = "-Xms{{min_ram}}M -Xmx{{max_ram}}M";

#[derive(Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Serialize, Deserialize)]
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
    #[serde(default = "default_proxy_mode")]
    pub proxy_mode: String,
    #[serde(default)]
    pub proxy_host: String,
    #[serde(default)]
    pub proxy_port: u16,
    #[serde(default)]
    pub proxy_username: String,
    #[serde(default)]
    pub proxy_password: String,
    #[serde(default = "default_timeout_secs")]
    pub request_timeout_secs: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub allow_insecure_tls: bool,
    #[serde(default)]
    pub onboarded: bool,
    #[serde(default = "default_accent_mode")]
    pub accent_mode: String,
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
    #[serde(default = "default_ok_color")]
    pub ok_color: String,
    #[serde(default = "default_warn_color")]
    pub warn_color: String,
    #[serde(default = "default_danger_color")]
    pub danger_color: String,
    #[serde(default = "default_true")]
    pub show_suggestions: bool,
    #[serde(default)]
    pub pack_content_updates: bool,
    #[serde(default)]
    pub minimize_on_launch: bool,
    #[serde(default = "default_true")]
    pub auto_update_checks: bool,
    #[serde(default)]
    pub wrapper_command: String,
    #[serde(default)]
    pub pre_launch_command: String,
    #[serde(default)]
    pub post_exit_command: String,
    #[serde(default = "default_true")]
    pub discord_rpc: bool,
    #[serde(default = "default_true")]
    pub discord_rpc_show_version: bool,
    #[serde(default = "default_true")]
    pub discord_rpc_show_streak: bool,
    #[serde(default = "default_true")]
    pub discord_rpc_show_logo: bool,
    #[serde(default)]
    pub discord_app_id: String,
    #[serde(default = "default_min_memory_mb")]
    pub server_min_memory_mb: u32,
    #[serde(default = "default_server_max_memory_mb")]
    pub server_max_memory_mb: u32,
    #[serde(default)]
    pub server_jvm_args: String,
    #[serde(default = "default_server_shutdown")]
    pub server_shutdown: String,
    #[serde(default = "default_server_stop_timeout_secs")]
    pub server_stop_timeout_secs: u32,
    #[serde(default = "default_server_console_layout")]
    pub server_console_layout: String,
}

pub fn default_proxy_mode() -> String {
    "system".to_string()
}

pub fn default_accent_mode() -> String {
    "banner".to_string()
}

pub fn default_accent_color() -> String {
    "#ff6a2b".to_string()
}

pub fn default_ok_color() -> String {
    "#46c08a".to_string()
}

pub fn default_warn_color() -> String {
    "#e9b949".to_string()
}

pub fn default_danger_color() -> String {
    "#e5484d".to_string()
}

pub fn default_true() -> bool {
    true
}

pub fn default_min_memory_mb() -> u32 {
    DEFAULT_MIN_MEMORY_MB
}

pub fn default_server_max_memory_mb() -> u32 {
    4096
}

pub fn default_server_shutdown() -> String {
    "ask".to_string()
}

pub fn default_server_stop_timeout_secs() -> u32 {
    60
}

pub fn default_server_console_layout() -> String {
    "sidebar".to_string()
}

pub fn default_timeout_secs() -> u64 {
    45
}

pub fn default_max_retries() -> u32 {
    10
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            min_memory_mb: DEFAULT_MIN_MEMORY_MB,
            max_memory_mb: DEFAULT_MAX_MEMORY_MB,
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
            proxy_mode: default_proxy_mode(),
            proxy_host: String::new(),
            proxy_port: 0,
            proxy_username: String::new(),
            proxy_password: String::new(),
            request_timeout_secs: default_timeout_secs(),
            max_retries: default_max_retries(),
            allow_insecure_tls: false,
            onboarded: false,
            accent_mode: default_accent_mode(),
            accent_color: default_accent_color(),
            ok_color: default_ok_color(),
            warn_color: default_warn_color(),
            danger_color: default_danger_color(),
            show_suggestions: true,
            pack_content_updates: false,
            minimize_on_launch: false,
            auto_update_checks: true,
            wrapper_command: String::new(),
            pre_launch_command: String::new(),
            post_exit_command: String::new(),
            discord_rpc: true,
            discord_rpc_show_version: true,
            discord_rpc_show_streak: true,
            discord_rpc_show_logo: true,
            discord_app_id: String::new(),
            server_min_memory_mb: default_min_memory_mb(),
            server_max_memory_mb: default_server_max_memory_mb(),
            server_jvm_args: String::new(),
            server_shutdown: default_server_shutdown(),
            server_stop_timeout_secs: default_server_stop_timeout_secs(),
            server_console_layout: default_server_console_layout(),
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
    #[serde(default)]
    pub jvm_args: Option<String>,
    #[serde(default)]
    pub jvm_args_mode: Option<String>,
    #[serde(default)]
    pub env_vars: Option<String>,
    #[serde(default)]
    pub env_vars_mode: Option<String>,
    #[serde(default)]
    pub import_source: Option<String>,
    #[serde(default)]
    pub import_source_id: Option<String>,
    #[serde(default)]
    pub banner_id: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub wrapper_command: Option<String>,
    #[serde(default)]
    pub pre_launch_command: Option<String>,
    #[serde(default)]
    pub post_exit_command: Option<String>,
}
