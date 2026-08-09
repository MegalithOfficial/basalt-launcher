use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    sync::{Arc, Mutex, OnceLock, Weak},
    time::Duration,
};

use futures::stream::{self, StreamExt};
use serde::Serialize;
use sha1_smol::Sha1;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};
use tokio_util::sync::CancellationToken;

use crate::{
    error::{Error, Result},
    files::FileManager,
    network::NetworkManager,
};

#[derive(Debug, Clone)]
pub struct DownloadSpec {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
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

pub async fn copy_verified(
    files: &FileManager,
    source: impl AsRef<std::path::Path>,
    destination: impl AsRef<std::path::Path>,
    expected_sha1: Option<&str>,
    expected_size: Option<u64>,
) -> Result<u64> {
    let destination = destination.as_ref();
    let copied = files.copy_external_into(source, destination).await?;
    let result = async {
        if let Some(expected) = expected_size {
            if copied != expected {
                return Err(Error::SizeMismatch {
                    path: destination.display().to_string(),
                    expected,
                    actual: copied,
                });
            }
        }
        if let Some(expected) = expected_sha1 {
            let bytes = files.read_async(destination).await?;
            let actual = sha1_hex(&bytes);
            if actual != expected {
                return Err(Error::Checksum {
                    path: destination.display().to_string(),
                    expected: expected.to_string(),
                    actual,
                });
            }
        }
        Ok(copied)
    }
    .await;
    if result.is_err() {
        let _ = files.remove_file_if_exists(destination);
    }
    result
}

async fn already_valid(files: &FileManager, spec: &DownloadSpec) -> bool {
    files.is_file(&spec.dest).unwrap_or(false)
        && verify_partial(files, &spec.dest, spec).await.is_ok()
}

const RETRY_BASE: Duration = Duration::from_millis(300);
const RETRY_CEILING: Duration = Duration::from_secs(8);
pub(crate) const PART_SUFFIX: &str = ".basalt-part";

static DOWNLOAD_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<AsyncMutex<()>>>>> = OnceLock::new();

fn partial_path(destination: &Path) -> PathBuf {
    let mut name = destination
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(PART_SUFFIX);
    destination.with_file_name(name)
}

async fn destination_lock(destination: &Path) -> OwnedMutexGuard<()> {
    let lock = {
        let mut locks = DOWNLOAD_LOCKS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        locks.retain(|_, lock| lock.strong_count() > 0);
        match locks.get(destination).and_then(Weak::upgrade) {
            Some(lock) => lock,
            None => {
                let lock = Arc::new(AsyncMutex::new(()));
                locks.insert(destination.to_path_buf(), Arc::downgrade(&lock));
                lock
            }
        }
    };
    lock.lock_owned().await
}

fn retryable_status(status: reqwest::StatusCode) -> bool {
    status.is_server_error()
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status == reqwest::StatusCode::TOO_EARLY
}

pub fn is_retryable(error: &Error) -> bool {
    match error {
        Error::Cancelled => false,
        Error::Checksum { .. } | Error::SizeMismatch { .. } => true,
        Error::HttpStatus(status) | Error::HttpResponse { status, .. } => retryable_status(*status),
        Error::Http(e) => {
            if let Some(status) = e.status() {
                return retryable_status(status);
            }
            e.is_timeout() || e.is_connect() || e.is_body() || e.is_decode() || e.is_request()
        }
        Error::Io(error) => matches!(
            error.kind(),
            std::io::ErrorKind::Interrupted
                | std::io::ErrorKind::TimedOut
                | std::io::ErrorKind::WouldBlock
        ),
        _ => false,
    }
}

pub fn retry_delay(attempt: u32) -> Duration {
    Duration::from_millis(RETRY_BASE.as_millis() as u64 * 2u64.pow(attempt.min(6)))
        .min(RETRY_CEILING)
}

pub type RetryHook<'a> = Option<&'a (dyn Fn(u32, u32, &str) + Send + Sync)>;
type ByteHook<'a> = Option<&'a (dyn Fn(u64) + Send + Sync)>;

pub async fn download_one(
    client: &NetworkManager,
    files: &FileManager,
    spec: &DownloadSpec,
) -> Result<bool> {
    download_one_reporting(client, files, spec, None, None).await
}

pub async fn download_one_reporting(
    client: &NetworkManager,
    files: &FileManager,
    spec: &DownloadSpec,
    on_retry: RetryHook<'_>,
    on_bytes: ByteHook<'_>,
) -> Result<bool> {
    let _guard = destination_lock(&spec.dest).await;
    let attempts = client.attempts();
    let mut attempt = 0;
    loop {
        match download_once(client, files, spec, on_bytes).await {
            Ok(created) => return Ok(created),
            Err(e) => {
                attempt += 1;
                if matches!(e, Error::Checksum { .. })
                    || matches!(e, Error::SizeMismatch { expected, actual, .. } if actual > expected)
                {
                    let _ = files.remove_file_if_exists(partial_path(&spec.dest));
                    if let Some(hook) = on_bytes {
                        hook(0);
                    }
                }
                if attempt >= attempts || !is_retryable(&e) {
                    return Err(match attempt {
                        1 => e,
                        n => Error::other(format!("{e} (after {n} attempts)")),
                    });
                }
                if let Some(hook) = on_retry {
                    let saved = partial_len(files, &partial_path(&spec.dest));
                    let reason = if saved > 0 {
                        format!(
                            "{}; resuming from {}",
                            short_reason(&e),
                            readable_bytes(saved)
                        )
                    } else {
                        short_reason(&e)
                    };
                    hook(attempt, attempts, &reason);
                }
                tokio::time::sleep(retry_delay(attempt)).await;
            }
        }
    }
}

fn short_reason(error: &Error) -> String {
    match error {
        Error::Checksum { .. } => "corrupt download".to_string(),
        Error::SizeMismatch { .. } => "incomplete download".to_string(),
        Error::Http(e) if e.is_timeout() => "timed out".to_string(),
        Error::Http(e) if e.is_connect() => "connection failed".to_string(),
        Error::Http(e) if e.is_body() || e.is_decode() => "interrupted transfer".to_string(),
        Error::Http(e) => match e.status() {
            Some(status) => format!("server said {status}"),
            None => "network error".to_string(),
        },
        Error::HttpStatus(status) | Error::HttpResponse { status, .. } => {
            format!("server said {status}")
        }
        other => other.to_string(),
    }
}

fn readable_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.0} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn partial_len(files: &FileManager, path: &Path) -> u64 {
    files
        .metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContentRange {
    start: u64,
    end: u64,
    total: Option<u64>,
}

fn parse_content_range(value: &str) -> Option<ContentRange> {
    let value = value.trim().strip_prefix("bytes ")?;
    let (range, total) = value.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start = start.parse().ok()?;
    let end = end.parse().ok()?;
    if end < start {
        return None;
    }
    let total = (total != "*").then(|| total.parse().ok()).flatten();
    Some(ContentRange { start, end, total })
}

fn response_range(response: &crate::network::ManagedResponse) -> Option<ContentRange> {
    response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)?
        .to_str()
        .ok()
        .and_then(parse_content_range)
}

