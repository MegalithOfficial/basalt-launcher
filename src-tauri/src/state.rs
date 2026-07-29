use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::db::Db;
use crate::files::FileManager;
use crate::launch::process::RunningHandle;
use crate::meta::media::{PatchNotes, VersionMedia};
use crate::network::NetworkManager;
use crate::paths::Paths;
use crate::tasks::Tasks;

pub struct AppState {
    pub network: Arc<NetworkManager>,
    pub files: FileManager,
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
        Self {
            network: Arc::new(NetworkManager::new()),
            files: FileManager::new(paths.clone()),
            paths,
            db,
            running: Mutex::new(HashMap::new()),
            patch_notes: Mutex::new(None),
            media_cache: Mutex::new(HashMap::new()),
            tasks,
        }
    }
}
