use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    files::FileManager,
    network::NetworkManager,
};

const NAME_LOOKUP: &str = "https://api.mojang.com/users/profiles/minecraft";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerList {
    Ops,
    Whitelist,
    Banned,
}

impl PlayerList {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "ops" => Ok(PlayerList::Ops),
            "whitelist" => Ok(PlayerList::Whitelist),
            "banned" => Ok(PlayerList::Banned),
            other => Err(Error::other(format!("unknown player list {other}"))),
        }
    }

    pub fn file(self) -> &'static str {
        match self {
            PlayerList::Ops => "ops.json",
            PlayerList::Whitelist => "whitelist.json",
            PlayerList::Banned => "banned-players.json",
        }
    }

    fn add_command(self, name: &str, reason: Option<&str>) -> String {
        match self {
            PlayerList::Ops => format!("op {name}"),
            PlayerList::Whitelist => format!("whitelist add {name}"),
            PlayerList::Banned => match reason {
                Some(reason) if !reason.trim().is_empty() => format!("ban {name} {reason}"),
                _ => format!("ban {name}"),
            },
        }
    }

    fn remove_command(self, name: &str) -> String {
        match self {
            PlayerList::Ops => format!("deop {name}"),
            PlayerList::Whitelist => format!("whitelist remove {name}"),
            PlayerList::Banned => format!("pardon {name}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerEntry {
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypasses_player_limit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub fn read(files: &FileManager, dir: &Path, list: PlayerList) -> Vec<PlayerEntry> {
    let Ok(bytes) = files.read(dir.join(list.file())) else {
        return Vec::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn write(
    files: &FileManager,
    dir: &Path,
    list: PlayerList,
    entries: &[PlayerEntry],
) -> Result<()> {
    let mut rendered = serde_json::to_vec_pretty(entries)?;
    rendered.push(b'\n');
    files.write_atomic(dir.join(list.file()), &rendered)
}

pub fn dashed(raw: &str) -> String {
    let clean: String = raw.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if clean.len() != 32 {
        return raw.to_string();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &clean[0..8],
        &clean[8..12],
        &clean[12..16],
        &clean[16..20],
        &clean[20..32]
    )
}

#[derive(Deserialize)]
struct NameLookup {
    id: String,
    name: String,
}

pub async fn look_up(client: &NetworkManager, name: &str) -> Result<(String, String)> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::other("Enter a player name."));
    }
    let response = client
        .send(client.get(format!("{NAME_LOOKUP}/{name}")))
        .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND
        || response.status() == reqwest::StatusCode::NO_CONTENT
    {
        return Err(Error::NotFound(format!("No player named {name}")));
    }
    let found: NameLookup = response.error_for_status()?.json().await?;
    Ok((dashed(&found.id), found.name))
}

pub fn entry_for(
    list: PlayerList,
    uuid: String,
    name: String,
    reason: Option<String>,
) -> PlayerEntry {
    PlayerEntry {
        uuid,
        name,
        level: matches!(list, PlayerList::Ops).then_some(4),
        bypasses_player_limit: matches!(list, PlayerList::Ops).then_some(false),
        created: matches!(list, PlayerList::Banned).then(|| {
            chrono::Utc::now()
                .format("%Y-%m-%d %H:%M:%S %z")
                .to_string()
        }),
        source: matches!(list, PlayerList::Banned).then(|| "Basalt".to_string()),
        expires: matches!(list, PlayerList::Banned).then(|| "forever".to_string()),
        reason: match list {
            PlayerList::Banned => Some(
                reason
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or_else(|| "Banned by an operator.".to_string()),
            ),
            _ => None,
        },
    }
}

pub fn command_to_add(list: PlayerList, name: &str, reason: Option<&str>) -> String {
    list.add_command(name, reason)
}

pub fn command_to_remove(list: PlayerList, name: &str) -> String {
    list.remove_command(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;

    #[test]
    fn an_undashed_id_is_given_its_dashes() {
        assert_eq!(
            dashed("069a79f444e94726a5befca90e38aaf5"),
            "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        );
        assert_eq!(
            dashed("069a79f4-44e9-4726-a5be-fca90e38aaf5"),
            "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        );
        assert_eq!(dashed("nonsense"), "nonsense");
    }

    #[test]
    fn each_list_speaks_its_own_console_command() {
        assert_eq!(command_to_add(PlayerList::Ops, "Notch", None), "op Notch");
        assert_eq!(
            command_to_add(PlayerList::Whitelist, "Notch", None),
            "whitelist add Notch"
        );
        assert_eq!(
            command_to_add(PlayerList::Banned, "Notch", Some("griefing")),
            "ban Notch griefing"
        );
        assert_eq!(
            command_to_add(PlayerList::Banned, "Notch", Some("  ")),
            "ban Notch"
        );
        assert_eq!(command_to_remove(PlayerList::Ops, "Notch"), "deop Notch");
        assert_eq!(
            command_to_remove(PlayerList::Banned, "Notch"),
            "pardon Notch"
        );
    }

    #[test]
    fn an_operator_is_written_the_way_the_server_writes_it() {
        let root = std::env::temp_dir().join(format!("basalt-players-{}", uuid::Uuid::new_v4()));
        let files = FileManager::new(Paths::plain(root.clone())).unwrap();
        let dir = root.join("servers").join("s1");
        files.ensure_dir(&dir).unwrap();

        let entry = entry_for(
            PlayerList::Ops,
            "069a79f4-44e9-4726-a5be-fca90e38aaf5".to_string(),
            "Notch".to_string(),
            None,
        );
        write(&files, &dir, PlayerList::Ops, &[entry]).unwrap();

        let raw = String::from_utf8(files.read(dir.join("ops.json")).unwrap()).unwrap();
        assert!(raw.contains("\"bypassesPlayerLimit\": false"));
        assert!(raw.contains("\"level\": 4"));
        assert!(!raw.contains("reason"));

        let back = read(&files, &dir, PlayerList::Ops);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].name, "Notch");
        assert_eq!(back[0].level, Some(4));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_missing_or_broken_file_reads_as_nobody() {
        let root = std::env::temp_dir().join(format!("basalt-players-{}", uuid::Uuid::new_v4()));
        let files = FileManager::new(Paths::plain(root.clone())).unwrap();
        let dir = root.join("servers").join("s1");
        files.ensure_dir(&dir).unwrap();

        assert!(read(&files, &dir, PlayerList::Whitelist).is_empty());
        files
            .write_atomic(dir.join("whitelist.json"), b"not json")
            .unwrap();
        assert!(read(&files, &dir, PlayerList::Whitelist).is_empty());
        std::fs::remove_dir_all(root).ok();
    }
}
