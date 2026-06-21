use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use futures::stream::{self, StreamExt};
use serde::Serialize;
use sha1_smol::Sha1;
use tokio::io::AsyncWriteExt;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct DownloadSpec {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub completed: usize,
    pub total: usize,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub current: String,
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hasher.digest().to_string()
}

async fn already_valid(spec: &DownloadSpec) -> bool {
    let bytes = match tokio::fs::read(&spec.dest).await {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    if let Some(expected) = &spec.sha1 {
        return &sha1_hex(&bytes) == expected;
    }
    if let Some(size) = spec.size {
        return bytes.len() as u64 == size;
    }
    true
}

pub async fn download_one(client: &reqwest::Client, spec: &DownloadSpec) -> Result<bool> {
    if already_valid(spec).await {
        return Ok(false);
    }
    if let Some(parent) = spec.dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let resp = client.get(&spec.url).send().await?.error_for_status()?;
    let mut stream = resp.bytes_stream();

    let tmp = spec.dest.with_extension("part");
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut hasher = Sha1::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await?;

    if let Some(expected) = &spec.sha1 {
        let actual = hasher.digest().to_string();
        if &actual != expected {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(Error::Checksum {
                path: spec.dest.display().to_string(),
                expected: expected.clone(),
                actual,
            });
        }
    }

    tokio::fs::rename(&tmp, &spec.dest).await?;
    Ok(true)
}

pub async fn download_many<F>(
    client: &reqwest::Client,
    specs: Vec<DownloadSpec>,
    concurrency: usize,
    on_progress: F,
) -> Result<()>
where
    F: Fn(DownloadProgress) + Send + Sync,
{
    let total = specs.len();
    let total_bytes: u64 = specs.iter().filter_map(|s| s.size).sum();
    let completed = AtomicUsize::new(0);
    let done_bytes = AtomicU64::new(0);
    let on_progress = &on_progress;
    let completed = &completed;
    let done_bytes = &done_bytes;

    let results = stream::iter(specs.into_iter().map(|spec| async move {
        let name = spec
            .dest
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        download_one(client, &spec).await?;
        let c = completed.fetch_add(1, Ordering::Relaxed) + 1;
        let b = done_bytes.fetch_add(spec.size.unwrap_or(0), Ordering::Relaxed)
            + spec.size.unwrap_or(0);
        on_progress(DownloadProgress {
            completed: c,
            total,
            downloaded_bytes: b,
            total_bytes,
            current: name,
        });
        Ok::<(), Error>(())
    }))
    .buffer_unordered(concurrency.max(1))
    .collect::<Vec<_>>()
    .await;

    for result in results {
        result?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::sha1_hex;

    #[test]
    fn sha1_matches_known_vector() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }
}
