use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::error::Result;

/// Resolves and owns the launcher's on-disk directory layout. Mirrors the standard
/// Mojang layout so the data dir interops with the wider ecosystem (and future modpacks).
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    /// Resolve the launcher root from the platform app-data directory.
    pub fn resolve(app: &AppHandle) -> Result<Self> {
        let root = app.path().app_data_dir()?;
        Ok(Self { root })
    }

    /// Create every directory the launcher expects. Idempotent.
    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [
            self.versions(),
            self.libraries(),
            self.assets_indexes(),
            self.assets_objects(),
            self.natives(),
            self.runtimes(),
            self.instances(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions().join(id)
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }
    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn assets_indexes(&self) -> PathBuf {
        self.assets().join("indexes")
    }
    pub fn assets_objects(&self) -> PathBuf {
        self.assets().join("objects")
    }
    pub fn natives(&self) -> PathBuf {
        self.root.join("natives")
    }
    pub fn natives_dir(&self, id: &str) -> PathBuf {
        self.natives().join(id)
    }
    pub fn runtimes(&self) -> PathBuf {
        self.root.join("runtimes")
    }
    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }
    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances().join(id)
    }
    pub fn manifest_cache(&self) -> PathBuf {
        self.root.join("version_manifest_v2.json")
    }
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("launcher.json")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }
    pub fn instances_file(&self) -> PathBuf {
        self.root.join("instances.json")
    }
}
