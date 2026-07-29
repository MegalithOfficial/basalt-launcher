use std::io::Read;
use std::path::Path;

use serde::Serialize;
use tauri::AppHandle;

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

#[tracing::instrument(skip(state), err)]
pub async fn load_version_json(state: &AppState, version_id: &str) -> Result<VersionJson> {
    let path = state.paths.version_json(version_id);
    if let Ok(bytes) = state.files.read_async(&path).await {
        if let Ok(parsed) = serde_json::from_slice(&bytes) {
            tracing::debug!("version json read from cache");
            return Ok(parsed);
        }
    }
    tracing::debug!("version json not cached, fetching from mojang");

    let manifest = manifest::fetch(&state.network, &state.files).await?;
    let entry = manifest
        .versions
        .iter()
        .find(|v| v.id == version_id)
        .ok_or_else(|| Error::NotFound(format!("version {version_id}")))?;

    let bytes = state
        .network
        .send(state.network.get(&entry.url))
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    state.files.write_atomic_async(&path, &bytes).await?;
    tracing::debug!(bytes = bytes.len(), "version json cached");
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
    if depth > 0 {
        tracing::debug!(version_id, depth, "merged inherited version json");
    }
    Ok(current)
}

#[tracing::instrument(skip_all, fields(version_id = %version.id), err)]
async fn load_asset_index(state: &AppState, version: &VersionJson) -> Result<AssetIndex> {
    let asset_index = version
        .asset_index
        .as_ref()
        .ok_or_else(|| Error::other(format!("version {} has no asset index", version.id)))?;
    let path = state
        .paths
        .assets_indexes()
        .join(format!("{}.json", asset_index.id));
    if let Ok(bytes) = state.files.read_async(&path).await {
        if let Ok(parsed) = serde_json::from_slice(&bytes) {
            return Ok(parsed);
        }
    }

    let bytes = state
        .network
        .send(state.network.get(&asset_index.url))
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    state.files.write_atomic_async(&path, &bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[tracing::instrument(skip_all, fields(count = natives.len(), dest = %dest.display()), err)]
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

#[tracing::instrument(skip_all, fields(version_id = %version_id), err)]
pub async fn install_version(
    _app: &AppHandle,
    state: &AppState,
    _instance_id: &str,
    version_id: &str,
    task: &crate::tasks::TaskHandle,
) -> Result<()> {
    let started = std::time::Instant::now();
    task.stage("metadata");
    let version = load_merged_version(state, version_id).await?;

    let resolved = version.resolve_libraries(&state.paths);

    task.stage("assets-index");
    let asset_index = load_asset_index(state, &version).await?;

    let mut specs: Vec<DownloadSpec> = Vec::new();
    specs.extend(resolved.classpath.iter().cloned());
    specs.extend(resolved.natives.iter().map(|n| n.spec.clone()));
    specs.push(version.client_spec(&state.paths).ok_or_else(|| {
        Error::other(format!("version {} has no client download", version.id))
    })?);
    specs.extend(asset_index.specs(&state.paths));

    let concurrency = state.db.load_settings()?.concurrent_downloads;
    tracing::info!(
        libraries = resolved.classpath.len(),
        natives = resolved.natives.len(),
        assets = asset_index.objects.len(),
        files = specs.len(),
        concurrency,
        "install plan resolved"
    );

    task.stage("downloading");
    task.set_total(specs.len() as u64, specs.iter().filter_map(|s| s.size).sum());
    download::download_many_cancellable(
        &state.network,
        &state.files,
        specs,
        concurrency,
        |progress| {
            task.progress(
                progress.completed as u64,
                progress.total as u64,
                progress.downloaded_bytes,
                progress.total_bytes,
            );
        },
        Some(task.token()),
        None,
        Some(&|attempt, max, reason| task.note_retry(attempt, max, reason)),
    )
    .await?;

    task.stage("natives");
    let natives = resolved.natives.clone();
    let natives_dir = state.paths.natives_dir(version_id);
    tokio::task::spawn_blocking(move || extract_natives(&natives, &natives_dir))
        .await
        .map_err(|e| Error::other(format!("native extraction task failed: {e}")))??;

    tracing::info!(elapsed_ms = started.elapsed().as_millis() as u64, "install finished");
    Ok(())
}
