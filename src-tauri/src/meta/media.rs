use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::paths::Paths;

const PATCH_NOTES_URL: &str = "https://launchercontent.mojang.com/v2/javaPatchNotes.json";
const CONTENT_BASE: &str = "https://launchercontent.mojang.com";

#[derive(Debug, Clone, Deserialize)]
pub struct PatchNotes {
    pub entries: Vec<PatchEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PatchEntry {
    pub version: String,
    #[serde(default)]
    pub image: Option<PatchImage>,
    #[serde(rename = "shortText", default)]
    pub short_text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PatchImage {
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionMedia {
    pub image_url: String,
    pub short_text: Option<String>,
    pub accent: Option<String>,
    pub local: bool,
}

const BANNER_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];

pub async fn fetch_notes(client: &reqwest::Client, paths: &Paths) -> Result<PatchNotes> {
    let cache = paths.root.join("patch_notes.json");
    let bytes = match client.get(PATCH_NOTES_URL).send().await {
        Ok(resp) => {
            let resp = resp.error_for_status()?;
            let bytes = resp.bytes().await?;
            let _ = tokio::fs::write(&cache, &bytes).await;
            bytes.to_vec()
        }
        Err(_) => tokio::fs::read(&cache).await?,
    };
    Ok(serde_json::from_slice(&bytes)?)
}

fn accent_from_pixels(img: &image::RgbImage) -> Option<String> {
    let mut buckets = [(0f32, 0f32, 0f32, 0f32); 36];
    for pixel in img.pixels() {
        let (r, g, b) = (
            pixel[0] as f32 / 255.0,
            pixel[1] as f32 / 255.0,
            pixel[2] as f32 / 255.0,
        );
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        if max < 0.15 || delta < 0.12 {
            continue;
        }
        let sat = delta / max;
        if sat < 0.25 {
            continue;
        }
        let hue = if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        let hue = if hue < 0.0 { hue + 360.0 } else { hue };
        let weight = sat * max;
        let bucket = &mut buckets[(hue as usize / 10).min(35)];
        bucket.0 += weight;
        bucket.1 += r * weight;
        bucket.2 += g * weight;
        bucket.3 += b * weight;
    }

    let best = buckets
        .iter()
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .filter(|b| b.0 > 1.0)?;
    let (w, r, g, b) = (best.0, best.1 / best.0, best.2 / best.0, best.3 / best.0);
    let _ = w;

    let max = r.max(g).max(b);
    let boost = if max > 0.0 { (0.82 / max).clamp(1.0, 1.8) } else { 1.0 };
    let to_byte = |v: f32| ((v * boost).clamp(0.0, 1.0) * 255.0) as u8;
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        to_byte(r),
        to_byte(g),
        to_byte(b)
    ))
}

async fn accent_for(
    client: &reqwest::Client,
    paths: &Paths,
    version_id: &str,
    image_url: &str,
) -> Option<String> {
    let media_dir = paths.root.join("media");
    let img_path = media_dir.join(format!("{version_id}.jpg"));
    let accent_path = media_dir.join(format!("{version_id}.accent"));

    if let Ok(cached) = tokio::fs::read_to_string(&accent_path).await {
        let cached = cached.trim().to_string();
        return if cached.is_empty() { None } else { Some(cached) };
    }

    let bytes = match tokio::fs::read(&img_path).await {
        Ok(bytes) => bytes,
        Err(_) => {
            let resp = client.get(image_url).send().await.ok()?.error_for_status().ok()?;
            let bytes = resp.bytes().await.ok()?.to_vec();
            let _ = tokio::fs::create_dir_all(&media_dir).await;
            let _ = tokio::fs::write(&img_path, &bytes).await;
            bytes
        }
    };

    let accent = compute_accent(bytes).await;
    let _ = tokio::fs::write(&accent_path, accent.clone().unwrap_or_default()).await;
    accent
}

async fn compute_accent(bytes: Vec<u8>) -> Option<String> {
    tokio::task::spawn_blocking(move || {
        let img = image::load_from_memory(&bytes).ok()?;
        let small = img.resize_exact(32, 32, image::imageops::FilterType::Triangle);
        accent_from_pixels(&small.to_rgb8())
    })
    .await
    .ok()
    .flatten()
}

fn banner_paths(paths: &Paths, instance_id: &str) -> Vec<std::path::PathBuf> {
    let media_dir = paths.root.join("media");
    BANNER_EXTENSIONS
        .iter()
        .map(|ext| media_dir.join(format!("instance-{instance_id}.{ext}")))
        .collect()
}

fn banner_accent_path(paths: &Paths, instance_id: &str) -> std::path::PathBuf {
    paths.root.join("media").join(format!("instance-{instance_id}.accent"))
}

pub async fn custom_banner(paths: &Paths, instance_id: &str) -> Option<VersionMedia> {
    let img_path = {
        let mut found = None;
        for candidate in banner_paths(paths, instance_id) {
            if candidate.is_file() {
                found = Some(candidate);
                break;
            }
        }
        found?
    };

    let accent_path = banner_accent_path(paths, instance_id);
    let accent = match tokio::fs::read_to_string(&accent_path).await {
        Ok(cached) => {
            let cached = cached.trim().to_string();
            if cached.is_empty() { None } else { Some(cached) }
        }
        Err(_) => {
            let bytes = tokio::fs::read(&img_path).await.ok()?;
            let accent = compute_accent(bytes).await;
            let _ = tokio::fs::write(&accent_path, accent.clone().unwrap_or_default()).await;
            accent
        }
    };

    Some(VersionMedia {
        image_url: img_path.display().to_string(),
        short_text: None,
        accent,
        local: true,
    })
}

pub async fn set_custom_banner(
    paths: &Paths,
    instance_id: &str,
    source: &str,
) -> crate::error::Result<VersionMedia> {
    use crate::error::Error;

    let source_path = std::path::Path::new(source);
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if !BANNER_EXTENSIONS.contains(&ext.as_str()) {
        return Err(Error::other(format!(
            "Unsupported image type .{ext}. Use png, jpg, webp, or gif."
        )));
    }

    clear_custom_banner(paths, instance_id).await;

    let media_dir = paths.root.join("media");
    tokio::fs::create_dir_all(&media_dir).await?;
    let dest = media_dir.join(format!("instance-{instance_id}.{ext}"));
    tokio::fs::copy(source_path, &dest).await?;

    let bytes = tokio::fs::read(&dest).await?;
    let accent = compute_accent(bytes).await;
    let _ = tokio::fs::write(
        banner_accent_path(paths, instance_id),
        accent.clone().unwrap_or_default(),
    )
    .await;

    Ok(VersionMedia {
        image_url: dest.display().to_string(),
        short_text: None,
        accent,
        local: true,
    })
}

pub async fn clear_custom_banner(paths: &Paths, instance_id: &str) {
    for path in banner_paths(paths, instance_id) {
        let _ = tokio::fs::remove_file(path).await;
    }
    let _ = tokio::fs::remove_file(banner_accent_path(paths, instance_id)).await;
}

pub async fn media_for(
    client: &reqwest::Client,
    paths: &Paths,
    notes: &PatchNotes,
    version_id: &str,
) -> Option<VersionMedia> {
    let entry = notes.entries.iter().find(|e| e.version == version_id)?;
    let image = entry.image.as_ref()?;
    let image_url = format!("{CONTENT_BASE}{}", image.url);
    let accent = accent_for(client, paths, version_id, &image_url).await;
    Some(VersionMedia {
        image_url,
        short_text: entry.short_text.clone(),
        accent,
        local: false,
    })
}
