use serde::Serialize;

use crate::{
    db::ContentFile,
    error::{Error, Result},
    files::FileManager,
    paths::Paths,
};

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
    let dir = content_dir(files.paths(), instance_id, kind)?;
    list_in(files, &dir)
}

pub fn list_in(files: &FileManager, dir: &std::path::Path) -> Result<Vec<ContentItem>> {
    let entries = match files.read_dir(dir) {
        Ok(entries) => entries,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new())
        }
        Err(error) => return Err(error),
    };

    let mut items = Vec::new();
    for path in entries {
        let Ok(meta) = files.metadata(&path) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let raw = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if raw.ends_with(crate::download::PART_SUFFIX) {
            continue;
        }
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
    items.sort_by_key(|item| item.file_name.to_lowercase());
    Ok(items)
}

pub fn toggle(files: &FileManager, instance_id: &str, kind: &str, file_name: &str) -> Result<bool> {
    let dir = content_dir(files.paths(), instance_id, kind)?;
    toggle_in(files, &dir, file_name)
}

pub fn toggle_in(files: &FileManager, dir: &std::path::Path, file_name: &str) -> Result<bool> {
    validate_file_name(file_name)?;
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
    let dir = content_dir(files.paths(), instance_id, kind)?;
    delete_in(files, &dir, file_name)
}

pub fn delete_in(files: &FileManager, dir: &std::path::Path, file_name: &str) -> Result<()> {
    validate_file_name(file_name)?;
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

pub fn add(
    files: &FileManager,
    instance_id: &str,
    kind: &str,
    sources: &[String],
) -> Result<usize> {
    let dir = content_dir(files.paths(), instance_id, kind)?;
    add_into(files, &dir, sources)
}

pub fn add_into(files: &FileManager, dir: &std::path::Path, sources: &[String]) -> Result<usize> {
    files.ensure_dir(dir)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_hides_incomplete_downloads() {
        let root = std::env::temp_dir().join(format!("basalt-content-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let files = FileManager::new(Paths::plain(root.clone())).unwrap();
        let mods = files.paths().instance_dir("test").join("mods");
        files.ensure_dir(&mods).unwrap();
        files
            .write_atomic(mods.join("ready.jar"), b"ready")
            .unwrap();
        files
            .write_atomic(
                mods.join(format!("downloading.jar{}", crate::download::PART_SUFFIX)),
                b"partial",
            )
            .unwrap();

        let items = list(&files, "test", "mods").unwrap();

        assert_eq!(
            items
                .into_iter()
                .map(|item| item.file_name)
                .collect::<Vec<_>>(),
            ["ready.jar"]
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
