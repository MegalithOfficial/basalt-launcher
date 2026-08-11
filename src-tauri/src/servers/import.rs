use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    error::{Error, Result},
    paths::{DataRoot, Paths},
};

use super::{
    Software,
    software::{self, Folder},
};

#[derive(Debug, Clone, Serialize)]
pub struct ServerFolder {
    pub path: String,
    pub name: String,
    pub flavor: Option<Software>,
    pub version_id: Option<String>,
    pub flavor_version: Option<String>,
    pub launch_jar: Option<String>,
    pub launch_argfiles: Vec<String>,
    pub eula_accepted: bool,
    pub port: Option<u16>,
}

pub fn validate(paths: &Paths, target: &Path) -> Result<PathBuf> {
    if target.as_os_str().is_empty() {
        return Err(Error::other("Pick a folder first."));
    }
    if !target.is_absolute() {
        return Err(Error::other("Pick a full path, not a relative one."));
    }
    if !target.is_dir() {
        return Err(Error::other("That path is not a folder."));
    }
    let canonical = target.canonicalize()?;
    if paths.root.starts_with(&canonical) {
        return Err(Error::other(
            "That folder holds the Basalt data directory. Pick the server folder itself.",
        ));
    }
    for slot in DataRoot::ALL {
        let located = paths.located_at(slot);
        if located.starts_with(&canonical) {
            return Err(Error::other(format!(
                "That folder holds the {} directory. Pick the server folder itself.",
                slot.label().to_lowercase()
            )));
        }
    }
    if canonical.parent().is_none() {
        return Err(Error::other("Pick a folder, not a whole drive."));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    if home.is_some_and(|home| home == canonical) {
        return Err(Error::other(
            "Pick the server folder, not your home folder.",
        ));
    }
    Ok(canonical)
}

pub fn inspect(dir: &Path) -> ServerFolder {
    let entries = std::fs::read_dir(dir)
        .map(|read| {
            read.flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut folder = ServerFolder {
        path: dir.display().to_string(),
        name: dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Server".to_string()),
        flavor: None,
        version_id: None,
        flavor_version: None,
        launch_jar: None,
        launch_argfiles: Vec::new(),
        eula_accepted: eula_accepted(dir),
        port: None,
    };

    if let Some((software, found)) = software::detect(&Folder {
        dir,
        entries: &entries,
    }) {
        folder.flavor = Some(software);
        folder.version_id = found.version_id;
        folder.flavor_version = found.flavor_version;
        folder.launch_jar = found.launch_jar;
        folder.launch_argfiles = found.launch_argfiles;
    }

    if folder.launch_jar.is_none() && folder.launch_argfiles.is_empty() {
        folder.launch_jar = fallback_jar(&entries);
    }

    if let Ok(bytes) = std::fs::read(dir.join("server.properties")) {
        let properties = super::properties::Properties::parse(&bytes);
        folder.port = properties
            .get("server-port")
            .and_then(|value| value.trim().parse().ok());
    }

    folder
}

pub fn fallback_jar(entries: &[String]) -> Option<String> {
    entries
        .iter()
        .find(|name| *name == "server.jar")
        .or_else(|| {
            entries
                .iter()
                .find(|name| name.starts_with("forge-") && name.ends_with(".jar"))
        })
        .or_else(|| {
            entries
                .iter()
                .find(|name| name.starts_with("minecraft_server") && name.ends_with(".jar"))
        })
        .cloned()
}

fn eula_accepted(dir: &Path) -> bool {
    std::fs::read(dir.join("eula.txt"))
        .map(|bytes| {
            super::properties::Properties::parse(&bytes)
                .get("eula")
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
        })
        .unwrap_or(false)
}

pub struct Modded {
    pub version: String,
    pub argfiles: Vec<String>,
}

pub fn modded_launch(dir: &Path, coordinates: &str) -> Option<Modded> {
    let parent = dir.join("libraries").join(coordinates);
    let mut versions = std::fs::read_dir(&parent)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    versions.sort();
    let version = versions.pop()?;

    let relative = format!(
        "libraries/{coordinates}/{version}/{}_args.txt",
        super::provision::PLATFORM_PLACEHOLDER
    );
    if !dir
        .join(super::provision::argfile_for_platform(&relative))
        .is_file()
    {
        return None;
    }

    let mut argfiles = Vec::new();
    if dir.join("user_jvm_args.txt").is_file() {
        argfiles.push("user_jvm_args.txt".to_string());
    }
    argfiles.push(relative);
    Some(Modded { version, argfiles })
}

pub struct History {
    pub purpur: bool,
    pub game_version: String,
    pub build: Option<String>,
}

pub fn version_history(dir: &Path) -> Option<History> {
    let bytes = std::fs::read(dir.join("version_history.json")).ok()?;
    let history: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let current = history.get("currentVersion")?.as_str()?;
    let game_version = current
        .split_once("(MC: ")
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(version, _)| version.trim().to_string())?;
    let build = current
        .split_whitespace()
        .find_map(|token| token.rsplit_once('-'))
        .and_then(|(_, build)| build.parse::<u32>().ok())
        .map(|build| build.to_string());
    Some(History {
        purpur: current.to_ascii_lowercase().contains("purpur"),
        game_version,
        build,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("basalt-{name}-{}", uuid::Uuid::new_v4()));
        let dir = root.join("smp");
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_paper_folder_is_recognised_from_its_version_history() {
        let dir = sandbox("import-paper");
        std::fs::write(
            dir.join("version_history.json"),
            br#"{"currentVersion": "git-Paper-60 (MC: 1.21.8)"}"#,
        )
        .unwrap();
        std::fs::write(dir.join("server.jar"), b"jar").unwrap();
        std::fs::write(dir.join("server.properties"), b"server-port=25580\n").unwrap();
        std::fs::write(dir.join("eula.txt"), b"eula=true\n").unwrap();

        let folder = inspect(&dir);

        assert_eq!(folder.flavor.unwrap().id(), "paper");
        assert_eq!(folder.version_id.as_deref(), Some("1.21.8"));
        assert_eq!(folder.flavor_version.as_deref(), Some("60"));
        assert_eq!(folder.launch_jar.as_deref(), Some("server.jar"));
        assert_eq!(folder.port, Some(25580));
        assert!(folder.eula_accepted);
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }

    #[test]
    fn a_neoforge_folder_is_recognised_from_its_argument_files() {
        let dir = sandbox("import-neoforge");
        let libraries = dir.join("libraries/net/neoforged/neoforge/21.8.54");
        std::fs::create_dir_all(&libraries).unwrap();
        for name in ["unix_args.txt", "win_args.txt"] {
            std::fs::write(libraries.join(name), b"-cp\n").unwrap();
        }
        std::fs::write(dir.join("user_jvm_args.txt"), b"# nothing\n").unwrap();

        let folder = inspect(&dir);

        assert_eq!(folder.flavor.unwrap().id(), "neoforge");
        assert_eq!(folder.flavor_version.as_deref(), Some("21.8.54"));
        assert_eq!(folder.launch_argfiles.len(), 2);
        assert!(folder.launch_jar.is_none());
        assert!(!folder.eula_accepted);
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }

    #[test]
    fn a_plain_folder_falls_back_to_vanilla() {
        let dir = sandbox("import-vanilla");
        std::fs::write(dir.join("server.jar"), b"jar").unwrap();

        let folder = inspect(&dir);

        assert_eq!(folder.flavor.unwrap().id(), "vanilla");
        assert!(folder.version_id.is_none());
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }

    #[test]
    fn folders_that_swallow_basalt_are_refused() {
        let root = std::env::temp_dir().join(format!("basalt-import-{}", uuid::Uuid::new_v4()));
        let data = root.join("data");
        std::fs::create_dir_all(&data).unwrap();
        let paths = Paths::plain(data.clone());

        assert!(validate(&paths, &root).is_err());
        assert!(validate(&paths, &data).is_err());
        assert!(validate(&paths, Path::new("relative")).is_err());
        assert!(validate(&paths, &root.join("missing")).is_err());

        let server = root.join("smp");
        std::fs::create_dir_all(&server).unwrap();
        assert!(validate(&paths, &server).is_ok());
        std::fs::remove_dir_all(root).ok();
    }
}
