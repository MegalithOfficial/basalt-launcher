use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    config::LauncherSettings,
    credentials::CredentialStore,
    db::Db,
    files::FileManager,
    launch::process::RunningHandle,
    meta::media::{PatchNotes, VersionMedia},
    network::NetworkManager,
    paths::Paths,
    presence::Presence,
    servers::runtime::Registry,
    tasks::Tasks,
    update::UpdateCoordinator,
};

pub struct AppState {
    pub network: Arc<NetworkManager>,
    pub files: FileManager,
    pub paths: Paths,
    pub db: Db,
    pub credentials: CredentialStore,
    pub running: Arc<Mutex<HashMap<String, RunningHandle>>>,
    pub servers: Registry,
    pub patch_notes: Mutex<Option<PatchNotes>>,
    pub media_cache: Mutex<HashMap<String, Option<VersionMedia>>>,
    pub tasks: std::sync::Arc<Tasks>,
    pub updates: Arc<UpdateCoordinator>,
    pub presence: Arc<Presence>,
}

impl AppState {
    pub fn new(files: FileManager, db: Db) -> Self {
        let paths = files.paths().clone();
        let credentials = CredentialStore::system();
        if !CredentialStore::available() {
            tracing::warn!("the operating system credential store is unavailable");
        }
        let tasks = std::sync::Arc::new(Tasks::new(db.clone()));
        let updates = Arc::new(UpdateCoordinator::new(db.clone()));
        let network = NetworkManager::new();
        if let Ok(settings) = db.load_runtime_settings(&credentials) {
            if let Err(error) = network.reconfigure(&settings) {
                tracing::warn!(error = %error, "could not apply the saved network settings");
            }
        }
        Self {
            network: Arc::new(network),
            files,
            paths,
            db,
            credentials,
            running: Arc::new(Mutex::new(HashMap::new())),
            servers: Arc::new(Mutex::new(HashMap::new())),
            patch_notes: Mutex::new(None),
            media_cache: Mutex::new(HashMap::new()),
            tasks,
            updates,
            presence: Arc::new(Presence::spawn()),
        }
    }

    pub fn runtime_settings(&self) -> crate::error::Result<LauncherSettings> {
        self.db.load_runtime_settings(&self.credentials)
    }
}
