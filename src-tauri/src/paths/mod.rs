use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use tauri::{AppHandle, Manager};

use crate::error::Result;

pub const OVERRIDES_FILE: &str = "paths.json";

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum DataRoot {
    Instances,
    Servers,
    Versions,
    Libraries,
    Assets,
    Natives,
    Runtimes,
    Snapshots,
    Cache,
}

impl DataRoot {
    pub const ALL: [DataRoot; 9] = [
        DataRoot::Instances,
        DataRoot::Servers,
        DataRoot::Versions,
        DataRoot::Libraries,
        DataRoot::Assets,
        DataRoot::Natives,
        DataRoot::Runtimes,
        DataRoot::Snapshots,
        DataRoot::Cache,
    ];

    pub fn directory(self) -> &'static str {
        match self {
            DataRoot::Instances => "instances",
            DataRoot::Servers => "servers",
            DataRoot::Versions => "versions",
            DataRoot::Libraries => "libraries",
            DataRoot::Assets => "assets",
            DataRoot::Natives => "natives",
            DataRoot::Runtimes => "runtimes",
            DataRoot::Snapshots => "snapshots",
            DataRoot::Cache => "cache",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DataRoot::Instances => "Instances",
            DataRoot::Servers => "Servers",
            DataRoot::Versions => "Game versions",
            DataRoot::Libraries => "Libraries",
            DataRoot::Assets => "Assets",
            DataRoot::Natives => "Native libraries",
            DataRoot::Runtimes => "Java runtimes",
            DataRoot::Snapshots => "Snapshots",
            DataRoot::Cache => "Cache",
        }
    }

    pub fn summary(self) -> &'static str {
        match self {
            DataRoot::Instances => {
                "Worlds, mods, configs and screenshots. The largest folder by far."
            }
            DataRoot::Servers => "Worlds, plugins and configs for the servers Basalt hosts.",
            DataRoot::Versions => "The client jars Basalt downloads for each Minecraft version.",
            DataRoot::Libraries => "Shared jars every instance links against.",
            DataRoot::Assets => "Sounds, languages and textures, shared across every version.",
            DataRoot::Natives => "Platform binaries unpacked next to each version.",
            DataRoot::Runtimes => "Java builds Basalt installed for you.",
            DataRoot::Snapshots => "Compressed instance backups.",
            DataRoot::Cache => "Rebuildable downloads and metadata. Safe to lose.",
        }
    }
}

pub const MAX_EXTRA_ROOTS: usize = 64;

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    overrides: Arc<RwLock<BTreeMap<DataRoot, PathBuf>>>,
    extras: Arc<RwLock<Vec<PathBuf>>>,
}

impl Paths {
    pub fn resolve(app: &AppHandle) -> Result<Self> {
        let root = app.path().app_data_dir()?;
        let overrides = read_overrides(&root);
        Ok(Self::relocated(root, overrides))
    }

    #[cfg(test)]
    pub fn plain(root: PathBuf) -> Self {
        Self::relocated(root, BTreeMap::new())
    }

