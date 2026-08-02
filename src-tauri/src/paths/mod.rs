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
    pub fn snapshots(&self) -> PathBuf {
        self.root.join("snapshots")
    }
    pub fn snapshot_blobs(&self) -> PathBuf {
        self.snapshots().join("blobs")
    }
    pub fn snapshot_instances(&self) -> PathBuf {
        self.snapshots().join("instances")
    }
    pub fn snapshot_restore_journals(&self) -> PathBuf {
        self.snapshots().join("restore-journals")
    }
    pub fn instance_snapshots(&self, instance_id: &str) -> PathBuf {
        self.snapshot_instances().join(instance_id)
    }
    pub fn snapshot_dir(&self, instance_id: &str, snapshot_id: &str) -> PathBuf {
        self.instance_snapshots(instance_id)
            .join(format!("{snapshot_id}.json"))
    }
    pub fn snapshot_blob(&self, sha256: &str) -> Option<PathBuf> {
        if sha256.len() != 64
            || !sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return None;
        }
        Some(
            self.snapshot_blobs()
                .join(&sha256[..2])
                .join(format!("{sha256}.zst")),
        )
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn run_log(&self, running_id: &str, stream: &str) -> PathBuf {
        self.logs()
            .join("runs")
            .join(format!("{running_id}.{stream}.log"))
    }
    pub fn media(&self) -> PathBuf {
        self.root.join("media")
    }

    pub fn banner_library(&self) -> PathBuf {
        self.root.join("media").join("library")
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

    pub fn snapshot_dir_checked(&self, instance_id: &str, snapshot_id: &str) -> Option<PathBuf> {
        self.instance_dir_checked(instance_id)?;
        let snapshot_id = snapshot_id.trim();
        if snapshot_id.is_empty()
            || snapshot_id.contains('/')
            || snapshot_id.contains('\\')
            || snapshot_id.contains("..")
        {
            return None;
        }
        let parent = self.instance_snapshots(instance_id);
        let path = parent.join(format!("{snapshot_id}.json"));
        (path.parent() == Some(parent.as_path())).then_some(path)
    }

    pub fn snapshot_restore_journal_checked(&self, instance_id: &str) -> Option<PathBuf> {
        self.instance_dir_checked(instance_id)?;
        let parent = self.snapshot_restore_journals();
        let path = parent.join(format!("{instance_id}.json"));
        (path.parent() == Some(parent.as_path())).then_some(path)
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

    #[test]
    fn snapshot_ids_cannot_escape_their_instance() {
        let p = paths();
        assert!(p.snapshot_dir_checked("instance", "snapshot").is_some());
        assert!(p.snapshot_dir_checked("instance", "../other").is_none());
        assert!(p.snapshot_dir_checked("../instance", "snapshot").is_none());
        assert!(p.snapshot_blob(&"a".repeat(64)).is_some());
        assert!(p.snapshot_blob("not-a-hash").is_none());
    }
}