fn forgecdn_fallback_url(url: &str) -> Option<String> {
    let mut parsed = reqwest::Url::parse(url).ok()?;
    if parsed.host_str() != Some("edge.forgecdn.net") {
        return None;
    }
    parsed.set_host(Some("mediafilez.forgecdn.net")).ok()?;
    Some(parsed.into())
}

async fn send_download_request(
    client: &NetworkManager,
    url: &str,
    offset: u64,
) -> Result<crate::network::ManagedResponse> {
    let fallback = forgecdn_fallback_url(url);
    let mut current = url;
    loop {
        let mut request = client
            .get(current)
            .header(reqwest::header::ACCEPT_ENCODING, "identity");
        if offset > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
        }
        let response = client.send_download_once(request).await?;
        if current == url && response.status() == reqwest::StatusCode::NOT_FOUND {
            if let Some(fallback) = fallback.as_deref() {
                tracing::warn!(url, fallback, "CurseForge edge download was unavailable");
                current = fallback;
                continue;
            }
        }
        return Ok(response);
    }
}

async fn verify_partial(files: &FileManager, path: &Path, spec: &DownloadSpec) -> Result<u64> {
    let files = files.clone();
    let path = path.to_path_buf();
    let expected_size = spec.size;
    let expected_sha1 = spec.sha1.clone();
    let expected_sha256 = spec.sha256.clone();
    tokio::task::spawn_blocking(move || {
        let mut file = files.open(&path)?;
        let mut sha1 = expected_sha1.as_ref().map(|_| Sha1::new());
        let mut sha256 = expected_sha256.as_ref().map(|_| Sha256::new());
        let mut actual_size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            actual_size += read as u64;
            if let Some(hasher) = &mut sha1 {
                hasher.update(&buffer[..read]);
            }
            if let Some(hasher) = &mut sha256 {
                hasher.update(&buffer[..read]);
            }
        }
        if let Some(expected) = expected_size {
            if actual_size != expected {
                return Err(Error::SizeMismatch {
                    path: path.display().to_string(),
                    expected,
                    actual: actual_size,
                });
            }
        }
        if let Some(expected) = expected_sha1 {
            let actual = sha1
                .expect("SHA-1 hasher exists when expected")
                .digest()
                .to_string();
            if actual != expected {
                return Err(Error::Checksum {
                    path: path.display().to_string(),
                    expected,
                    actual,
                });
            }
        }
        if let Some(expected) = expected_sha256 {
            let actual = format!(
                "{:x}",
                sha256
                    .expect("SHA-256 hasher exists when expected")
                    .finalize()
            );
            if !actual.eq_ignore_ascii_case(&expected) {
                return Err(Error::Checksum {
                    path: path.display().to_string(),
                    expected,
                    actual,
                });
            }
        }
        Ok(actual_size)
    })
    .await
    .map_err(|error| Error::other(format!("download verification task failed: {error}")))?
}

