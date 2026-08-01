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
    update::UpdateCoordinator,
};

pub struct AppState {
    pub network: Arc<NetworkManager>,
    pub files: FileManager,
    pub paths: Paths,
    pub db: Db,
    pub running: Arc<Mutex<HashMap<String, RunningHandle>>>,
    pub patch_notes: Mutex<Option<PatchNotes>>,
    pub media_cache: Mutex<HashMap<String, Option<VersionMedia>>>,
    pub tasks: std::sync::Arc<Tasks>,
    pub updates: Arc<UpdateCoordinator>,
}

impl AppState {
    pub fn new(files: FileManager, db: Db) -> Self {
        let paths = files.paths().clone();
        let tasks = std::sync::Arc::new(Tasks::new(db.clone()));
        let updates = Arc::new(UpdateCoordinator::new(db.clone()));
        let network = NetworkManager::new();
        if let Ok(settings) = db.load_settings() {
            if let Err(error) = network.reconfigure(&settings) {
                tracing::warn!(error = %error, "could not apply the saved network settings");
            }
        }
        Self {
            network: Arc::new(network),
            files,
            paths,
            db,
            running: Arc::new(Mutex::new(HashMap::new())),
            patch_notes: Mutex::new(None),
            media_cache: Mutex::new(HashMap::new()),
            tasks,
            updates,
        }
    }
}
