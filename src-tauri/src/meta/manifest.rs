use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::paths::Paths;

const MANIFEST_URL: &str =
    "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";

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

pub async fn fetch(client: &reqwest::Client, paths: &Paths) -> Result<VersionManifest> {
    let cache = paths.manifest_cache();
    let bytes = match client.get(MANIFEST_URL).send().await {
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
