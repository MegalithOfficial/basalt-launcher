use serde::{Deserialize, Serialize};

use crate::{error::Result, files::FileManager, network::NetworkManager};

const MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub time: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    pub sha1: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionEntry>,
}

pub async fn fetch(client: &NetworkManager, files: &FileManager) -> Result<VersionManifest> {
    let cache = files.paths().manifest_cache();
    let bytes = match client.send(client.get(MANIFEST_URL)).await {
        Ok(resp) => {
            let resp = resp.error_for_status()?;
            let bytes = resp.bytes().await?;
            let _ = files.write_atomic_async(&cache, &bytes).await;
            bytes.to_vec()
        }
        Err(e) => {
            tracing::warn!(error = %e, cache = %cache.display(), "manifest fetch failed, using cache");
            files.read_async(&cache).await?
        }
    };
    Ok(serde_json::from_slice(&bytes)?)
}
