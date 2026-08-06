use std::{sync::Mutex, time::Duration};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{
    error::{Error, Result},
    network::NetworkManager,
};

pub const REPO: &str = "MegalithOfficial/basalt-launcher";
pub const REPO_URL: &str = "https://github.com/MegalithOfficial/basalt-launcher";

const UPDATE_PREFERENCES_KEY: &str = "app_update_preferences";
const INITIAL_CHECK_DELAY: Duration = Duration::from_secs(45);
const MIN_CHECK_INTERVAL: i64 = 4 * 60 * 60;
const DISABLED_CHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePolicy {
    SelfManaged,
    PackageManaged,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallSource {
    pub id: String,
    pub label: String,
    pub policy: UpdatePolicy,
    pub update_hint: String,
}

fn source(id: &str, label: &str, policy: UpdatePolicy, update_hint: &str) -> InstallSource {
    InstallSource {
        id: id.to_string(),
        label: label.to_string(),
        policy,
        update_hint: update_hint.to_string(),
    }
}

#[cfg(target_os = "linux")]
fn linux_install_source(
    distribution: &str,
    appimage: bool,
    flatpak: bool,
    snap: bool,
    nix_store: bool,
    aur_marker: bool,
) -> InstallSource {
    if appimage {
        return source(
            "appimage",
            "AppImage",
            UpdatePolicy::SelfManaged,
            "Basalt downloads and installs signed updates.",
        );
    }
    if flatpak {
        return source(
            "flatpak",
            "Flatpak",
            UpdatePolicy::PackageManaged,
            "Updates are installed by Flatpak.",
        );
    }
    if snap {
        return source(
            "snap",
            "Snap",
            UpdatePolicy::PackageManaged,
            "Updates are installed by Snap.",
        );
    }
    if aur_marker {
        return source(
            "aur",
            "AUR package",
            UpdatePolicy::PackageManaged,
            "Update using your AUR helper.",
        );
    }
    if distribution == "nix" || nix_store {
        return source(
            "nix",
            "Nix package",
            UpdatePolicy::PackageManaged,
            "Update through your Nix profile, flake, or NixOS configuration.",
        );
    }
    if distribution == "apt" {
        return source(
            "apt",
            "APT repository",
            UpdatePolicy::PackageManaged,
            "Updates are installed by your system package manager.",
        );
    }
    if distribution == "linux_bundle" || distribution == "deb" {
        return source(
            "deb",
            "Debian package",
            UpdatePolicy::Manual,
            "Download the latest .deb. Repository-managed packages update through their package manager.",
        );
    }
    source(
        "source",
        "Source build",
        UpdatePolicy::Manual,
        "Download or build the new version manually.",
    )
}

pub fn install_source() -> InstallSource {
    #[cfg(target_os = "linux")]
    {
        let executable = std::env::current_exe().ok();
        let nix_store = executable
            .as_ref()
            .is_some_and(|path| path.starts_with("/nix/store"));
        linux_install_source(
            crate::build_info::DISTRIBUTION,
            std::env::var_os("APPIMAGE").is_some(),
            std::env::var_os("FLATPAK_ID").is_some(),
            std::env::var_os("SNAP").is_some(),
            nix_store,
            std::path::Path::new("/usr/share/basalt-launcher/aur-package").is_file(),
        )
    }

    #[cfg(target_os = "windows")]
    {
        source(
            "windows",
            "Windows installer",
            UpdatePolicy::SelfManaged,
            "Basalt downloads and installs signed updates.",
        )
    }

    #[cfg(target_os = "macos")]
    {
        source(
            "macos",
            "macOS application",
            UpdatePolicy::SelfManaged,
            "Basalt downloads and installs signed updates.",
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub notes_url: Option<String>,
    pub published_at: Option<String>,
    pub update_available: bool,
    pub install_source: InstallSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppUpdatePhase {
    Idle,
    Available,
    Downloading,
    Ready,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppUpdateStatus {
    pub phase: AppUpdatePhase,
    pub info: Option<UpdateInfo>,
    pub dismissed: bool,
    pub last_checked_at: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UpdatePreferences {
    dismissed_version: Option<String>,
    last_checked_at: Option<i64>,
}

struct PendingUpdate {
    update: Update,
    bytes: Vec<u8>,
}

struct CoordinatorInner {
    status: AppUpdateStatus,
    dismissed_version: Option<String>,
    checking: bool,
    pending: Option<PendingUpdate>,
}

pub struct UpdateCoordinator {
    inner: Mutex<CoordinatorInner>,
    db: crate::db::Db,
}

impl UpdateCoordinator {
    pub fn new(db: crate::db::Db) -> Self {
        let preferences = db
            .get_kv(UPDATE_PREFERENCES_KEY)
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_str::<UpdatePreferences>(&value).ok())
            .unwrap_or_default();
        Self {
            inner: Mutex::new(CoordinatorInner {
                status: AppUpdateStatus {
                    phase: AppUpdatePhase::Idle,
                    info: None,
                    dismissed: false,
                    last_checked_at: preferences.last_checked_at,
                },
                dismissed_version: preferences.dismissed_version,
                checking: false,
                pending: None,
            }),
            db,
        }
    }

    pub fn status(&self) -> AppUpdateStatus {
        self.inner.lock().unwrap().status.clone()
    }

    fn persist(&self, inner: &CoordinatorInner) -> Result<()> {
        self.db.put_kv(
            UPDATE_PREFERENCES_KEY,
            &serde_json::to_string(&UpdatePreferences {
                dismissed_version: inner.dismissed_version.clone(),
                last_checked_at: inner.status.last_checked_at,
            })?,
        )
    }

    fn begin_check(&self) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if inner.checking
            || matches!(
                inner.status.phase,
                AppUpdatePhase::Downloading | AppUpdatePhase::Ready
            )
        {
            return false;
        }
        inner.checking = true;
        true
    }

    fn finish_check(&self, info: Option<UpdateInfo>) -> Result<AppUpdateStatus> {
        let mut inner = self.inner.lock().unwrap();
        inner.checking = false;
        if let Some(info) = info {
            let latest = info.latest.as_deref();
            let dismissed_version = inner.dismissed_version.clone();
            inner.status.dismissed =
                latest.is_some_and(|version| dismissed_version.as_deref() == Some(version));
            inner.status.phase = if info.update_available {
                AppUpdatePhase::Available
            } else {
                AppUpdatePhase::Idle
            };
            inner.status.info = Some(info);
            inner.status.last_checked_at = Some(chrono::Utc::now().timestamp());
            self.persist(&inner)?;
        }
        Ok(inner.status.clone())
    }

    fn fail_check(&self) {
        self.inner.lock().unwrap().checking = false;
    }

    pub fn dismiss(&self, version: &str) -> Result<AppUpdateStatus> {
        let mut inner = self.inner.lock().unwrap();
        if inner
            .status
            .info
            .as_ref()
            .and_then(|info| info.latest.as_deref())
            != Some(version)
        {
            return Err(Error::other("That update is no longer current."));
        }
        inner.dismissed_version = Some(version.to_string());
        inner.status.dismissed = true;
        self.persist(&inner)?;
        Ok(inner.status.clone())
    }

    fn begin_download(&self) -> Result<UpdateInfo> {
        let mut inner = self.inner.lock().unwrap();
        if inner.status.phase == AppUpdatePhase::Downloading {
            return Err(Error::other("The update is already downloading."));
        }
        if inner.status.phase == AppUpdatePhase::Ready {
            return Err(Error::other("The update is already ready to install."));
        }
        let info = inner
            .status
            .info
            .clone()
            .filter(|info| info.update_available)
            .ok_or_else(|| Error::other("Check for updates before downloading."))?;
        if info.install_source.policy != UpdatePolicy::SelfManaged {
            return Err(Error::other(info.install_source.update_hint.clone()));
        }
        inner.status.phase = AppUpdatePhase::Downloading;
        inner.status.dismissed = true;
        Ok(info)
    }

    fn download_failed(&self) -> AppUpdateStatus {
        let mut inner = self.inner.lock().unwrap();
        inner.status.phase = AppUpdatePhase::Available;
        inner.status.dismissed = false;
        inner.status.clone()
    }

    fn download_ready(&self, pending: PendingUpdate) -> AppUpdateStatus {
        let mut inner = self.inner.lock().unwrap();
        inner.pending = Some(pending);
        inner.status.phase = AppUpdatePhase::Ready;
        inner.status.clone()
    }

    fn take_pending(&self) -> Result<PendingUpdate> {
        self.inner
            .lock()
            .unwrap()
            .pending
            .take()
            .ok_or_else(|| Error::other("No verified update is ready to install."))
    }

    fn restore_pending(&self, pending: PendingUpdate) {
        self.inner.lock().unwrap().pending = Some(pending);
    }
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
}

#[derive(Debug, PartialEq)]
struct Version {
    release: Vec<u64>,
    dev: Option<Vec<u64>>,
}

fn parse_version(raw: &str) -> Option<Version> {
    let raw = raw.trim().trim_start_matches(['v', 'V']);
    let (release, dev) = match raw.split_once("-dev.") {
        Some((release, dev)) => (release, Some(dev)),
        None => (raw, None),
    };
    let release = release
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<Vec<u64>, _>>()
        .ok()?;
    if release.is_empty() {
        return None;
    }
    let dev = dev
        .map(|value| {
            value
                .split('.')
                .map(str::parse)
                .collect::<std::result::Result<Vec<u64>, _>>()
        })
        .transpose()
        .ok()?;
    if dev.as_ref().is_some_and(Vec::is_empty) {
        return None;
    }
    Some(Version { release, dev })
}

fn compare_parts(latest: &[u64], current: &[u64]) -> std::cmp::Ordering {
    for i in 0..latest.len().max(current.len()) {
        let ordering = latest
            .get(i)
            .copied()
            .unwrap_or(0)
            .cmp(&current.get(i).copied().unwrap_or(0));
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    std::cmp::Ordering::Equal
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let (Some(latest), Some(current)) = (parse_version(latest), parse_version(current)) else {
        return false;
    };
    match compare_parts(&latest.release, &current.release) {
        std::cmp::Ordering::Greater => return true,
        std::cmp::Ordering::Less => return false,
        std::cmp::Ordering::Equal => {}
    }
    match (latest.dev, current.dev) {
        (None, Some(_)) => true,
        (Some(_), None) | (None, None) => false,
        (Some(latest), Some(current)) => {
            compare_parts(&latest, &current) == std::cmp::Ordering::Greater
        }
    }
}

fn latest_development_release(releases: Vec<Release>) -> Option<Release> {
    releases
        .into_iter()
        .filter(|release| release.prerelease && release.tag_name.contains("-dev."))
        .max_by(|a, b| {
            if is_newer(&a.tag_name, &b.tag_name) {
                std::cmp::Ordering::Greater
            } else if is_newer(&b.tag_name, &a.tag_name) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
}

fn up_to_date(current: String) -> UpdateInfo {
    UpdateInfo {
        current,
        latest: None,
        notes_url: None,
        published_at: None,
        update_available: false,
        install_source: install_source(),
    }
}

#[tracing::instrument(skip_all, err)]
pub async fn check(client: &NetworkManager) -> Result<UpdateInfo> {
    let current = crate::build_info::display_version();
    let development = crate::build_info::CHANNEL == "dev";
    let url = if development {
        format!("https://api.github.com/repos/{REPO}/releases?per_page=100")
    } else {
        format!("https://api.github.com/repos/{REPO}/releases/latest")
    };

    let request = client
        .get(&url)
        .header("Accept", "application/vnd.github+json");
    let response = client.send(request).await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        tracing::info!(repo = REPO, "no published releases yet");
        return Ok(up_to_date(current));
    }

    let response = response.error_for_status()?;
    let release = if development {
        latest_development_release(response.json::<Vec<Release>>().await?)
    } else {
        Some(response.json::<Release>().await?)
    };
    let Some(release) = release else {
        tracing::info!(repo = REPO, channel = "dev", "no development releases yet");
        return Ok(up_to_date(current));
    };
    let update_available = is_newer(&release.tag_name, &current);
    tracing::info!(
        current = %current,
        latest = %release.tag_name,
        update_available,
        "checked for updates"
    );

    Ok(UpdateInfo {
        current,
        latest: Some(release.tag_name),
        notes_url: Some(release.html_url),
        published_at: release.published_at,
        update_available,
        install_source: install_source(),
    })
}

async fn signed_update(app: &AppHandle, client: &NetworkManager) -> Result<Update> {
    let manifest_url = if crate::build_info::CHANNEL == "dev" {
        let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=100");
        let response = client
            .send(
                client
                    .get(&url)
                    .header("Accept", "application/vnd.github+json"),
            )
            .await?
            .error_for_status()?;
        let release = latest_development_release(response.json::<Vec<Release>>().await?)
            .ok_or_else(|| Error::other("No development update is available."))?;
        format!(
            "{REPO_URL}/releases/download/{}/latest.json",
            release.tag_name
        )
    } else {
        format!("{REPO_URL}/releases/latest/download/latest.json")
    };
    let endpoint = manifest_url
        .parse()
        .map_err(|error| Error::other(format!("invalid update endpoint: {error}")))?;
    let current = crate::build_info::display_version();
    app.updater_builder()
        .version_comparator(move |_, release| is_newer(&release.version.to_string(), &current))
        .endpoints(vec![endpoint])
        .map_err(|error| Error::other(format!("could not configure updater: {error}")))?
        .build()
        .map_err(|error| Error::other(format!("could not start updater: {error}")))?
        .check()
        .await
        .map_err(|error| Error::other(format!("could not check signed update: {error}")))?
        .ok_or_else(|| Error::other("No signed update is available for this build."))
}

pub fn emit_status(app: &AppHandle, status: &AppUpdateStatus) {
    let _ = app.emit("app:update-status", status);
}

#[tracing::instrument(skip_all, err)]
pub async fn check_and_record(
    app: &AppHandle,
    client: &NetworkManager,
    coordinator: &UpdateCoordinator,
) -> Result<UpdateInfo> {
    if !coordinator.begin_check() {
        return coordinator
            .status()
            .info
            .ok_or_else(|| Error::other("An update check is already running."));
    }

    match check(client).await {
        Ok(info) => {
            let status = coordinator.finish_check(Some(info.clone()))?;
            emit_status(app, &status);
            Ok(info)
        }
        Err(error) => {
            coordinator.fail_check();
            Err(error)
        }
    }
}

#[tracing::instrument(skip_all, err)]
pub async fn download(app: AppHandle, state: &crate::state::AppState) -> Result<AppUpdateStatus> {
    let info = state.updates.begin_download()?;
    let status = state.updates.status();
    emit_status(&app, &status);

    let task = state.tasks.start(
        &app,
        crate::tasks::TaskKind::AppUpdate,
        crate::tasks::TaskSpec {
            title: format!("Basalt {}", info.latest.as_deref().unwrap_or("update")),
            subtitle: Some("Launcher update".to_string()),
            ..Default::default()
        },
    )?;

    let result = async {
        task.stage("preparing");
        let update = signed_update(&app, &state.network).await?;
        task.stage("downloading");
        let token = task.token();
        let mut downloaded = 0_u64;
        let bytes = {
            let download = update.download(
                |chunk_length, content_length| {
                    downloaded = downloaded.saturating_add(chunk_length as u64);
                    task.progress(0, 0, downloaded, content_length.unwrap_or(0));
                },
                || task.stage("verifying"),
            );
            tokio::pin!(download);
            tokio::select! {
                result = &mut download => result
                    .map_err(|error| Error::other(format!("could not download update: {error}")))?,
                () = token.cancelled() => return Err(Error::Cancelled),
            }
        };
        Ok(PendingUpdate { update, bytes })
    }
    .await;

    match result {
        Ok(pending) => {
            let status = state.updates.download_ready(pending);
            task.succeed();
            emit_status(&app, &status);
            Ok(status)
        }
        Err(error) => {
            let status = state.updates.download_failed();
            match &error {
                Error::Cancelled => task.cancelled(),
                _ => task.fail(&error),
            }
            emit_status(&app, &status);
            Err(error)
        }
    }
}

#[tracing::instrument(skip_all, err)]
pub fn install_ready(app: AppHandle, state: &crate::state::AppState) -> Result<()> {
    let game_running = state
        .running
        .lock()
        .unwrap()
        .values()
        .any(|handle| handle.status.lock().unwrap().state == "running");
    if game_running {
        return Err(Error::other("Close the game before restarting Basalt."));
    }
    if state
        .tasks
        .list()
        .iter()
        .any(|task| task.state == crate::tasks::TaskState::Running)
    {
        return Err(Error::other(
            "Wait for active downloads and installs to finish before updating.",
        ));
    }

    let pending = state.updates.take_pending()?;
    if let Err(error) = pending.update.install(&pending.bytes) {
        state.updates.restore_pending(pending);
        return Err(Error::other(format!("could not install update: {error}")));
    }
    app.restart();
}

pub fn start_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(INITIAL_CHECK_DELAY).await;
        let mut failures = 0_u32;
        loop {
            let state = app.state::<crate::state::AppState>();
            match state.db.load_settings() {
                Ok(settings) if !settings.auto_update_checks => {
                    tokio::time::sleep(DISABLED_CHECK_INTERVAL).await;
                    continue;
                }
                Err(error) => {
                    tracing::warn!(error = %error, "could not read automatic update preference");
                }
                _ => {}
            }
            let status = state.updates.status();
            if matches!(
                status.phase,
                AppUpdatePhase::Downloading | AppUpdatePhase::Ready
            ) {
                tokio::time::sleep(Duration::from_secs(MIN_CHECK_INTERVAL as u64)).await;
                continue;
            }
            let now = chrono::Utc::now().timestamp();
            let since_last = status
                .last_checked_at
                .map(|checked| now.saturating_sub(checked))
                .unwrap_or(MIN_CHECK_INTERVAL);
            if since_last < MIN_CHECK_INTERVAL {
                tokio::time::sleep(Duration::from_secs(
                    (MIN_CHECK_INTERVAL - since_last) as u64,
                ))
                .await;
                continue;
            }

            match check_and_record(&app, &state.network, &state.updates).await {
                Ok(_) => failures = 0,
                Err(error) => {
                    failures = failures.saturating_add(1);
                    tracing::warn!(error = %error, failures, "automatic update check failed");
                }
            }

            let delay = if failures == 0 {
                let jitter = now.rem_euclid(2 * 60 * 60) as u64;
                Duration::from_secs(MIN_CHECK_INTERVAL as u64 + jitter)
            } else {
                Duration::from_secs((15 * 60 * 2_u64.pow(failures.min(3) - 1)).min(2 * 60 * 60))
            };
            tokio::time::sleep(delay).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{
        is_newer, latest_development_release, linux_install_source, parse_version, Release,
        UpdatePolicy, Version,
    };

    fn release(tag_name: &str, prerelease: bool) -> Release {
        Release {
            tag_name: tag_name.to_string(),
            html_url: format!("https://example.com/{tag_name}"),
            prerelease,
            published_at: None,
        }
    }

    #[test]
    fn strips_tag_prefixes_and_suffixes() {
        assert_eq!(
            parse_version("v1.2.3-dev.42.2"),
            Some(Version {
                release: vec![1, 2, 3],
                dev: Some(vec![42, 2])
            })
        );
        assert_eq!(
            parse_version("0.1.0"),
            Some(Version {
                release: vec![0, 1, 0],
                dev: None
            })
        );
        assert_eq!(parse_version("2.0.0-beta.1"), None);
        assert_eq!(parse_version("nightly"), None);
    }

    #[test]
    fn compares_versions_by_component() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("nightly", "0.1.0"));
    }

    #[test]
    fn compares_development_builds() {
        assert!(is_newer("v1.0.0-dev.5.1", "1.0.0-dev.4.1"));
        assert!(is_newer("v1.0.0-dev.5.2", "1.0.0-dev.5.1"));
        assert!(!is_newer("v1.0.0-dev.5.1", "1.0.0-dev.5.1"));
        assert!(!is_newer("v1.0.0-dev.4.1", "1.0.0-dev.5.1"));
        assert!(is_newer("v1.0.0", "1.0.0-dev.5.1"));
        assert!(!is_newer("v1.0.0-dev.5.1", "1.0.0"));
    }

    #[test]
    fn development_feed_ignores_stable_and_unrelated_prereleases() {
        let latest = latest_development_release(vec![
            release("v2.0.0", false),
            release("v1.0.0-beta.1", true),
            release("v1.0.0-dev.4.1", true),
            release("v1.0.0-dev.5.1", true),
        ])
        .unwrap();

        assert_eq!(latest.tag_name, "v1.0.0-dev.5.1");
    }

    #[test]
    fn treats_missing_components_as_zero() {
        assert!(!is_newer("1.0", "1.0.0"));
        assert!(is_newer("1.0.1", "1.0"));
    }

    #[test]
    fn linux_appimage_is_self_managed() {
        let source = linux_install_source("linux_bundle", true, false, false, false, false);
        assert_eq!(source.id, "appimage");
        assert_eq!(source.policy, UpdatePolicy::SelfManaged);
    }

    #[test]
    fn linux_packages_take_precedence_over_bundle_fallback() {
        let aur = linux_install_source("linux_bundle", false, false, false, false, true);
        let nix = linux_install_source("nix", false, false, false, true, false);
        assert_eq!(aur.id, "aur");
        assert_eq!(nix.id, "nix");
        assert_eq!(aur.policy, UpdatePolicy::PackageManaged);
    }

    #[test]
    fn linux_bundle_without_appimage_is_a_manual_debian_package() {
        let source = linux_install_source("linux_bundle", false, false, false, false, false);
        assert_eq!(source.id, "deb");
        assert_eq!(source.policy, UpdatePolicy::Manual);
    }
}
