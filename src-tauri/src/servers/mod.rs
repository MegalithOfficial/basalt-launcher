use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub mod files;
pub mod properties;
pub mod provision;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextProblem {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerFlavor {
    Vanilla,
    Paper,
    Purpur,
    Fabric,
    Neoforge,
    Forge,
}

impl ServerFlavor {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "vanilla" => Ok(ServerFlavor::Vanilla),
            "paper" => Ok(ServerFlavor::Paper),
            "purpur" => Ok(ServerFlavor::Purpur),
            "fabric" => Ok(ServerFlavor::Fabric),
            "neoforge" => Ok(ServerFlavor::Neoforge),
            "forge" => Ok(ServerFlavor::Forge),
            other => Err(Error::other(format!("unknown server flavor {other}"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ServerFlavor::Vanilla => "vanilla",
            ServerFlavor::Paper => "paper",
            ServerFlavor::Purpur => "purpur",
            ServerFlavor::Fabric => "fabric",
            ServerFlavor::Neoforge => "neoforge",
            ServerFlavor::Forge => "forge",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub flavor: ServerFlavor,
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

#[cfg(test)]
mod tests {
    use super::ServerFlavor;

    #[test]
    fn every_flavor_round_trips_through_its_string() {
        for flavor in [
            ServerFlavor::Vanilla,
            ServerFlavor::Paper,
            ServerFlavor::Purpur,
            ServerFlavor::Fabric,
            ServerFlavor::Neoforge,
            ServerFlavor::Forge,
        ] {
            assert_eq!(ServerFlavor::parse(flavor.as_str()).unwrap(), flavor);
        }
        assert!(ServerFlavor::parse("spigot").is_err());
    }
}
