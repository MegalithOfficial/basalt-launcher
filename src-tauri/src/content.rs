use serde::Serialize;

use crate::db::ContentFile;
use crate::error::{Error, Result};
use crate::files::FileManager;
use crate::paths::Paths;

const DISABLED_SUFFIX: &str = ".disabled";

#[derive(Debug, Clone, Serialize)]
pub struct ContentItem {
    pub file_name: String,
    pub size: u64,
    pub enabled: bool,
    pub source: Option<ContentFile>,
    pub update: Option<crate::db::ContentUpdate>,
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

pub fn list(files: &FileManager, instance_id: &str, kind: &str) -> Result<Vec<ContentItem>> {
    let paths = files.paths();
    let dir = content_dir(paths, instance_id, kind)?;
    let entries = match files.read_dir(&dir) {
        Ok(entries) => entries,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new())
        }
        Err(error) => return Err(error),
    };

    let mut items = Vec::new();
    for path in entries {
        let Ok(meta) = files.metadata(&path) else { continue };
        if !meta.is_file() {
            continue;
        }
        let raw = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
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
            update: None,
        });
    }
    items.sort_by(|a, b| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()));
    Ok(items)
}

pub fn toggle(files: &FileManager, instance_id: &str, kind: &str, file_name: &str) -> Result<bool> {
    let paths = files.paths();
    validate_file_name(file_name)?;
    let dir = content_dir(paths, instance_id, kind)?;
    let enabled_path = dir.join(file_name);
    let disabled_path = dir.join(format!("{file_name}{DISABLED_SUFFIX}"));

    if files.is_file(&enabled_path)? {
        files.rename(&enabled_path, &disabled_path)?;
        Ok(false)
    } else if files.is_file(&disabled_path)? {
        files.rename(&disabled_path, &enabled_path)?;
        Ok(true)
    } else {
        Err(Error::NotFound(format!("content file {file_name}")))
    }
}

pub fn delete(files: &FileManager, instance_id: &str, kind: &str, file_name: &str) -> Result<()> {
    let paths = files.paths();
    validate_file_name(file_name)?;
    let dir = content_dir(paths, instance_id, kind)?;
    let enabled_path = dir.join(file_name);
    let disabled_path = dir.join(format!("{file_name}{DISABLED_SUFFIX}"));
    if files.is_file(&enabled_path)? {
        files.remove_file_if_exists(enabled_path)?;
    } else if files.is_file(&disabled_path)? {
        files.remove_file_if_exists(disabled_path)?;
    }
    Ok(())
}

pub fn dir_for(paths: &Paths, instance_id: &str, kind: &str) -> Result<std::path::PathBuf> {
    content_dir(paths, instance_id, kind)
}

pub fn resolve_path(
    files: &FileManager,
    dir: &std::path::Path,
    file_name: &str,
) -> std::path::PathBuf {
    let enabled = dir.join(file_name);
    if files.is_file(&enabled).unwrap_or(false) {
        return enabled;
    }
    let disabled = dir.join(format!("{file_name}{DISABLED_SUFFIX}"));
    if files.is_file(&disabled).unwrap_or(false) {
        return disabled;
    }
    enabled
}

pub fn add(files: &FileManager, instance_id: &str, kind: &str, sources: &[String]) -> Result<usize> {
    let paths = files.paths();
    let dir = content_dir(paths, instance_id, kind)?;
    files.ensure_dir(&dir)?;
    let mut copied = 0;
    for source in sources {
        let source_path = std::path::Path::new(source);
        let Some(file_name) = source_path.file_name() else {
            continue;
        };
        if files.is_external_file(source_path) {
            files.copy_external_into_sync(source_path, dir.join(file_name))?;
            copied += 1;
        }
    }
    Ok(copied)
}
