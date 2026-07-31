mod export;
mod import;

pub use export::{export_instance, PackExport};
pub(crate) use import::plan_curseforge_archive;
pub use import::{finish_import, inspect_pack, prepare_import, PackPreview};

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const CONTENT_DIRS: [&str; 4] = ["mods", "resourcepacks", "shaderpacks", "schematics"];

const SKIP_DIRS: [&str; 8] = [
    "saves",
    "logs",
    "crash-reports",
    "screenshots",
    "backups",
    ".fabric",
    "downloads",
    "versions",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackFormat {
    Mrpack,
    Curseforge,
}

impl PackFormat {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "mrpack" | "modrinth" => Ok(Self::Mrpack),
            "curseforge" => Ok(Self::Curseforge),
            other => Err(Error::other(format!("unknown pack format {other}"))),
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Mrpack => "mrpack",
            Self::Curseforge => "zip",
        }
    }
}

pub(super) fn loader_dependency_key(loader: &str) -> Option<&'static str> {
    match loader {
        "fabric" => Some("fabric-loader"),
        "quilt" => Some("quilt-loader"),
        "forge" => Some("forge"),
        "neoforge" => Some("neoforge"),
        _ => None,
    }
}

pub(super) fn is_skipped(relative: &Path) -> bool {
    relative
        .components()
        .next()
        .and_then(|part| part.as_os_str().to_str())
        .map(|name| SKIP_DIRS.contains(&name))
        .unwrap_or(false)
}

pub(super) fn is_content_path(relative: &Path) -> bool {
    relative
        .components()
        .next()
        .and_then(|part| part.as_os_str().to_str())
        .map(|name| CONTENT_DIRS.contains(&name))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{is_content_path, is_skipped, loader_dependency_key, PackFormat};

    #[test]
    fn classifies_instance_paths() {
        assert!(is_content_path(Path::new("mods/sodium.jar")));
        assert!(!is_content_path(Path::new("config/sodium.json")));
        assert!(is_skipped(Path::new("saves/world/level.dat")));
        assert!(!is_skipped(Path::new("config/options.txt")));
    }

    #[test]
    fn maps_loader_keys() {
        assert_eq!(loader_dependency_key("fabric"), Some("fabric-loader"));
        assert_eq!(loader_dependency_key("neoforge"), Some("neoforge"));
        assert_eq!(loader_dependency_key("vanilla"), None);
        assert_eq!(PackFormat::parse("mrpack").unwrap().extension(), "mrpack");
    }
}
