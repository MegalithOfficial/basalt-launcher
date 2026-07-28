use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use crate::db::Db;
use crate::launch::process::RunningHandle;
use crate::meta::media::{PatchNotes, VersionMedia};
use crate::paths::Paths;
use crate::search::http::RateLimiter;
use crate::tasks::Tasks;

const USER_AGENT: &str = concat!(
    "MegalithOfficial/basalt-launcher/",
    env!("CARGO_PKG_VERSION"),
    " (github.com/MegalithOfficial/basalt-launcher)"
);

const REQUESTS_PER_MINUTE: usize = 250;
const MAX_CONCURRENT_API_CALLS: usize = 8;

pub struct AppState {
    pub http: reqwest::Client,
    pub limiter: RateLimiter,
    pub paths: Paths,
    pub db: Db,
    pub running: Mutex<HashMap<String, RunningHandle>>,
    pub patch_notes: Mutex<Option<PatchNotes>>,
    pub media_cache: Mutex<HashMap<String, Option<VersionMedia>>>,
    pub tasks: std::sync::Arc<Tasks>,
}

impl AppState {
    pub fn new(paths: Paths, db: Db) -> Self {
        let tasks = std::sync::Arc::new(Tasks::new(db.clone()));
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(45))
            .build()
            .expect("failed to build HTTP client");
        Self {
            http,
            limiter: RateLimiter::new(
                REQUESTS_PER_MINUTE,
                Duration::from_secs(60),
                MAX_CONCURRENT_API_CALLS,
            ),
            paths,
            db,
            running: Mutex::new(HashMap::new()),
            patch_notes: Mutex::new(None),
            media_cache: Mutex::new(HashMap::new()),
            tasks,
        }
    }
}
