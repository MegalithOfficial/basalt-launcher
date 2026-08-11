use serde::{Deserialize, Serialize};

use crate::{error::Result, state::AppState};

pub mod config;
pub mod files;
pub mod import;
pub mod properties;
pub mod provision;
pub mod runtime;
pub mod software;
pub mod usage;

pub use software::Software;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextProblem {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub flavor: Software,
    pub version_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub managed: bool,
    #[serde(default)]
    pub dir: String,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub flavor_version: Option<String>,
    #[serde(default)]
    pub launch_jar: Option<String>,
    #[serde(default)]
    pub launch_argfiles: Vec<String>,
    #[serde(default)]
    pub min_memory_mb: Option<u32>,
    #[serde(default)]
    pub max_memory_mb: Option<u32>,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default)]
    pub jvm_args: Option<String>,
    #[serde(default)]
    pub jvm_args_mode: Option<String>,
    #[serde(default)]
    pub stop_timeout_secs: Option<u32>,
    #[serde(default)]
    pub eula_accepted_at: Option<i64>,
    #[serde(default)]
    pub installed_at: Option<i64>,
    #[serde(default)]
    pub last_started_at: Option<i64>,
    #[serde(default)]
    pub uptime_secs: i64,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub motd: Option<String>,
    #[serde(default)]
    pub max_players: Option<u32>,
    #[serde(default)]
    pub notes: Option<String>,
}

pub fn adopt_imported_dirs(state: &AppState) -> Result<()> {
    let dirs = state.db.imported_server_dirs()?;
    state.paths.adopt_extras(dirs);
    state.files.reopen()
}