    pub fn relocated(root: PathBuf, overrides: BTreeMap<DataRoot, PathBuf>) -> Self {
        Self {
            root,
            overrides: Arc::new(RwLock::new(overrides)),
            extras: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn adopt(&self, overrides: BTreeMap<DataRoot, PathBuf>) {
        *self.overrides.write().unwrap() = overrides;
    }

    pub fn adopt_extras(&self, extras: Vec<PathBuf>) {
        let managed = self.managed_roots();
        let mut kept: Vec<PathBuf> = Vec::new();
        for path in extras {
            if !path.is_absolute() || managed.iter().any(|root| path.starts_with(root)) {
                continue;
            }
            if kept.iter().any(|other| path.starts_with(other)) {
                continue;
            }
            kept.retain(|other| !other.starts_with(&path));
            kept.push(path);
            if kept.len() == MAX_EXTRA_ROOTS {
                break;
            }
        }
        *self.extras.write().unwrap() = kept;
    }

    pub fn extras(&self) -> Vec<PathBuf> {
        self.extras.read().unwrap().clone()
    }

    fn located(&self, slot: DataRoot) -> PathBuf {
        self.overrides
            .read()
            .unwrap()
            .get(&slot)
            .cloned()
            .unwrap_or_else(|| self.root.join(slot.directory()))
    }

    pub fn overrides(&self) -> BTreeMap<DataRoot, PathBuf> {
        self.overrides.read().unwrap().clone()
    }

    pub fn located_at(&self, slot: DataRoot) -> PathBuf {
        self.located(slot)
    }

    pub fn default_for(&self, slot: DataRoot) -> PathBuf {
        self.root.join(slot.directory())
    }

    fn managed_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![self.root.clone()];
        for path in self.overrides.read().unwrap().values() {
            if !path.starts_with(&self.root) {
                roots.push(path.clone());
            }
        }
        roots
    }

    pub fn capability_roots(&self) -> Vec<PathBuf> {
        let mut roots = self.managed_roots();
        roots.sort_by_key(|path| std::cmp::Reverse(path.as_os_str().len()));
        roots.dedup();
        roots
    }

    pub fn extra_roots(&self) -> Vec<PathBuf> {
        let managed = self.capability_roots();
        let mut roots = self
            .extras
            .read()
            .unwrap()
            .iter()
            .filter(|path| !managed.iter().any(|root| path.starts_with(root)))
            .cloned()
            .collect::<Vec<_>>();
        roots.sort_by_key(|path| std::cmp::Reverse(path.as_os_str().len()));
        roots.dedup();
        roots
    }

    pub fn unavailable(&self) -> Vec<(DataRoot, PathBuf)> {
        self.overrides
            .read()
            .unwrap()
            .iter()
            .filter(|(_, path)| !parent_exists(path))
            .map(|(slot, path)| (*slot, path.clone()))
            .collect()
    }

    pub fn versions(&self) -> PathBuf {
        self.located(DataRoot::Versions)
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
        self.located(DataRoot::Libraries)
    }
    pub fn assets(&self) -> PathBuf {
        self.located(DataRoot::Assets)
    }
    pub fn assets_indexes(&self) -> PathBuf {
        self.assets().join("indexes")
    }
    pub fn assets_objects(&self) -> PathBuf {
        self.assets().join("objects")
    }
    pub fn natives(&self) -> PathBuf {
        self.located(DataRoot::Natives)
    }
    pub fn natives_dir(&self, id: &str) -> PathBuf {
        self.natives().join(id)
    }
    pub fn runtimes(&self) -> PathBuf {
        self.located(DataRoot::Runtimes)
    }
    pub fn instances(&self) -> PathBuf {
        self.located(DataRoot::Instances)
    }
    pub fn servers(&self) -> PathBuf {
        self.located(DataRoot::Servers)
    }
    pub fn server_dir(&self, id: &str) -> PathBuf {
        self.servers().join(id)
    }
    pub fn modpack_upgrade_journals(&self) -> PathBuf {
        self.root.join("modpack-upgrade-journals")
    }
    pub fn modpack_upgrade_journal(&self, instance_id: &str) -> Option<PathBuf> {
        self.instance_dir_checked(instance_id)?;
        let parent = self.modpack_upgrade_journals();
        let path = parent.join(format!("{instance_id}.json"));
        (path.parent() == Some(parent.as_path())).then_some(path)
    }
    pub fn snapshots(&self) -> PathBuf {
        self.located(DataRoot::Snapshots)
    }
    pub fn cache(&self) -> PathBuf {
        self.located(DataRoot::Cache)
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

    pub fn server_dir_checked(&self, id: &str) -> Option<PathBuf> {
        let id = id.trim();
        if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
            return None;
        }
        let dir = self.servers().join(id);
        if dir.parent() != Some(self.servers().as_path()) {
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

pub fn parent_exists(path: &Path) -> bool {
    match path.parent() {
        Some(parent) => parent.is_dir(),
        None => false,
    }
}

fn read_overrides(root: &Path) -> BTreeMap<DataRoot, PathBuf> {
    let Ok(bytes) = std::fs::read(root.join(OVERRIDES_FILE)) else {
        return BTreeMap::new();
    };
    match serde_json::from_slice::<BTreeMap<DataRoot, PathBuf>>(&bytes) {
        Ok(map) => map
            .into_iter()
            .filter(|(_, path)| path.is_absolute())
            .collect(),
        Err(error) => {
            tracing::warn!(error = %error, "ignoring an unreadable {OVERRIDES_FILE}");
            BTreeMap::new()
        }
    }
}

pub fn write_overrides(root: &Path, overrides: &BTreeMap<DataRoot, PathBuf>) -> Result<()> {
    let path = root.join(OVERRIDES_FILE);
    if overrides.is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    std::fs::write(&path, serde_json::to_vec_pretty(overrides)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::Paths;

    fn paths() -> Paths {
        Paths::plain(PathBuf::from("/tmp/basalt-test"))
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
        assert_eq!(
            p.modpack_upgrade_journal("c4dbff5d-a385-47fb-9710-7d33bd154c3f")
                .unwrap()
                .parent(),
            Some(p.modpack_upgrade_journals().as_path())
        );
    }

    #[test]
    fn server_ids_resolve_under_servers() {
        let p = paths();
        assert!(p.server_dir_checked("").is_none());
        assert!(p.server_dir_checked("../escape").is_none());
        assert!(p.server_dir_checked("a/b").is_none());
        let dir = p.server_dir_checked("a2a2b0f0-0000-4000-8000-000000000000");
        assert_eq!(dir.unwrap().parent().unwrap(), p.servers());
    }

    #[test]
    fn extra_roots_drop_anything_already_covered() {
        let p = paths();
        p.adopt_extras(vec![
            p.root.join("servers").join("inside"),
            PathBuf::from("/mnt/disk/smp"),
            PathBuf::from("/mnt/disk/smp/world"),
            PathBuf::from("relative/path"),
        ]);
        assert_eq!(p.extra_roots(), vec![PathBuf::from("/mnt/disk/smp")]);
        assert!(p.capability_roots().iter().all(|root| *root == p.root));
        assert!(p.unavailable().is_empty());
    }

    #[test]
    fn an_extra_root_replaces_the_nested_ones_it_covers() {
        let p = paths();
        p.adopt_extras(vec![
            PathBuf::from("/mnt/disk/smp/world"),
            PathBuf::from("/mnt/disk/smp"),
        ]);
        assert_eq!(p.extra_roots(), vec![PathBuf::from("/mnt/disk/smp")]);
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
