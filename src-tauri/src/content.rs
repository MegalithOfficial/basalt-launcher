use serde::Serialize;

use crate::db::ContentSource;
use crate::error::{Error, Result};
use crate::paths::Paths;

const DISABLED_SUFFIX: &str = ".disabled";

#[derive(Debug, Clone, Serialize)]
pub struct ContentItem {
    pub file_name: String,
    pub size: u64,
    pub enabled: bool,
    pub source: Option<ContentSource>,
} 

fn kind_subdir(kind: &str) -> Result<&'static str> {
    match kind {
        "mods" => Ok("mods"),
        "resourcepacks" => Ok("resourcepacks"),
        "shaderpacks" => Ok("shaderpacks"),
        "schematics" => Ok("schematics"),
        other => Err(Error::other(format!("unknown content kind {other}"))),
    }
}

fn validate_file_name(file_name: &str) -> Result<()> {
    if file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains("..")
    {
        return Err(Error::other("invalid file name"));
    }
    Ok(())
}

fn content_dir(paths: &Paths, instance_id: &str, kind: &str) -> Result<std::path::PathBuf> {
    Ok(paths.instance_dir(instance_id).join(kind_subdir(kind)?))
}

pub fn list(paths: &Paths, instance_id: &str, kind: &str) -> Result<Vec<ContentItem>> {
    let dir = content_dir(paths, instance_id, kind)?;
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };

    let mut items = Vec::new();
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let raw = entry.file_name().to_string_lossy().into_owned();
        let enabled = !raw.ends_with(DISABLED_SUFFIX);
        let file_name = raw
            .strip_suffix(DISABLED_SUFFIX)
            .unwrap_or(&raw)
            .to_string();
        items.push(ContentItem {
            file_name,
            size: meta.len(),
            enabled,
            source: None,
        });
    }
    items.sort_by(|a, b| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()));
    Ok(items)
}

pub fn toggle(paths: &Paths, instance_id: &str, kind: &str, file_name: &str) -> Result<bool> {
    validate_file_name(file_name)?;
    let dir = content_dir(paths, instance_id, kind)?;
    let enabled_path = dir.join(file_name);
    let disabled_path = dir.join(format!("{file_name}{DISABLED_SUFFIX}"));

    if enabled_path.is_file() {
        std::fs::rename(&enabled_path, &disabled_path)?;
        Ok(false)
    } else if disabled_path.is_file() {
        std::fs::rename(&disabled_path, &enabled_path)?;
        Ok(true)
    } else {
        Err(Error::NotFound(format!("content file {file_name}")))
    }
}

pub fn delete(paths: &Paths, instance_id: &str, kind: &str, file_name: &str) -> Result<()> {
    validate_file_name(file_name)?;
    let dir = content_dir(paths, instance_id, kind)?;
    let enabled_path = dir.join(file_name);
    let disabled_path = dir.join(format!("{file_name}{DISABLED_SUFFIX}"));
    if enabled_path.is_file() {
        std::fs::remove_file(enabled_path)?;
    } else if disabled_path.is_file() {
        std::fs::remove_file(disabled_path)?;
    }
    Ok(())
}

pub fn dir_for(paths: &Paths, instance_id: &str, kind: &str) -> Result<std::path::PathBuf> {
    content_dir(paths, instance_id, kind)
}

pub fn add(paths: &Paths, instance_id: &str, kind: &str, sources: &[String]) -> Result<usize> {
    let dir = content_dir(paths, instance_id, kind)?;
    std::fs::create_dir_all(&dir)?;
    let mut copied = 0;
    for source in sources {
        let source_path = std::path::Path::new(source);
        let Some(file_name) = source_path.file_name() else {
            continue;
        };
        if source_path.is_file() {
            std::fs::copy(source_path, dir.join(file_name))?;
            copied += 1;
        }
    }
    Ok(copied)
}
