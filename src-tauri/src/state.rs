use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    db::Db,
    files::FileManager,
    launch::process::RunningHandle,
    meta::media::{PatchNotes, VersionMedia},
    network::NetworkManager,
    paths::Paths,
    tasks::Tasks,
};

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
    pub fn new(files: FileManager, db: Db) -> Self {
        let paths = files.paths().clone();
        let tasks = std::sync::Arc::new(Tasks::new(db.clone()));
        Self {
            network: Arc::new(NetworkManager::new()),
            files,
            paths,
            db,
            running: Mutex::new(HashMap::new()),
            patch_notes: Mutex::new(None),
            media_cache: Mutex::new(HashMap::new()),
            tasks,
        }
    }
}
