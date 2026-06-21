use std::collections::HashMap;
use std::sync::Mutex;

use crate::config::LauncherSettings;
use crate::launch::process::RunningHandle;
use crate::paths::Paths;

pub struct AppState {
    pub http: reqwest::Client,
    pub paths: Paths,
    pub settings: Mutex<LauncherSettings>,
    pub running: Mutex<HashMap<String, RunningHandle>>,
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
            running: Mutex::new(HashMap::new()),
        }
    }
}
