mod atlauncher;
mod modrinth;
mod prism;

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    db::Db,
    error::{Error, Result},
    files::FileManager,
    tasks::TaskHandle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LauncherKind {
    Atlauncher,
    Prism,
    Modrinth,
}

impl LauncherKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Atlauncher => "atlauncher",
            Self::Prism => "prism",
            Self::Modrinth => "modrinth",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "atlauncher" => Ok(Self::Atlauncher),
            "prism" => Ok(Self::Prism),
            "modrinth" => Ok(Self::Modrinth),
            other => Err(Error::other(format!("unknown launcher {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LauncherSource {
    pub kind: LauncherKind,
    pub label: String,
    pub root: String,
    pub instance_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrationCandidate {
    pub id: String,
    pub name: String,
    pub version_id: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub icon_data_url: Option<String>,
    pub pack: Option<String>,
    pub mod_count: usize,
    pub file_count: usize,
    pub total_bytes: u64,
    pub last_played_ms: Option<i64>,
    pub warnings: Vec<String>,
    pub importable: bool,
    pub imported: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrationScan {
    pub kind: LauncherKind,
    pub root: String,
    pub candidates: Vec<MigrationCandidate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrationOutcome {
    pub imported: Vec<String>,
    pub failed: Vec<(String, String)>,
}

pub fn detect(files: &FileManager) -> Vec<LauncherSource> {
    [
        atlauncher::detect(files),
        prism::detect(files),
        modrinth::detect(files),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn scan(
    files: &FileManager,
    db: &Db,
    kind: LauncherKind,
    root: &Path,
) -> Result<MigrationScan> {
    let already: std::collections::HashSet<String> = db
        .imported_sources(kind.as_str())
        .unwrap_or_default()
        .into_iter()
        .collect();

    let mut scan = match kind {
        LauncherKind::Atlauncher => atlauncher::scan(files, root)?,
        LauncherKind::Prism => prism::scan(files, root)?,
        LauncherKind::Modrinth => modrinth::scan(files, root)?,
    };

    for candidate in &mut scan.candidates {
        if already.contains(&candidate.id) {
            candidate.imported = true;
            candidate.importable = false;
        }
    }
    Ok(scan)
}

pub fn import(
    files: &FileManager,
    db: &Db,
    kind: LauncherKind,
    root: &Path,
    ids: &[String],
    task: &TaskHandle,
) -> Result<MigrationOutcome> {
    match kind {
        LauncherKind::Atlauncher => atlauncher::import(files, db, root, ids, task),
        LauncherKind::Prism => prism::import(files, db, root, ids, task),
        LauncherKind::Modrinth => modrinth::import(files, db, root, ids, task),
    }
}

pub(super) fn relative_within(root: &Path, path: &Path) -> Option<PathBuf> {
    let relative = path.strip_prefix(root).ok()?;
    if relative
        .components()
        .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return None;
    }
    Some(relative.to_path_buf())
}

pub(super) fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub(super) fn candidate_roots(segments: &[&str]) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
        roots.push(PathBuf::from(data));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        roots.push(PathBuf::from(appdata));
    }
    if let Some(home) = home_dir() {
        roots.push(home.join(".local").join("share"));
        roots.push(home.join("Library").join("Application Support"));
        roots.push(home);
    }

    roots
        .into_iter()
        .flat_map(|root| segments.iter().map(move |name| root.join(name)))
        .collect()
}

/// Walks a directory tree, skipping symbolic links and special files rather than failing on
/// them, since a game folder legitimately holds whatever mods decided to write there.
pub(super) fn walk_files(
    files: &FileManager,
    root: &Path,
    skip: &dyn Fn(&Path) -> bool,
) -> Result<Vec<(PathBuf, u64)>> {
    let mut pending = vec![root.to_path_buf()];
    let mut output = Vec::new();

    while let Some(directory) = pending.pop() {
        let entries = match files.read_external_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for path in entries {
            if skip(&path) {
                continue;
            }
            let Ok(metadata) = files.external_symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                output.push((path, metadata.len()));
            }
        }
    }

    Ok(output)
}
