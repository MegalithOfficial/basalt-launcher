use std::path::{Path, PathBuf};

use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::{
    error::{Error, Result},
    state::AppState,
};

const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];
const THUMBNAIL_WIDTH: u32 = 420;

#[derive(Debug, serde::Serialize)]
pub struct Screenshot {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub modified_ms: i64,
    pub thumbnail: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct Thumbnail {
    pub name: String,
    pub path: Option<String>,
}

fn thumbnails_dir(state: &AppState) -> PathBuf {
    state.paths.media().join("thumbnails")
}

fn thumbnail_for(directory: &Path, source: &Path, modified_ms: i64, size_bytes: u64) -> PathBuf {
    let mut hasher = sha1_smol::Sha1::new();
    hasher.update(source.to_string_lossy().as_bytes());
    hasher.update(&modified_ms.to_le_bytes());
    hasher.update(&size_bytes.to_le_bytes());
    hasher.update(&THUMBNAIL_WIDTH.to_le_bytes());
    directory.join(format!("{}.jpg", hasher.digest()))
}

fn build_thumbnail(source: &Path, destination: &Path) -> Result<()> {
    let image = image::open(source)
        .map_err(|error| Error::other(format!("could not read the image: {error}")))?;
    let small = image.thumbnail(THUMBNAIL_WIDTH, THUMBNAIL_WIDTH);

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let staging = destination.with_extension("jpg.part");
    small
        .into_rgb8()
        .save_with_format(&staging, image::ImageFormat::Jpeg)
        .map_err(|error| Error::other(format!("could not write the preview: {error}")))?;
    std::fs::rename(&staging, destination)?;
    Ok(())
}

fn screenshots_dir(state: &AppState, instance_id: &str) -> Result<PathBuf> {
    let instance = super::find_instance(state, instance_id)?;
    Ok(PathBuf::from(instance.dir).join("screenshots"))
}

fn screenshot_path(state: &AppState, instance_id: &str, name: &str) -> Result<PathBuf> {
    let lower = name.to_ascii_lowercase();
    let known = EXTENSIONS
        .iter()
        .any(|extension| lower.ends_with(&format!(".{extension}")));
    if !known || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(Error::other(format!("not a screenshot: {name}")));
    }

    let dir = screenshots_dir(state, instance_id)?;
    let path = dir.join(name);
    if path.parent() != Some(dir.as_path()) {
        return Err(Error::other(format!("not a screenshot: {name}")));
    }
    Ok(path)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn list_screenshots(state: State<AppState>, instance_id: String) -> Result<Vec<Screenshot>> {
    let dir = screenshots_dir(&state, &instance_id)?;
    let thumbnails = thumbnails_dir(&state);
    let mut shots = Vec::new();

    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(shots);
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let lower = name.to_ascii_lowercase();
        if !EXTENSIONS
            .iter()
            .any(|extension| lower.ends_with(&format!(".{extension}")))
        {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_millis() as i64)
            .unwrap_or(0);
        let source = entry.path();
        let thumbnail = thumbnail_for(&thumbnails, &source, modified_ms, meta.len());
        shots.push(Screenshot {
            name,
            path: source.display().to_string(),
            size_bytes: meta.len(),
            modified_ms,
            thumbnail: thumbnail.is_file().then(|| thumbnail.display().to_string()),
        });
    }

    shots.sort_by(|a, b| {
        b.modified_ms
            .cmp(&a.modified_ms)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(shots)
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub fn delete_screenshots(
    state: State<AppState>,
    instance_id: String,
    names: Vec<String>,
) -> Result<usize> {
    let directory = thumbnails_dir(&state);
    let mut removed = 0;
    for name in &names {
        let path = screenshot_path(&state, &instance_id, name)?;
        if let Ok(meta) = path.metadata() {
            let modified_ms = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|since| since.as_millis() as i64)
                .unwrap_or(0);
            let _ = std::fs::remove_file(thumbnail_for(&directory, &path, modified_ms, meta.len()));
        }
        match std::fs::remove_file(&path) {
            Ok(()) => removed += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    tracing::info!(removed, "screenshots deleted");
    Ok(removed)
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub fn copy_screenshot(
    app: AppHandle,
    state: State<AppState>,
    instance_id: String,
    name: String,
) -> Result<()> {
    let path = screenshot_path(&state, &instance_id, &name)?;
    let image = tauri::image::Image::from_path(&path)
        .map_err(|error| Error::other(format!("could not read {name}: {error}")))?;
    app.clipboard()
        .write_image(&image)
        .map_err(|error| Error::other(format!("could not reach the clipboard: {error}")))
}

#[tauri::command]
#[tracing::instrument(skip(state), err)]
pub async fn ensure_thumbnails(
    state: State<'_, AppState>,
    instance_id: String,
    names: Vec<String>,
) -> Result<Vec<Thumbnail>> {
    let directory = thumbnails_dir(&state);
    let mut jobs = Vec::new();
    for name in names {
        let source = screenshot_path(&state, &instance_id, &name)?;
        let Ok(meta) = source.metadata() else {
            continue;
        };
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_millis() as i64)
            .unwrap_or(0);
        let destination = thumbnail_for(&directory, &source, modified_ms, meta.len());
        jobs.push((name, source, destination));
    }

    let running: Vec<_> = jobs
        .into_iter()
        .map(|(name, source, destination)| {
            tokio::task::spawn_blocking(move || {
                if !destination.is_file() {
                    if let Err(error) = build_thumbnail(&source, &destination) {
                        tracing::warn!(file = %name, error = %error, "no preview for this screenshot");
                        return Thumbnail { name, path: None };
                    }
                }
                Thumbnail {
                    name,
                    path: Some(destination.display().to_string()),
                }
            })
        })
        .collect();

    let mut built = Vec::with_capacity(running.len());
    for handle in running {
        if let Ok(thumbnail) = handle.await {
            built.push(thumbnail);
        }
    }
    Ok(built)
}
