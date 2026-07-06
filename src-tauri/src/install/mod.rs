use std::io::Read;
use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::download::{self, DownloadProgress, DownloadSpec};
use crate::error::{Error, Result};
use crate::meta::manifest;
use crate::meta::version::{merge_versions, AssetIndex, NativeSpec, VersionJson};
use crate::state::AppState;

#[derive(Clone, Serialize)]
struct StagePayload {
    instance_id: String,
    stage: String,
}

#[derive(Clone, Serialize)]
struct ProgressPayload {
    instance_id: String,
    #[serde(flatten)]
    progress: DownloadProgress,
}

pub fn emit_stage(app: &AppHandle, instance_id: &str, stage: &str) {
    let _ = app.emit(
        "install:stage",
        StagePayload {
            instance_id: instance_id.to_string(),
            stage: stage.to_string(),
        },
    );
}

pub async fn load_version_json(state: &AppState, version_id: &str) -> Result<VersionJson> {
    let path = state.paths.version_json(version_id);
    if let Ok(bytes) = tokio::fs::read(&path).await {
        if let Ok(parsed) = serde_json::from_slice(&bytes) {
            return Ok(parsed);
        }
    }

    let manifest = manifest::fetch(&state.http, &state.paths).await?;
    let entry = manifest
        .versions
        .iter()
        .find(|v| v.id == version_id)
        .ok_or_else(|| Error::NotFound(format!("version {version_id}")))?;

    let bytes = state
        .http
        .get(&entry.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, &bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub async fn load_merged_version(state: &AppState, version_id: &str) -> Result<VersionJson> {
    let mut current = load_version_json(state, version_id).await?;
    let mut depth = 0;
    while let Some(parent_id) = current.inherits_from.clone() {
        if depth > 5 {
            return Err(Error::other(format!(
                "version inheritance too deep for {version_id}"
            )));
        }
        let parent = load_version_json(state, &parent_id).await?;
        current = merge_versions(parent, current);
        depth += 1;
    }
    Ok(current)
}

async fn load_asset_index(state: &AppState, version: &VersionJson) -> Result<AssetIndex> {
    let asset_index = version
        .asset_index
        .as_ref()
        .ok_or_else(|| Error::other(format!("version {} has no asset index", version.id)))?;
    let path = state
        .paths
        .assets_indexes()
        .join(format!("{}.json", asset_index.id));
    if let Ok(bytes) = tokio::fs::read(&path).await {
        if let Ok(parsed) = serde_json::from_slice(&bytes) {
            return Ok(parsed);
        }
    }

    let bytes = state
        .http
        .get(&asset_index.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, &bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn extract_natives(natives: &[NativeSpec], dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for native in natives {
        let file = std::fs::File::open(&native.spec.dest)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| Error::other(format!("opening native jar: {e}")))?;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| Error::other(format!("reading native entry: {e}")))?;
            let name = entry.name().to_string();
            if entry.is_dir() || name.starts_with("META-INF/") {
                continue;
            }
            if native.exclude.iter().any(|ex| name.starts_with(ex)) {
                continue;
            }
            let out = dest.join(Path::new(&name).file_name().unwrap_or_default());
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf)?;
            std::fs::write(out, buf)?;
        }
    }
    Ok(())
}

pub async fn install_version(
    app: &AppHandle,
    state: &AppState,
    instance_id: &str,
    version_id: &str,
) -> Result<()> {
    emit_stage(app, instance_id, "metadata");
    let version = load_merged_version(state, version_id).await?;

    let resolved = version.resolve_libraries(&state.paths);

    emit_stage(app, instance_id, "assets-index");
    let asset_index = load_asset_index(state, &version).await?;

    let mut specs: Vec<DownloadSpec> = Vec::new();
    specs.extend(resolved.classpath.iter().cloned());
    specs.extend(resolved.natives.iter().map(|n| n.spec.clone()));
    specs.push(version.client_spec(&state.paths).ok_or_else(|| {
        Error::other(format!("version {} has no client download", version.id))
    })?);
    specs.extend(asset_index.specs(&state.paths));

    let concurrency = state.db.load_settings()?.concurrent_downloads;

    emit_stage(app, instance_id, "downloading");
    let emit_app = app.clone();
    let emit_id = instance_id.to_string();
    download::download_many(&state.http, specs, concurrency, move |progress| {
        let _ = emit_app.emit(
            "install:progress",
            ProgressPayload {
                instance_id: emit_id.clone(),
                progress,
            },
        );
    })
    .await?;

    emit_stage(app, instance_id, "natives");
    let natives = resolved.natives.clone();
    let natives_dir = state.paths.natives_dir(version_id);
    tokio::task::spawn_blocking(move || extract_natives(&natives, &natives_dir))
        .await
        .map_err(|e| Error::other(format!("native extraction task failed: {e}")))??;

    emit_stage(app, instance_id, "done");
    Ok(())
}