async fn download_once(
    client: &NetworkManager,
    files: &FileManager,
    spec: &DownloadSpec,
    on_bytes: ByteHook<'_>,
) -> Result<bool> {
    if already_valid(files, spec).await {
        if let Some(hook) = on_bytes {
            hook(spec.size.unwrap_or_else(|| {
                files
                    .metadata(&spec.dest)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0)
            }));
        }
        tracing::trace!(dest = %spec.dest.display(), "already on disk, skipped");
        return Ok(false);
    }

    let partial = partial_path(&spec.dest);
    let mut offset = partial_len(files, &partial);
    if spec.size.is_some_and(|expected| offset > expected) {
        files.remove_file_if_exists(&partial)?;
        offset = 0;
    }
    if spec.size == Some(offset) && offset > 0 {
        match verify_partial(files, &partial, spec).await {
            Ok(_) => {
                files.remove_file_if_exists(&spec.dest)?;
                files.commit_download_part(&partial, &spec.dest).await?;
                if let Some(hook) = on_bytes {
                    hook(offset);
                }
                return Ok(true);
            }
            Err(Error::Checksum { .. }) => {
                files.remove_file_if_exists(&partial)?;
                offset = 0;
            }
            Err(error) => return Err(error),
        }
    }

    if let Some(hook) = on_bytes {
        hook(offset);
    }
    let response = loop {
        let response = send_download_request(client, &spec.url, offset).await?;
        if offset == 0 {
            let response = response.error_for_status()?;
            if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                let valid = response_range(&response).is_some_and(|range| {
                    range.start == 0
                        && spec
                            .size
                            .is_none_or(|expected| range.total == Some(expected))
                });
                if !valid {
                    return Err(Error::other(
                        "server returned an invalid partial download response",
                    ));
                }
            }
            break response;
        }
        if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            let valid = response_range(&response).is_some_and(|range| {
                range.start == offset
                    && spec
                        .size
                        .is_none_or(|expected| range.total == Some(expected))
            });
            if valid {
                break response;
            }
        } else if response.status().is_success() {
            files.remove_file_if_exists(&partial)?;
            offset = 0;
            if let Some(hook) = on_bytes {
                hook(0);
            }
            break response;
        } else if response.status() != reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            break response.error_for_status()?;
        }
        files.remove_file_if_exists(&partial)?;
        offset = 0;
        if let Some(hook) = on_bytes {
            hook(0);
        }
    };

    let expected_response = spec.size.map(|expected| expected.saturating_sub(offset));
    if let (Some(expected), Some(actual)) = (expected_response, response.content_length()) {
        if actual != expected {
            return Err(Error::SizeMismatch {
                path: spec.dest.display().to_string(),
                expected: spec.size.unwrap_or(expected),
                actual: offset + actual,
            });
        }
    }
    let mut stream = response.bytes_stream();
    let mut writer = files.open_download_part(&partial, offset > 0).await?;
    let mut downloaded = offset;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        downloaded += chunk.len() as u64;
        if let Some(expected) = spec.size {
            if downloaded > expected {
                return Err(Error::SizeMismatch {
                    path: spec.dest.display().to_string(),
                    expected,
                    actual: downloaded,
                });
            }
        }
        writer.write_all(&chunk).await?;
        if let Some(hook) = on_bytes {
            hook(downloaded);
        }
    }
    writer.flush().await?;
    writer.sync_all().await?;
    drop(writer);

    verify_partial(files, &partial, spec).await?;
    files.remove_file_if_exists(&spec.dest)?;
    files.commit_download_part(&partial, &spec.dest).await?;
    tracing::trace!(url = %spec.url, dest = %spec.dest.display(), "downloaded");
    Ok(true)
}

