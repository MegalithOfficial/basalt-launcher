use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn resolve(app: &AppHandle) -> Result<Self> {
        let root = app.path().app_data_dir()?;
        Ok(Self { root })
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
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn run_log(&self, running_id: &str, stream: &str) -> PathBuf {
        self.logs()
            .join("runs")
            .join(format!("{running_id}.{stream}.log"))
    }
    pub fn skins(&self) -> PathBuf {
        self.root.join("media").join("skins")
    }
    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances().join(id)
    }

    pub fn instance_dir_checked(&self, id: &str) -> Option<PathBuf> {
        let id = id.trim();
        if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
            return None;
        }
        let dir = self.instances().join(id);
        if dir.parent() != Some(self.instances().as_path()) {
            return None;
        }
        Some(dir)
    }

    pub fn instance_saves_dir_checked(&self, id: &str) -> Option<PathBuf> {
        self.instance_dir_checked(id).map(|dir| dir.join("saves"))
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::Paths;

    fn paths() -> Paths {
        Paths {
            root: PathBuf::from("/tmp/basalt-test"),
        }
    }

    #[test]
    fn blank_and_traversing_ids_are_refused() {
        let p = paths();
        assert!(p.instance_dir_checked("").is_none());
        assert!(p.instance_dir_checked("   ").is_none());
        assert!(p.instance_dir_checked("..").is_none());
        assert!(p.instance_dir_checked("../..").is_none());
        assert!(p.instance_dir_checked("a/b").is_none());
    }

    #[test]
    fn real_ids_resolve_under_instances() {
        let p = paths();
        let dir = p
            .instance_dir_checked("c4dbff5d-a385-47fb-9710-7d33bd154c3f")
            .unwrap();
        assert_eq!(dir.parent().unwrap(), p.instances());
    }
}
