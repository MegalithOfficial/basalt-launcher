use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    db::{BannerRecord, Db},
    error::{Error, Result},
    files::FileManager,
};

pub const MAX_BYTES: u64 = 100 * 1024 * 1024;

pub const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];
pub const VIDEO_EXTENSIONS: [&str; 4] = ["mp4", "webm", "mkv", "mov"];

#[derive(Debug, Clone, Serialize)]
pub struct BannerEntry {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub original_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bytes: i64,
    pub accent: Option<String>,
    pub added_at: i64,
    pub in_use_by: Vec<String>,
}

pub fn kind_for(extension: &str) -> Option<&'static str> {
    let lower = extension.to_lowercase();
    if IMAGE_EXTENSIONS.contains(&lower.as_str()) {
        Some("image")
    } else if VIDEO_EXTENSIONS.contains(&lower.as_str()) {
        Some("video")
    } else {
        None
    }
}

pub fn library_path(files: &FileManager, record: &BannerRecord) -> PathBuf {
    files.paths().banner_library().join(&record.file_name)
}

fn entry_from(files: &FileManager, record: BannerRecord, in_use_by: Vec<String>) -> BannerEntry {
    BannerEntry {
        path: library_path(files, &record).display().to_string(),
        id: record.id,
        kind: record.kind,
        original_name: record.original_name,
        width: record.width,
        height: record.height,
        bytes: record.bytes,
        accent: record.accent,
        added_at: record.added_at,
        in_use_by,
    }
}

pub fn list(files: &FileManager, db: &Db) -> Result<Vec<BannerEntry>> {
    let mut entries = Vec::new();
    for record in db.list_banners()? {
        let users = db.banner_users(&record.id).unwrap_or_default();
        entries.push(entry_from(files, record, users));
    }
    Ok(entries)
}

pub async fn import(files: &FileManager, db: &Db, source: &str) -> Result<BannerEntry> {
    let source_path = Path::new(source);
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_lowercase())
        .unwrap_or_default();

    let kind = kind_for(&extension).ok_or_else(|| {
        Error::other(format!(
            "Basalt cannot use .{extension} as a banner. Try png, jpg, webp, gif, mp4 or webm."
        ))
    })?;

    let size = files.external_symlink_metadata(source_path)?.len();
    if size > MAX_BYTES {
        return Err(Error::other(format!(
            "That file is {} MB. Banners are capped at {} MB.",
            size / (1024 * 1024),
            MAX_BYTES / (1024 * 1024)
        )));
    }

    let bytes = files.read_external_async(source_path).await?;
    let id = {
        let mut hasher = sha1_smol::Sha1::new();
        hasher.update(&bytes);
        hasher.digest().to_string()
    };
    let file_name = format!("{id}.{extension}");

    let library = files.paths().banner_library();
    files.ensure_dir_async(&library).await?;
    let destination = library.join(&file_name);
    if !files.exists(&destination).unwrap_or(false) {
        files
            .write_atomic_async(&destination, bytes.clone())
            .await?;
    }

    let (width, height, accent) = if kind == "image" {
        probe_image(bytes).await
    } else {
        (None, None, None)
    };

    let record = BannerRecord {
        id: id.clone(),
        file_name,
        original_name: source_path
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.to_string()),
        kind: kind.to_string(),
        width,
        height,
        bytes: size as i64,
        accent,
        added_at: chrono::Utc::now().timestamp(),
    };
    db.insert_banner(&record)?;

    let users = db.banner_users(&record.id).unwrap_or_default();
    Ok(entry_from(files, record, users))
}

pub async fn remove(files: &FileManager, db: &Db, id: &str) -> Result<()> {
    let Some(record) = db.banner(id)? else {
        return Ok(());
    };
    db.delete_banner(id)?;
    let _ = files
        .remove_file_if_exists_async(library_path(files, &record))
        .await;
    Ok(())
}

pub fn adopt_legacy_banners(files: &FileManager, db: &Db) -> Result<usize> {
    let media = files.paths().media();
    let library = files.paths().banner_library();
    files.ensure_dir(&library)?;

    let entries = match files.read_dir(&media) {
        Ok(entries) => entries,
        Err(_) => return Ok(0),
    };

    let mut adopted = 0;
    for path in entries {
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some(instance_id) = stem.strip_prefix("instance-") else {
            continue;
        };
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        let extension = extension.to_lowercase();
        let Some(kind) = kind_for(&extension) else {
            continue;
        };
        if db.instance_banner_id(instance_id)?.is_some() {
            continue;
        }

        let Ok(bytes) = files.read(&path) else {
            continue;
        };
        let id = {
            let mut hasher = sha1_smol::Sha1::new();
            hasher.update(&bytes);
            hasher.digest().to_string()
        };
        let file_name = format!("{id}.{extension}");
        let destination = library.join(&file_name);
        if !files.exists(&destination).unwrap_or(false) {
            files.write_atomic(&destination, &bytes)?;
        }

        let accent = files
            .read(media.join(format!("instance-{instance_id}.accent")))
            .ok()
            .and_then(|raw| String::from_utf8(raw).ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        db.insert_banner(&BannerRecord {
            id: id.clone(),
            file_name,
            original_name: None,
            kind: kind.to_string(),
            width: None,
            height: None,
            bytes: bytes.len() as i64,
            accent,
            added_at: chrono::Utc::now().timestamp(),
        })?;
        db.set_instance_banner_id(instance_id, Some(&id))?;

        let _ = files.remove_file_if_exists(&path);
        let _ = files.remove_file_if_exists(media.join(format!("instance-{instance_id}.accent")));
        adopted += 1;
    }

    Ok(adopted)
}

pub fn media_for_instance(
    files: &FileManager,
    db: &Db,
    instance_id: &str,
) -> Option<crate::meta::media::VersionMedia> {
    let id = db.instance_banner_id(instance_id).ok().flatten()?;
    let record = db.banner(&id).ok().flatten()?;
    let path = library_path(files, &record);
    if !files.is_file(&path).unwrap_or(false) {
        return None;
    }
    Some(crate::meta::media::VersionMedia {
        image_url: path.display().to_string(),
        short_text: None,
        accent: record.accent,
        local: true,
        kind: record.kind,
    })
}

async fn probe_image(bytes: Vec<u8>) -> (Option<u32>, Option<u32>, Option<String>) {
    tokio::task::spawn_blocking(move || {
        let Ok(image) = image::load_from_memory(&bytes) else {
            return (None, None, None);
        };
        let width = image.width();
        let height = image.height();
        let small = image.resize_exact(32, 32, image::imageops::FilterType::Triangle);
        let accent = super::media::accent_from_pixels(&small.to_rgb8());
        (Some(width), Some(height), accent)
    })
    .await
    .unwrap_or((None, None, None))
}

#[cfg(test)]
mod tests {
    use super::kind_for;

    #[test]
    fn separates_images_from_videos() {
        assert_eq!(kind_for("PNG"), Some("image"));
        assert_eq!(kind_for("gif"), Some("image"));
        assert_eq!(kind_for("mp4"), Some("video"));
        assert_eq!(kind_for("webm"), Some("video"));
        assert_eq!(kind_for("txt"), None);
    }
}
