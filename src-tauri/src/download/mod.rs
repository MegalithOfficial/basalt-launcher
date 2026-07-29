use std::path::PathBuf;
use std::time::Duration;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use futures::stream::{self, StreamExt};
use serde::Serialize;
use sha1_smol::Sha1;
use tokio::io::AsyncWriteExt;

use tokio_util::sync::CancellationToken;

use crate::error::{Error, Result};
use crate::files::FileManager;
use crate::network::NetworkManager;

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

async fn already_valid(files: &FileManager, spec: &DownloadSpec) -> bool {
    let bytes = match files.read_async(&spec.dest).await {
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

const DOWNLOAD_ATTEMPTS: u32 = 4;
const RETRY_BASE: Duration = Duration::from_millis(300);
const RETRY_CEILING: Duration = Duration::from_secs(8);

pub fn is_retryable(error: &Error) -> bool {
    match error {
        Error::Cancelled => false,
        Error::Checksum { .. } => true,
        Error::Http(e) => {
            if let Some(status) = e.status() {
                return status.is_server_error()
                    || status == reqwest::StatusCode::TOO_MANY_REQUESTS
                    || status == reqwest::StatusCode::REQUEST_TIMEOUT;
            }
            e.is_timeout() || e.is_connect() || e.is_body() || e.is_decode() || e.is_request()
        }
        _ => false,
    }
}

pub fn retry_delay(attempt: u32) -> Duration {
    Duration::from_millis(RETRY_BASE.as_millis() as u64 * 2u64.pow(attempt.min(6))).min(RETRY_CEILING)
}

pub type RetryHook<'a> = Option<&'a (dyn Fn(u32, u32, &str) + Send + Sync)>;

pub async fn download_one(
    client: &NetworkManager,
    files: &FileManager,
    spec: &DownloadSpec,
) -> Result<bool> {
    download_one_reporting(client, files, spec, None).await
}

pub async fn download_one_reporting(
    client: &NetworkManager,
    files: &FileManager,
    spec: &DownloadSpec,
    on_retry: RetryHook<'_>,
) -> Result<bool> {
    let mut attempt = 0;
    loop {
        match download_once(client, files, spec).await {
            Ok(created) => return Ok(created),
            Err(e) => {
                attempt += 1;
                if attempt >= DOWNLOAD_ATTEMPTS || !is_retryable(&e) {
                    return Err(match attempt {
                        1 => e,
                        n => Error::other(format!("{e} (after {n} attempts)")),
                    });
                }
                if let Some(hook) = on_retry {
                    hook(attempt, DOWNLOAD_ATTEMPTS, &short_reason(&e));
                }
                tokio::time::sleep(retry_delay(attempt)).await;
            }
        }
    }
}

fn short_reason(error: &Error) -> String {
    match error {
        Error::Checksum { .. } => "corrupt download".to_string(),
        Error::Http(e) if e.is_timeout() => "timed out".to_string(),
        Error::Http(e) if e.is_connect() => "connection failed".to_string(),
        Error::Http(e) if e.is_body() || e.is_decode() => "interrupted transfer".to_string(),
        Error::Http(e) => match e.status() {
            Some(status) => format!("server said {status}"),
            None => "network error".to_string(),
        },
        other => other.to_string(),
    }
}

async fn download_once(
    client: &NetworkManager,
    files: &FileManager,
    spec: &DownloadSpec,
) -> Result<bool> {
    if already_valid(files, spec).await {
        tracing::trace!(dest = %spec.dest.display(), "already on disk, skipped");
        return Ok(false);
    }
    if let Some(parent) = spec.dest.parent() {
        files.ensure_dir_async(parent).await?;
    }

    let resp = client
        .send_once(client.get(&spec.url))
        .await?
        .error_for_status()?;
    let mut stream = resp.bytes_stream();

    let tmp = files.temporary_for(&spec.dest)?;
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
            tracing::warn!(
                url = %spec.url,
                expected = %expected,
                actual = %actual,
                "checksum mismatch, discarding download"
            );
            let _ = files.remove_file_if_exists_async(&tmp).await;
            return Err(Error::Checksum {
                path: spec.dest.display().to_string(),
                expected: expected.clone(),
                actual,
            });
        }
    }

    files.commit_temporary(&tmp, &spec.dest).await?;
    tracing::trace!(url = %spec.url, dest = %spec.dest.display(), "downloaded");
    Ok(true)
}

pub async fn download_many<F>(
    client: &NetworkManager,
    files: &FileManager,
    specs: Vec<DownloadSpec>,
    concurrency: usize,
    on_progress: F,
) -> Result<()>
where
    F: Fn(DownloadProgress) + Send + Sync,
{
    download_many_cancellable(client, files, specs, concurrency, on_progress, None, None, None).await
}

