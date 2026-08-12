use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Modrinth,
    Curseforge,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Modrinth => "modrinth",
            Provider::Curseforge => "curseforge",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "modrinth" => Ok(Provider::Modrinth),
            "curseforge" => Ok(Provider::Curseforge),
            other => Err(Error::other(format!("unknown provider {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentKind {
    Mod,
    Modpack,
    ResourcePack,
    Shader,
    DataPack,
}

impl ContentKind {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "mods" => Ok(ContentKind::Mod),
            "modpacks" => Ok(ContentKind::Modpack),
            "resourcepacks" => Ok(ContentKind::ResourcePack),
            "shaderpacks" => Ok(ContentKind::Shader),
            "datapacks" => Ok(ContentKind::DataPack),
            other => Err(Error::other(format!("cannot browse {other}"))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ContentKind::Mod => "mods",
            ContentKind::Modpack => "modpacks",
            ContentKind::ResourcePack => "resourcepacks",
            ContentKind::Shader => "shaderpacks",
            ContentKind::DataPack => "datapacks",
        }
    }

    pub fn uses_loaders(&self) -> bool {
        matches!(self, ContentKind::Mod | ContentKind::Modpack)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Client,
    Server,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub environment: Option<Environment>,
    #[serde(default)]
    pub open_source_only: bool,
    #[serde(default)]
    pub sort: SortOrder,
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    40
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            game_versions: Vec::new(),
            loaders: Vec::new(),
            categories: Vec::new(),
            environment: None,
            open_source_only: false,
            sort: SortOrder::default(),
            offset: 0,
            limit: default_limit(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub id: String,
    pub slug: Option<String>,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub author: String,
    pub categories: Vec<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub updated: Option<String>,
    pub color: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchPage {
    pub hits: Vec<ProjectSummary>,
    pub total: u32,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectLink {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GalleryImage {
    pub url: String,
    pub raw_url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub featured: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDetails {
    pub id: String,
    pub slug: Option<String>,
    pub title: String,
    pub description: String,
    pub body: String,
    pub body_format: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub author: String,
    pub gallery: Vec<GalleryImage>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub client_side: Option<String>,
    pub server_side: Option<String>,
    pub categories: Vec<String>,
    pub license: Option<String>,
    pub links: Vec<ProjectLink>,
    pub published: Option<String>,
    pub updated: Option<String>,
    pub website_url: Option<String>,
    pub color: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDependency {
    pub project_id: String,
    #[serde(default)]
    pub version_id: Option<String>,
    pub dependency_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionFile {
    pub url: Option<String>,
    pub file_name: String,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub size: Option<u64>,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub channel: String,
    pub date: String,
    pub downloads: u64,
    pub file_name: String,
    pub size: Option<u64>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub compatible: bool,
    pub changelog: Option<String>,
    pub dependencies: Vec<VersionDependency>,
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub server_pack_file_id: Option<String>,
}

impl ProjectVersion {
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }

    pub fn matches(&self, game_version: &str, loader: Option<&str>, kind: ContentKind) -> bool {
        let version_ok =
            game_version.is_empty() || self.game_versions.iter().any(|g| g == game_version);
        let loader_ok = !kind.uses_loaders()
            || loader
                .is_none_or(|l| self.loaders.is_empty() || self.loaders.iter().any(|x| x == l));
        version_ok && loader_ok
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Changelog {
    pub body: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilterOption {
    pub id: String,
    pub name: String,
    pub group: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilterTaxonomy {
    pub categories: Vec<FilterOption>,
    pub loaders: Vec<FilterOption>,
    pub game_versions: Vec<String>,
}

pub const KNOWN_LOADERS: &[&str] = &[
    "fabric",
    "forge",
    "neoforge",
    "quilt",
    "liteloader",
    "modloader",
    "rift",
    "bukkit",
    "spigot",
    "paper",
    "purpur",
    "sponge",
    "bungeecord",
    "velocity",
    "waterfall",
    "folia",
    "datapack",
    "minecraft",
    "iris",
    "optifine",
    "canvas",
    "vanilla",
];

pub const INSTALLABLE_LOADERS: &[&str] = &["fabric", "quilt", "neoforge", "forge"];

pub const PLUGIN_LOADERS: &[&str] = &[
    "bukkit",
    "spigot",
    "paper",
    "purpur",
    "folia",
    "sponge",
    "velocity",
    "waterfall",
    "bungeecord",
];

pub fn is_loader_token(value: &str) -> bool {
    let lowered = value.to_lowercase();
    KNOWN_LOADERS.contains(&lowered.as_str())
}

pub fn is_plugin_loader(value: &str) -> bool {
    let lowered = value.to_lowercase();
    PLUGIN_LOADERS.contains(&lowered.as_str())
}

pub fn is_installable_loader(value: &str) -> bool {
    let lowered = value.to_lowercase();
    INSTALLABLE_LOADERS.contains(&lowered.as_str())
}

pub fn looks_like_game_version(value: &str) -> bool {
    value.chars().next().is_some_and(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::{is_loader_token, looks_like_game_version, ContentKind, ProjectVersion};

    #[test]
    fn loader_tokens_reject_curseforge_noise() {
        assert!(is_loader_token("Fabric"));
        assert!(is_loader_token("NeoForge"));
        assert!(!is_loader_token("Client"));
        assert!(!is_loader_token("Server"));
        assert!(!is_loader_token("Java 17"));
        assert!(!is_loader_token("1.20.1"));
    }

    #[test]
    fn game_versions_start_with_a_digit() {
        assert!(looks_like_game_version("1.20.1"));
        assert!(looks_like_game_version("25w14a"));
        assert!(!looks_like_game_version("Fabric"));
    }

    fn version(game_versions: &[&str], loaders: &[&str]) -> ProjectVersion {
        ProjectVersion {
            server_pack_file_id: None,
            id: "v".into(),
            project_id: "p".into(),
            name: "v".into(),
            version_number: "1".into(),
            channel: "release".into(),
            date: String::new(),
            downloads: 0,
            file_name: "a.jar".into(),
            size: None,
            game_versions: game_versions.iter().map(|s| s.to_string()).collect(),
            loaders: loaders.iter().map(|s| s.to_string()).collect(),
            compatible: false,
            changelog: None,
            dependencies: Vec::new(),
            files: Vec::new(),
        }
    }

    #[test]
    fn compatibility_checks_version_and_loader() {
        let v = version(&["1.20.1", "1.20.4"], &["fabric"]);
        assert!(v.matches("1.20.1", Some("fabric"), ContentKind::Mod));
        assert!(!v.matches("1.21", Some("fabric"), ContentKind::Mod));
        assert!(!v.matches("1.20.1", Some("forge"), ContentKind::Mod));
        assert!(v.matches("1.20.1", Some("forge"), ContentKind::ResourcePack));
    }
}