fn deduplicate_specs(specs: Vec<DownloadSpec>) -> Result<Vec<DownloadSpec>> {
    let mut unique: HashMap<PathBuf, DownloadSpec> = HashMap::with_capacity(specs.len());
    for spec in specs {
        if let Some(existing) = unique.get(&spec.dest) {
            if existing.url != spec.url
                || existing.sha1 != spec.sha1
                || existing.sha256 != spec.sha256
                || existing.size != spec.size
            {
                return Err(Error::other(format!(
                    "conflicting downloads target {}",
                    spec.dest.display()
                )));
            }
            continue;
        }
        unique.insert(spec.dest.clone(), spec);
    }
    let mut specs: Vec<_> = unique.into_values().collect();
    specs.sort_by(|left, right| left.dest.cmp(&right.dest));
    Ok(specs)
}

#[allow(clippy::too_many_arguments)]
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
    let specs = deduplicate_specs(specs)?;
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
        let reported = AtomicU64::new(0);
        let report_bytes = |bytes: u64| {
            if spec.size.is_none() {
                return;
            }
            let previous = reported.swap(bytes, Ordering::Relaxed);
            let aggregate = if bytes >= previous {
                done_bytes.fetch_add(bytes - previous, Ordering::Relaxed) + bytes - previous
            } else {
                done_bytes.fetch_sub(previous - bytes, Ordering::Relaxed) - (previous - bytes)
            };
            on_progress(DownloadProgress {
                completed: completed.load(Ordering::Relaxed),
                total,
                downloaded_bytes: aggregate,
                total_bytes,
                current: name.clone(),
            });
        };
        let created = match cancel {
            Some(token) => tokio::select! {
                biased;
                _ = token.cancelled() => return Err(Error::Cancelled),
                result = download_one_reporting(client, files, &spec, on_retry, Some(&report_bytes)) => result,
            },
            None => download_one_reporting(client, files, &spec, on_retry, Some(&report_bytes)).await,
        }
        .inspect_err(|e| tracing::warn!(url = %spec.url, error = %e, "download failed"))?;
        if created {
            if let Some(sink) = written {
                sink.lock().unwrap().push(spec.dest.clone());
            }
        }
        let c = completed.fetch_add(1, Ordering::Relaxed) + 1;
        let expected = spec.size.unwrap_or(0);
        let previous = reported.swap(expected, Ordering::Relaxed);
        let b = if expected >= previous {
            done_bytes.fetch_add(expected - previous, Ordering::Relaxed) + expected - previous
        } else {
            done_bytes.fetch_sub(previous - expected, Ordering::Relaxed) - (previous - expected)
        };
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
    use super::{sha1_hex, *};

    fn network() -> NetworkManager {
        NetworkManager::new()
    }

    fn files(root: &std::path::Path) -> FileManager {
        FileManager::new(crate::paths::Paths::plain(root.to_path_buf())).unwrap()
    }

    fn read_request(socket: &mut std::net::TcpStream) -> String {
        let mut request = [0_u8; 2048];
        let read = std::io::Read::read(socket, &mut request).unwrap();
        String::from_utf8_lossy(&request[..read]).into_owned()
    }

    #[tokio::test]
    async fn verified_copies_reject_corrupt_manual_files() {
        let dir = std::env::temp_dir().join(format!("basalt-manual-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source.jar");
        let destination = dir.join("installed.jar");
        std::fs::write(&source, b"correct").unwrap();
        let files = files(&dir);

        copy_verified(
            &files,
            &source,
            &destination,
            Some(&sha1_hex(b"correct")),
            Some(7),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"correct");

        std::fs::write(&source, b"corrupt").unwrap();
        let error = copy_verified(
            &files,
            &source,
            &destination,
            Some(&sha1_hex(b"correct")),
            Some(7),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, Error::Checksum { .. }));
        assert!(!destination.exists());
        let _ = std::fs::remove_dir_all(&dir);
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
            sha256: None,
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

        assert!(
            matches!(result, Err(Error::Cancelled)),
            "expected pre-cancelled batch to stop, got {result:?}"
        );
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
        assert!(!is_retryable(&Error::Io(std::io::Error::other(
            "disk full"
        ))));
        assert!(!is_retryable(&Error::other("nope")));
    }

    #[test]
    fn incomplete_bodies_are_retried() {
        assert!(is_retryable(&Error::SizeMismatch {
            path: "a.jar".into(),
            expected: 10,
            actual: 5,
        }));
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
            sha256: None,
            size: None,
        };
        let files = files(std::env::temp_dir().as_path());
        let network = network();
        network
            .reconfigure(&crate::config::LauncherSettings {
                max_retries: 2,
                ..Default::default()
            })
            .unwrap();
        let attempts = network.attempts();
        let result = download_one(&network, &files, &spec).await;
        let message = result.unwrap_err().to_string();

        assert!(
            message.contains(&format!("after {attempts} attempts")),
            "error should report the retries, got: {message}"
        );
        assert!(
            started.elapsed() >= Duration::from_millis(500),
            "should have backed off between attempts, took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn downloads_default_to_ten_attempts() {
        assert_eq!(network().attempts(), 10);
        assert_eq!(crate::config::default_max_retries(), 10);
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
                sha256: None,
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
        let result =
            download_many_cancellable(&network, &files, specs, 4, |_| {}, Some(token), None, None)
                .await;

        assert!(
            matches!(result, Err(Error::Cancelled)),
            "expected Cancelled, got {result:?}"
        );
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "cancel should abort in flight requests, took {:?}",
            started.elapsed()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn cancelling_an_active_download_preserves_its_partial_file() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            read_request(&mut socket);
            std::io::Write::write_all(
                &mut socket,
                b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
            std::io::Write::write_all(&mut socket, &vec![b'x'; 128 * 1024]).unwrap();
            std::io::Write::flush(&mut socket).unwrap();
            std::thread::sleep(Duration::from_secs(2));
        });

        let dir =
            std::env::temp_dir().join(format!("basalt-active-cancel-{}", uuid::Uuid::new_v4()));
        let token = CancellationToken::new();
        let cancel_on_progress = token.clone();
        let destination = dir.join("file.jar");

        let result = download_many_cancellable(
            &network(),
            &files(&dir),
            vec![DownloadSpec {
                url: format!("http://{address}/file"),
                dest: destination.clone(),
                sha1: None,
                sha256: None,
                size: Some(1_000_000),
            }],
            1,
            move |progress| {
                if progress.downloaded_bytes > 0 {
                    cancel_on_progress.cancel();
                }
            },
            Some(token),
            None,
            None,
        )
        .await;

        assert!(
            matches!(result, Err(Error::Cancelled)),
            "expected cancellation after receiving the first chunk, got {result:?}"
        );
        let saved = std::fs::read(partial_path(&destination)).unwrap();
        assert!(!saved.is_empty());
        assert!(saved.iter().all(|byte| *byte == b'x'));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn interrupted_transfer_resumes_from_the_saved_byte() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut first, _) = listener.accept().unwrap();
            let first_request = read_request(&mut first);
            assert!(!first_request.to_ascii_lowercase().contains("range:"));
            std::io::Write::write_all(
                &mut first,
                b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nabc",
            )
            .unwrap();
            drop(first);

            let (mut second, _) = listener.accept().unwrap();
            let second_request = read_request(&mut second);
            assert!(second_request
                .to_ascii_lowercase()
                .contains("range: bytes=3-"));
            std::io::Write::write_all(
                &mut second,
                b"HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\nContent-Range: bytes 3-7/8\r\nConnection: close\r\n\r\ndefgh",
            )
            .unwrap();
        });

        let dir = std::env::temp_dir().join(format!("basalt-resume-{}", uuid::Uuid::new_v4()));
        let destination = dir.join("file.jar");
        let spec = DownloadSpec {
            url: format!("http://{address}/file"),
            dest: destination.clone(),
            sha1: Some(sha1_hex(b"abcdefgh")),
            sha256: None,
            size: Some(8),
        };

        assert!(download_one(&network(), &files(&dir), &spec).await.unwrap());
        server.join().unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"abcdefgh");
        assert!(!partial_path(&destination).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn persisted_partial_is_resumed_by_a_later_download() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            assert!(read_request(&mut socket)
                .to_ascii_lowercase()
                .contains("range: bytes=3-"));
            std::io::Write::write_all(
                &mut socket,
                b"HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\nContent-Range: bytes 3-7/8\r\nConnection: close\r\n\r\ndefgh",
            )
            .unwrap();
        });

        let dir =
            std::env::temp_dir().join(format!("basalt-persisted-resume-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let destination = dir.join("file.jar");
        std::fs::write(partial_path(&destination), b"abc").unwrap();
        let spec = DownloadSpec {
            url: format!("http://{address}/file"),
            dest: destination.clone(),
            sha1: Some(sha1_hex(b"abcdefgh")),
            sha256: None,
            size: Some(8),
        };

        assert!(download_one(&network(), &files(&dir), &spec).await.unwrap());
        server.join().unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"abcdefgh");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_server_ignoring_range_restarts_without_duplicating_bytes() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            assert!(read_request(&mut socket)
                .to_ascii_lowercase()
                .contains("range: bytes=3-"));
            std::io::Write::write_all(
                &mut socket,
                b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nabcdefgh",
            )
            .unwrap();
        });

        let dir = std::env::temp_dir().join(format!("basalt-no-range-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let destination = dir.join("file.jar");
        std::fs::write(partial_path(&destination), b"abc").unwrap();
        let spec = DownloadSpec {
            url: format!("http://{address}/file"),
            dest: destination.clone(),
            sha1: Some(sha1_hex(b"abcdefgh")),
            sha256: None,
            size: Some(8),
        };

        assert!(download_one(&network(), &files(&dir), &spec).await.unwrap());
        server.join().unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"abcdefgh");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_destinations_are_collapsed_or_rejected() {
        let destination = PathBuf::from("same.jar");
        let spec = DownloadSpec {
            url: "https://example.invalid/file".into(),
            dest: destination.clone(),
            sha1: Some("hash".into()),
            sha256: None,
            size: Some(3),
        };
        assert_eq!(
            deduplicate_specs(vec![spec.clone(), spec.clone()])
                .unwrap()
                .len(),
            1
        );

        let mut conflict = spec.clone();
        conflict.url = "https://example.invalid/other".into();
        assert!(deduplicate_specs(vec![spec, conflict]).is_err());
    }

    #[test]
    fn content_ranges_are_strictly_parsed() {
        assert_eq!(
            parse_content_range("bytes 3-7/8"),
            Some(ContentRange {
                start: 3,
                end: 7,
                total: Some(8),
            })
        );
        assert_eq!(
            parse_content_range("bytes 3-7/*"),
            Some(ContentRange {
                start: 3,
                end: 7,
                total: None,
            })
        );
        assert_eq!(parse_content_range("bytes 7-3/8"), None);
        assert_eq!(parse_content_range("items 3-7/8"), None);
    }

    #[test]
    fn curseforge_edge_urls_get_a_media_fallback() {
        assert_eq!(
            forgecdn_fallback_url(
                "https://edge.forgecdn.net/files/7703/848/Apotheosis-1.21.1-8.5.2.jar"
            )
            .as_deref(),
            Some("https://mediafilez.forgecdn.net/files/7703/848/Apotheosis-1.21.1-8.5.2.jar")
        );
        assert_eq!(
            forgecdn_fallback_url("https://example.com/files/7703/848/file.jar"),
            None
        );
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
            sha256: None,
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
        assert!(
            written.lock().unwrap().is_empty(),
            "pre-existing file must not be rollback eligible"
        );
        assert!(dest.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sha1_matches_known_vector() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[tokio::test]
    async fn verifies_sha256_downloads() {
        let dir = std::env::temp_dir().join(format!("basalt-sha256-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let destination = dir.join("runtime.zip");
        std::fs::write(&destination, b"abc").unwrap();
        let mut spec = DownloadSpec {
            url: "https://example.invalid/runtime.zip".into(),
            dest: destination.clone(),
            sha1: None,
            sha256: Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into()),
            size: Some(3),
        };
        let files = files(&dir);

        assert_eq!(
            verify_partial(&files, &destination, &spec).await.unwrap(),
            3
        );
        spec.sha256 = Some("wrong".into());
        assert!(matches!(
            verify_partial(&files, &destination, &spec).await,
            Err(Error::Checksum { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