pub async fn download_many_cancellable<F>(
    client: &NetworkManager,
    files: &FileManager,
    specs: Vec<DownloadSpec>,
    concurrency: usize,
    on_progress: F,
    cancel: Option<CancellationToken>,
    written: Option<&std::sync::Mutex<Vec<PathBuf>>>,
    on_retry: RetryHook<'_>,
) -> Result<()>
where
    F: Fn(DownloadProgress) + Send + Sync,
{
    let started = std::time::Instant::now();
    let total = specs.len();
    let total_bytes: u64 = specs.iter().filter_map(|s| s.size).sum();
    tracing::debug!(total, total_bytes, concurrency, "download batch started");
    let completed = AtomicUsize::new(0);
    let done_bytes = AtomicU64::new(0);
    let on_progress = &on_progress;
    let completed = &completed;
    let done_bytes = &done_bytes;

    let cancel = &cancel;
    let results = stream::iter(specs.into_iter().map(|spec| async move {
        if let Some(token) = cancel {
            if token.is_cancelled() {
                return Err(Error::Cancelled);
            }
        }
        let name = spec
            .dest
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let created = match cancel {
            Some(token) => tokio::select! {
                biased;
                _ = token.cancelled() => return Err(Error::Cancelled),
                result = download_one_reporting(client, files, &spec, on_retry) => result,
            },
            None => download_one_reporting(client, files, &spec, on_retry).await,
        }
        .inspect_err(|e| tracing::warn!(url = %spec.url, error = %e, "download failed"))?;
        if created {
            if let Some(sink) = written {
                sink.lock().unwrap().push(spec.dest.clone());
            }
        }
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

    let failures = results.iter().filter(|r| r.is_err()).count();
    for result in results {
        result?;
    }
    tracing::info!(
        total,
        total_bytes,
        failures,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "download batch finished"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::sha1_hex;

    fn network() -> NetworkManager {
        NetworkManager::with_client(reqwest::Client::new())
    }

    fn files(root: &std::path::Path) -> FileManager {
        FileManager::new(crate::paths::Paths {
            root: root.to_path_buf(),
        })
    }

    #[tokio::test]
    async fn cancelled_before_start_downloads_nothing() {
        let dir = std::env::temp_dir().join(format!("basalt-cancel-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let token = CancellationToken::new();
        token.cancel();

        let written = std::sync::Mutex::new(Vec::new());
        let specs = vec![DownloadSpec {
            url: "http://127.0.0.1:1/never".into(),
            dest: dir.join("never.jar"),
            sha1: None,
            size: None,
        }];

        let network = network();
        let files = files(&dir);
        let result = download_many_cancellable(
            &network,
            &files,
            specs,
            2,
            |_| {},
            Some(token),
            Some(&written),
            None,
        )
        .await;

        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(written.lock().unwrap().is_empty());
        assert!(!dir.join("never.jar").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancellation_is_never_retried() {
        assert!(!is_retryable(&Error::Cancelled));
    }

    #[test]
    fn corrupt_bodies_are_retried_but_disk_errors_are_not() {
        assert!(is_retryable(&Error::Checksum {
            path: "a.jar".into(),
            expected: "x".into(),
            actual: "y".into(),
        }));
        assert!(!is_retryable(&Error::Io(std::io::Error::other("disk full"))));
        assert!(!is_retryable(&Error::other("nope")));
    }

    #[test]
    fn retry_delay_grows_then_settles() {
        assert!(retry_delay(1) < retry_delay(2));
        assert!(retry_delay(2) < retry_delay(3));
        assert_eq!(retry_delay(30), RETRY_CEILING);
    }

    #[tokio::test]
    async fn a_dead_host_is_retried_before_giving_up() {
        let started = std::time::Instant::now();
        let spec = DownloadSpec {
            url: "http://127.0.0.1:1/gone".into(),
            dest: std::env::temp_dir().join("basalt-retry-probe.jar"),
            sha1: None,
            size: None,
        };
        let files = files(std::env::temp_dir().as_path());
        let result = download_one(&network(), &files, &spec).await;
        let message = result.unwrap_err().to_string();

        assert!(
            message.contains(&format!("after {DOWNLOAD_ATTEMPTS} attempts")),
            "error should report the retries, got: {message}"
        );
        assert!(
            started.elapsed() >= Duration::from_millis(4000),
            "should have backed off between attempts, took {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn cancelling_mid_flight_aborts_the_remaining_queue() {
        let dir = std::env::temp_dir().join(format!("basalt-midflight-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let token = CancellationToken::new();

        let specs: Vec<DownloadSpec> = (0..8)
            .map(|i| DownloadSpec {
                url: "http://10.255.255.1/hang".into(),
                dest: dir.join(format!("f{i}.jar")),
                sha1: None,
                size: None,
            })
            .collect();

        let cancel = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            cancel.cancel();
        });

        let started = std::time::Instant::now();
        let network = network();
        let files = files(&dir);
        let result = download_many_cancellable(
            &network,
            &files,
            specs,
            4,
            |_| {},
            Some(token),
            None,
            None,
        )
        .await;

        assert!(matches!(result, Err(Error::Cancelled)), "expected Cancelled, got {result:?}");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "cancel should abort in flight requests, took {:?}",
            started.elapsed()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn already_valid_files_are_not_recorded_as_written() {
        let dir = std::env::temp_dir().join(format!("basalt-existing-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let dest = dir.join("present.jar");
        std::fs::write(&dest, b"abc").unwrap();

        let spec = DownloadSpec {
            url: "http://127.0.0.1:1/unused".into(),
            dest: dest.clone(),
            sha1: Some(sha1_hex(b"abc")),
            size: None,
        };
        let network = network();
        let files = files(&dir);
        assert!(!download_one(&network, &files, &spec).await.unwrap());

        let written = std::sync::Mutex::new(Vec::new());
        let result = download_many_cancellable(
            &network,
            &files,
            vec![spec],
            2,
            |_| {},
            None,
            Some(&written),
            None,
        )
        .await;

        assert!(result.is_ok());
        assert!(written.lock().unwrap().is_empty(), "pre-existing file must not be rollback eligible");
        assert!(dest.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha1_matches_known_vector() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }
}
