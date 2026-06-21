use std::sync::Mutex;

use crate::config::LauncherSettings;
use crate::paths::Paths;

/// Shared application state managed by Tauri and injected into commands via `State<AppState>`.
pub struct AppState {
    /// Shared HTTP client (connection pooling) for Mojang/Microsoft APIs and downloads.
    pub http: reqwest::Client,
    /// Resolved on-disk layout.
    pub paths: Paths,
    /// In-memory copy of persisted settings.
    pub settings: Mutex<LauncherSettings>,
    // Running-instance registry is added in Milestone 4.
}

impl AppState {
    pub fn new(paths: Paths, settings: LauncherSettings) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("basalt-launcher/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("failed to build HTTP client");
        Self {
            http,
            paths,
            settings: Mutex::new(settings),
        }
    }
}
