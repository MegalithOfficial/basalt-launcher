use std::path::{Path, PathBuf};

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    files::FileManager,
    network::NetworkManager,
    state::AppState,
    tasks::TaskHandle,
};

use super::{provision::Provisioned, Server};

mod fabric;
mod forge;
mod neoforge;
mod paper;
mod pumpkin;
mod purpur;
mod vanilla;

pub static ALL: &[&dyn ServerSoftware] = &[
    &vanilla::Vanilla,
    &paper::Paper,
    &purpur::Purpur,
    &fabric::Fabric,
    &neoforge::Neoforge,
    &forge::Forge,
    &pumpkin::Pumpkin,
];

pub fn find(id: &str) -> Result<Software> {
    ALL.iter()
        .find(|entry| entry.spec().id == id)
        .map(|entry| Software(*entry))
        .ok_or_else(|| Error::other(format!("unknown server software {id}")))
}

pub fn vanilla() -> Software {
    Software(&vanilla::Vanilla)
}

pub fn specs() -> Vec<Spec> {
    ALL.iter().map(|entry| entry.spec()).collect()
}

pub fn detect(folder: &Folder) -> Option<(Software, Detected)> {
    let mut fallback = None;
    for entry in ALL {
        let Some(found) = entry.detect(folder) else {
            continue;
        };
        if found.certain {
            return Some((Software(*entry), found));
        }
        fallback.get_or_insert((Software(*entry), found));
    }
    fallback
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Runtime {
    Java,
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Spec {
    pub id: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub runtime: Runtime,
    pub builds: bool,
    pub config_file: &'static str,
    pub content_dir: Option<&'static str>,
}

pub struct Install<'a> {
    pub state: &'a AppState,
    pub server: &'a Server,
    pub task: &'a TaskHandle,
}

impl<'a> Install<'a> {
    pub fn dir(&self) -> PathBuf {
        PathBuf::from(&self.server.dir)
    }

    pub fn game_version(&self) -> &'a str {
        &self.server.version_id
    }

    pub fn build(&self) -> Option<&'a str> {
        self.server.flavor_version.as_deref()
    }

    pub fn network(&self) -> &'a NetworkManager {
        &self.state.network
    }

    pub fn files(&self) -> &'a FileManager {
        &self.state.files
    }
}

pub struct Folder<'a> {
    pub dir: &'a Path,
    pub entries: &'a [String],
}

impl Folder<'_> {
    pub fn has(&self, name: &str) -> bool {
        self.entries.iter().any(|entry| entry == name)
    }

    pub fn named(&self, matches: impl Fn(&str) -> bool) -> Option<String> {
        self.entries.iter().find(|entry| matches(entry)).cloned()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detected {
    pub certain: bool,
    pub version_id: Option<String>,
    pub flavor_version: Option<String>,
    pub launch_jar: Option<String>,
    pub launch_argfiles: Vec<String>,
}

pub trait ServerSoftware: Send + Sync {
    fn spec(&self) -> Spec;

    fn versions<'a>(
        &'a self,
        _client: &'a NetworkManager,
        _game_version: &'a str,
    ) -> BoxFuture<'a, Result<Vec<String>>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn install<'a>(&'a self, install: Install<'a>) -> BoxFuture<'a, Result<Provisioned>>;

    fn detect(&self, _folder: &Folder) -> Option<Detected> {
        None
    }

    fn launch_args(&self) -> &'static [&'static str] {
        &["nogui"]
    }

    fn port(&self, files: &FileManager, dir: &Path) -> Option<u16> {
        let bytes = files.read(dir.join("server.properties")).ok()?;
        super::properties::Properties::parse(&bytes)
            .get("server-port")
            .and_then(|value| value.trim().parse().ok())
    }
}

#[derive(Clone, Copy)]
pub struct Software(&'static dyn ServerSoftware);

impl Software {
    pub fn id(self) -> &'static str {
        self.0.spec().id
    }

    pub fn label(self) -> &'static str {
        self.0.spec().label
    }

    pub fn runtime(self) -> Runtime {
        self.0.spec().runtime
    }

    pub fn native(self) -> bool {
        self.runtime() == Runtime::Native
    }

    pub fn config_file(self) -> &'static str {
        self.0.spec().config_file
    }

    pub fn content_dir(self) -> Option<&'static str> {
        self.0.spec().content_dir
    }

    pub fn launch_args(self) -> &'static [&'static str] {
        self.0.launch_args()
    }

    pub async fn versions(
        self,
        client: &NetworkManager,
        game_version: &str,
    ) -> Result<Vec<String>> {
        self.0.versions(client, game_version).await
    }

    pub async fn install(self, install: Install<'_>) -> Result<Provisioned> {
        self.0.install(install).await
    }

    pub fn port(self, files: &FileManager, dir: &Path) -> Option<u16> {
        self.0.port(files, dir)
    }
}

impl std::fmt::Debug for Software {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

impl std::fmt::Display for Software {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

impl PartialEq for Software {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for Software {}

impl Serialize for Software {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.id())
    }
}

impl<'de> Deserialize<'de> for Software {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let id = String::deserialize(deserializer)?;
        find(&id).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_software_is_reachable_by_its_id() {
        for spec in specs() {
            assert_eq!(find(spec.id).unwrap().id(), spec.id);
        }
        assert!(find("spigot").is_err());
    }

    #[test]
    fn the_registry_holds_no_duplicate_ids() {
        let mut ids = specs().into_iter().map(|spec| spec.id).collect::<Vec<_>>();
        ids.sort_unstable();
        let total = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), total);
    }

    #[test]
    fn a_certain_match_wins_over_a_guess() {
        let dir = std::env::temp_dir();
        let entries = vec![
            "server.jar".to_string(),
            "fabric-server-launch.jar".to_string(),
        ];
        let folder = Folder {
            dir: &dir,
            entries: &entries,
        };
        let (software, _) = detect(&folder).unwrap();
        assert_eq!(software.id(), "fabric");
    }

    #[test]
    fn a_native_software_needs_no_eula_and_no_java() {
        let pumpkin = find("pumpkin").unwrap();
        assert!(pumpkin.native());
        assert!(pumpkin.launch_args().is_empty());
        assert_eq!(pumpkin.config_file(), "pumpkin.toml");

        let paper = find("paper").unwrap();
        assert!(!paper.native());
        assert_eq!(paper.launch_args(), ["nogui"]);
        assert_eq!(paper.config_file(), "server.properties");
    }
}
