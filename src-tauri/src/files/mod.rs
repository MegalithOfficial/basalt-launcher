use std::{
    io::Write,
    path::{Component, Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::{
    error::{Error, Result},
    paths::Paths,
};
use cap_std::{
    ambient_authority,
    fs::{Dir, Metadata, OpenOptions},
};

type Roots = Arc<Vec<(PathBuf, Arc<Dir>)>>;

#[derive(Clone)]
pub struct FileManager {
    paths: Paths,
    roots: Arc<RwLock<Roots>>,
}

pub struct Tail {
    pub bytes: Vec<u8>,
    pub len: u64,
    pub restarted: bool,
}

impl FileManager {
    pub fn new(paths: Paths) -> Result<Self> {
        if let Some((slot, path)) = paths.unavailable().into_iter().next() {
            return Err(Error::other(format!(
                "the {} folder is on a drive that is not mounted: {}",
                slot.directory(),
                path.display()
            )));
        }
        let roots = open_roots(&paths)?;
        Ok(Self {
            paths,
            roots: Arc::new(RwLock::new(roots)),
        })
    }

    pub fn reopen(&self) -> Result<()> {
        if let Some((slot, path)) = self.paths.unavailable().into_iter().next() {
            return Err(Error::other(format!(
                "the {} folder is on a drive that is not mounted: {}",
                slot.directory(),
                path.display()
            )));
        }
        let roots = open_roots(&self.paths)?;
        *self.roots.write().unwrap() = roots;
        Ok(())
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn ensure_base_dirs(&self) -> Result<()> {
        for directory in [
            self.paths.versions(),
            self.paths.libraries(),
            self.paths.assets_indexes(),
            self.paths.assets_objects(),
            self.paths.natives(),
            self.paths.runtimes(),
            self.paths.instances(),
            self.paths.servers(),
            self.paths.snapshots(),
            self.paths.snapshot_blobs(),
            self.paths.snapshot_instances(),
            self.paths.snapshot_restore_journals(),
            self.paths.logs(),
            self.paths.skins(),
        ] {
            self.ensure_dir(directory)?;
        }
        Ok(())
    }

    fn unmanaged(path: &Path) -> Error {
        Error::other(format!(
            "refusing to access unmanaged path {}",
            path.display()
        ))
    }

    fn resolve<'a>(&self, path: &'a Path) -> Result<(Arc<Dir>, &'a Path)> {
        let roots = self.roots.read().unwrap().clone();
        for (prefix, dir) in roots.iter() {
            let Ok(relative) = path.strip_prefix(prefix) else {
                continue;
            };
            if relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            }) {
                return Err(Self::unmanaged(path));
            }
            return Ok((
                dir.clone(),
                if relative.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    relative
                },
            ));
        }
        Err(Self::unmanaged(path))
    }

    fn resolve_pair<'a>(
        &self,
        source: &'a Path,
        destination: &'a Path,
    ) -> Result<(Arc<Dir>, &'a Path, &'a Path)> {
        let (source_dir, source_relative) = self.resolve(source)?;
        let (destination_dir, destination_relative) = self.resolve(destination)?;
        if !Arc::ptr_eq(&source_dir, &destination_dir) {
            return Err(Error::other(format!(
                "refusing to move {} across data locations",
                source.display()
            )));
        }
        Ok((source_dir, source_relative, destination_relative))
    }

    pub fn ensure_dir(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let (dir, relative) = self.resolve(path)?;
        dir.create_dir_all(relative)?;
        Ok(())
    }

    pub async fn ensure_dir_async(&self, path: impl AsRef<Path>) -> Result<()> {
        let files = self.clone();
        let path = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || files.ensure_dir(path))
            .await
            .map_err(|error| Error::other(format!("directory task failed: {error}")))?
    }

    pub fn read(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        let (dir, relative) = self.resolve(path.as_ref())?;
        Ok(dir.read(relative)?)
    }

    pub fn read_external(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        Ok(std::fs::read(path)?)
    }

    pub async fn read_async(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        let files = self.clone();
        let path = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || files.read(path))
            .await
            .map_err(|error| Error::other(format!("read task failed: {error}")))?
    }

    pub async fn read_tail_async(&self, path: impl AsRef<Path>, from: u64) -> Result<Tail> {
        let files = self.clone();
        let path = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || {
            use std::io::{Read, Seek, SeekFrom};

            let mut file = files.open(&path)?;
            let len = file.metadata()?.len();
            let restarted = len < from;
            let start = if restarted { 0 } else { from };
            file.seek(SeekFrom::Start(start))?;
            let mut bytes = Vec::with_capacity(len.saturating_sub(start) as usize);
            file.read_to_end(&mut bytes)?;
            Ok(Tail {
                bytes,
                len,
                restarted,
            })
        })
        .await
        .map_err(|error| Error::other(format!("read task failed: {error}")))?
    }

    pub async fn read_external_async(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        Ok(tokio::fs::read(path).await?)
    }

    pub async fn read_string_async(&self, path: impl AsRef<Path>) -> Result<String> {
        let files = self.clone();
        let path = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || {
            let (dir, relative) = files.resolve(&path)?;
            Ok(dir.read_to_string(relative)?)
        })
        .await
        .map_err(|error| Error::other(format!("read task failed: {error}")))?
    }

    pub fn write_atomic(&self, path: impl AsRef<Path>, bytes: &[u8]) -> Result<()> {
        self.write_atomic_with(path.as_ref(), |file| file.write_all(bytes))?;
        Ok(())
    }

    pub async fn write_atomic_async(
        &self,
        path: impl AsRef<Path>,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        let files = self.clone();
        let path = path.as_ref().to_path_buf();
        let bytes = bytes.as_ref().to_vec();
        tokio::task::spawn_blocking(move || files.write_atomic(path, &bytes))
            .await
            .map_err(|error| Error::other(format!("atomic write task failed: {error}")))?
    }

    pub async fn copy_external_into(
        &self,
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<u64> {
        let files = self.clone();
        let source = source.as_ref().to_path_buf();
        let destination = destination.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || files.copy_external_into_sync(source, destination))
            .await
            .map_err(|error| Error::other(format!("copy task failed: {error}")))?
    }

    pub fn link_or_copy(
        &self,
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<()> {
        let source = source.as_ref();
        let destination = destination.as_ref();
        if source == destination {
            return Ok(());
        }

        let (source_dir, source_relative) = self.resolve(source)?;
        self.resolve(destination)?;
        if let Some(parent) = destination.parent() {
            self.ensure_dir(parent)?;
        }

        let temporary = temporary_path(destination);
        let hard_linked = match self.resolve_pair(&temporary, destination) {
            Ok((temporary_dir, temporary_relative, _))
                if Arc::ptr_eq(&source_dir, &temporary_dir) =>
            {
                temporary_dir.hard_link(source_relative, &temporary_dir, temporary_relative)
            }
            Ok(_) | Err(_) => Err(std::io::Error::from(std::io::ErrorKind::CrossesDevices)),
        };
        match hard_linked {
            Ok(()) => {
                let result = self.replace_staged(&temporary, destination);
                if result.is_err() {
                    let _ = self.remove_file_if_exists(&temporary);
                }
                result
            }
            Err(_) => {
                let mut source = self.open(source)?;
                self.write_atomic_with(destination, |target| {
                    std::io::copy(&mut source, target).map(|_| ())
                })
            }
        }
    }

    pub async fn link_or_copy_async(
        &self,
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<()> {
        let files = self.clone();
        let source = source.as_ref().to_path_buf();
        let destination = destination.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || files.link_or_copy(source, destination))
            .await
            .map_err(|error| Error::other(format!("link task failed: {error}")))?
    }

    pub fn copy_external_into_sync(
        &self,
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<u64> {
        let mut source = std::fs::File::open(source)?;
        let size = self.write_atomic_with(destination.as_ref(), |target| {
            std::io::copy(&mut source, target)
        })?;
        Ok(size)
    }

    pub fn exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        let (dir, relative) = self.resolve(path.as_ref())?;
        match dir.metadata(relative) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub fn is_file(&self, path: impl AsRef<Path>) -> Result<bool> {
        let (dir, relative) = self.resolve(path.as_ref())?;
        match dir.metadata(relative) {
            Ok(metadata) => Ok(metadata.is_file()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub fn is_external_file(&self, path: impl AsRef<Path>) -> bool {
        path.as_ref().is_file()
    }

    pub fn read_external_dir(&self, path: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
        Ok(std::fs::read_dir(path)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?)
    }

    pub fn external_symlink_metadata(&self, path: impl AsRef<Path>) -> Result<std::fs::Metadata> {
        Ok(std::fs::symlink_metadata(path)?)
    }

    pub fn open_external(&self, path: impl AsRef<Path>) -> Result<std::fs::File> {
        Ok(std::fs::File::open(path)?)
    }

    pub fn canonicalize_external(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        Ok(std::fs::canonicalize(path)?)
    }

    pub fn read_dir(&self, path: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
        let path = path.as_ref();
        let (dir, relative) = self.resolve(path)?;
        let entries = dir
            .read_dir(relative)?
            .map(|entry| entry.map(|entry| path.join(entry.file_name())))
            .collect::<std::io::Result<Vec<_>>>()?;
        Ok(entries)
    }

    pub fn metadata(&self, path: impl AsRef<Path>) -> Result<Metadata> {
        let (dir, relative) = self.resolve(path.as_ref())?;
        Ok(dir.metadata(relative)?)
    }

    pub fn symlink_metadata(&self, path: impl AsRef<Path>) -> Result<Metadata> {
        let (dir, relative) = self.resolve(path.as_ref())?;
        Ok(dir.symlink_metadata(relative)?)
    }

    pub fn open(&self, path: impl AsRef<Path>) -> Result<std::fs::File> {
        let (dir, relative) = self.resolve(path.as_ref())?;
        Ok(dir.open(relative)?.into_std())
    }

    pub fn create(&self, path: impl AsRef<Path>) -> Result<std::fs::File> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            self.ensure_dir(parent)?;
        }
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        let (dir, relative) = self.resolve(path)?;
        Ok(dir.open_with(relative, &options)?.into_std())
    }

    pub fn copy_reader_into_sync(
        &self,
        reader: &mut impl std::io::Read,
        destination: impl AsRef<Path>,
    ) -> Result<u64> {
        self.write_atomic_with(destination.as_ref(), |target| std::io::copy(reader, target))
    }

    pub fn rename(&self, source: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<()> {
        let source = source.as_ref();
        let destination_path = destination.as_ref();
        self.resolve_pair(source, destination_path)?;
        if let Some(parent) = destination_path.parent() {
            self.ensure_dir(parent)?;
        }
        let (dir, source, destination) = self.resolve_pair(source, destination_path)?;
        dir.rename(source, &dir, destination)?;
        Ok(())
    }

    fn remove_dir_all_if_exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        let (dir, path) = self.resolve(path.as_ref())?;
        if path == Path::new(".") {
            return Err(Error::other("refusing to remove a managed root"));
        }
        match dir.remove_dir_all(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub fn remove_managed_dir_all_if_exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        self.remove_dir_all_if_exists(path)
    }

    pub fn remove_instance_dir(&self, instance_id: &str) -> Result<bool> {
        let path = self
            .paths
            .instance_dir_checked(instance_id)
            .ok_or_else(|| Error::other("invalid instance id"))?;
        self.remove_dir_all_if_exists(path)
    }

    pub async fn open_download_part(
        &self,
        path: impl AsRef<Path>,
        append: bool,
    ) -> Result<tokio::fs::File> {
        let path = path.as_ref().to_path_buf();
        self.resolve(&path)?;
        if let Some(parent) = path.parent() {
            self.ensure_dir_async(parent).await?;
        }
        let mut options = OpenOptions::new();
        options.write(true).create(true);
        if append {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let (dir, relative) = self.resolve(&path)?;
        let file = dir.open_with(relative, &options)?.into_std();
        Ok(tokio::fs::File::from_std(file))
    }

    pub async fn commit_download_part(
        &self,
        partial: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<()> {
        let partial = partial.as_ref().to_path_buf();
        let destination = destination.as_ref().to_path_buf();
        self.resolve_pair(&partial, &destination)?;
        self.commit_staged(partial, destination).await
    }

    async fn commit_staged(&self, temporary: PathBuf, destination: PathBuf) -> Result<()> {
        let files = self.clone();
        tokio::task::spawn_blocking(move || files.replace_staged(&temporary, &destination))
            .await
            .map_err(|error| Error::other(format!("commit task failed: {error}")))?
    }

    pub fn remove_file_if_exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        let (dir, path) = self.resolve(path.as_ref())?;
        match dir.remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn remove_file_if_exists_async(&self, path: impl AsRef<Path>) -> Result<bool> {
        let files = self.clone();
        let path = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || files.remove_file_if_exists(path))
            .await
            .map_err(|error| Error::other(format!("remove task failed: {error}")))?
    }

    fn write_atomic_with<T>(
        &self,
        destination: &Path,
        write: impl FnOnce(&mut std::fs::File) -> std::io::Result<T>,
    ) -> Result<T> {
        self.resolve(destination)?;
        if let Some(parent) = destination.parent() {
            self.ensure_dir(parent)?;
        }
        let temporary = temporary_path(destination);
        let (dir, temporary_relative) = self.resolve(&temporary)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = dir.open_with(temporary_relative, &options)?.into_std();

        let result = (|| {
            let value = write(&mut file)?;
            file.sync_all()?;
            drop(file);
            self.replace_staged(&temporary, destination)?;
            Ok(value)
        })();
        if result.is_err() {
            let _ = self.remove_file_if_exists(&temporary);
        }
        result
    }

    fn replace_staged(&self, temporary: &Path, destination: &Path) -> Result<()> {
        let (dir, temporary, destination) = self.resolve_pair(temporary, destination)?;
        dir.rename(temporary, &dir, destination)?;
        #[cfg(unix)]
        {
            let parent = destination
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            dir.open(parent)?.into_std().sync_all()?;
        }
        Ok(())
    }
}

fn open_roots(paths: &Paths) -> Result<Roots> {
    let mut roots = Vec::new();
    for path in paths.capability_roots() {
        Dir::create_ambient_dir_all(&path, ambient_authority())?;
        let dir = Dir::open_ambient_dir(&path, ambient_authority())?;
        roots.push((path, Arc::new(dir)));
    }
    for path in paths.extra_roots() {
        match Dir::open_ambient_dir(&path, ambient_authority()) {
            Ok(dir) => roots.push((path, Arc::new(dir))),
            Err(error) => tracing::warn!(
                path = %path.display(),
                error = %error,
                "skipping a folder Basalt cannot reach right now"
            ),
        }
    }
    roots.sort_by_key(|(path, _)| std::cmp::Reverse(path.as_os_str().len()));
    Ok(Arc::new(roots))
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(format!(".{}.part", uuid::Uuid::new_v4()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::FileManager;
    use crate::paths::{DataRoot, Paths};

    fn relocated(name: &str, slots: &[(DataRoot, &str)]) -> (PathBuf, FileManager) {
        let base = std::env::temp_dir().join(format!("basalt-{name}-{}", uuid::Uuid::new_v4()));
        let root = base.join("data");
        let overrides = slots
            .iter()
            .map(|(slot, suffix)| (*slot, base.join(suffix)))
            .collect::<BTreeMap<_, _>>();
        for path in overrides.values() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        }
        let files = FileManager::new(Paths::relocated(root, overrides)).unwrap();
        (base, files)
    }

    #[test]
    fn rejects_paths_outside_the_managed_root() {
        let files =
            FileManager::new(Paths::plain(std::env::temp_dir().join("basalt-files-test"))).unwrap();
        assert!(files.read("/tmp/not-basalt").is_err());
        assert!(files
            .write_atomic(files.paths().root.join("../escape"), b"no")
            .is_err());
        assert!(files.remove_dir_all_if_exists(&files.paths().root).is_err());
    }

    #[test]
    fn atomic_write_creates_parents_and_replaces_content() {
        let root = std::env::temp_dir().join(format!("basalt-files-test-{}", uuid::Uuid::new_v4()));
        let files = FileManager::new(Paths::plain(root.clone())).unwrap();
        let path = root.join("nested").join("value");
        files.write_atomic(&path, b"first").unwrap();
        files.write_atomic(&path, b"second").unwrap();
        assert_eq!(files.read(&path).unwrap(), b"second");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn create_truncates_managed_files() {
        let root = std::env::temp_dir().join(format!("basalt-files-test-{}", uuid::Uuid::new_v4()));
        let files = FileManager::new(Paths::plain(root.clone())).unwrap();
        let path = root.join("logs").join("run.log");

        {
            let mut file = files.create(&path).unwrap();
            std::io::Write::write_all(&mut file, b"old log").unwrap();
        }
        drop(files.create(&path).unwrap());

        assert!(files.read(&path).unwrap().is_empty());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn links_managed_files_without_changing_their_contents() {
        let root = std::env::temp_dir().join(format!("basalt-link-test-{}", uuid::Uuid::new_v4()));
        let files = FileManager::new(Paths::plain(root.clone())).unwrap();
        let source = root.join("versions/1.21.1/1.21.1.jar");
        let destination = root.join("versions/neoforge/neoforge.jar");
        files.write_atomic(&source, b"minecraft").unwrap();
        files.write_atomic(&destination, b"stale").unwrap();

        files.link_or_copy(&source, &destination).unwrap();

        assert_eq!(files.read(&destination).unwrap(), b"minecraft");
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                std::fs::metadata(&source).unwrap().ino(),
                std::fs::metadata(&destination).unwrap().ino()
            );
        }
        std::fs::remove_dir_all(root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_escape_the_managed_root() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!("basalt-files-root-{}", uuid::Uuid::new_v4()));
        let outside =
            std::env::temp_dir().join(format!("basalt-files-outside-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&outside).unwrap();
        let files = FileManager::new(Paths::plain(root.clone())).unwrap();
        symlink(&outside, root.join("escape")).unwrap();

        assert!(files
            .write_atomic(root.join("escape/value"), b"no")
            .is_err());
        assert!(!outside.join("value").exists());

        std::fs::remove_dir_all(root).ok();
        std::fs::remove_dir_all(outside).ok();
    }

    #[test]
    fn a_relocated_root_is_writable_and_lands_on_its_own_disk() {
        let (base, files) = relocated("relocated", &[(DataRoot::Instances, "disk-two/games")]);
        let path = files.paths().instance_dir("atm10").join("options.txt");

        files.write_atomic(&path, b"fov:90").unwrap();

        assert!(path.starts_with(base.join("disk-two/games")));
        assert!(!path.starts_with(&files.paths().root));
        assert_eq!(files.read(&path).unwrap(), b"fov:90");
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn an_extra_root_is_writable_without_being_created() {
        let base = std::env::temp_dir().join(format!("basalt-extra-{}", uuid::Uuid::new_v4()));
        let outside = base.join("elsewhere/smp");
        std::fs::create_dir_all(&outside).unwrap();
        let files = FileManager::new(Paths::plain(base.join("data"))).unwrap();

        files.paths().adopt_extras(vec![outside.clone()]);
        files.reopen().unwrap();

        files
            .write_atomic(outside.join("eula.txt"), b"eula=true")
            .unwrap();
        assert_eq!(files.read(outside.join("eula.txt")).unwrap(), b"eula=true");
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn an_extra_root_that_is_gone_is_skipped_instead_of_fatal() {
        let base = std::env::temp_dir().join(format!("basalt-extra-gone-{}", uuid::Uuid::new_v4()));
        let missing = base.join("unplugged/smp");
        let paths = Paths::plain(base.join("data"));
        paths.adopt_extras(vec![missing.clone()]);

        let files = FileManager::new(paths).unwrap();

        assert!(!missing.exists());
        assert!(files.read(missing.join("server.properties")).is_err());
        assert!(files.paths().unavailable().is_empty());
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn paths_under_no_root_are_refused() {
        let (base, files) = relocated("stranger", &[(DataRoot::Versions, "disk-two/versions")]);

        assert!(files
            .read(base.join("disk-two/somewhere-else/file"))
            .is_err());
        assert!(files
            .write_atomic(base.join("disk-two/file"), b"no")
            .is_err());
        assert!(files.read("/etc/passwd").is_err());
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn traversal_out_of_a_relocated_root_is_refused() {
        let (base, files) = relocated("traversal", &[(DataRoot::Assets, "disk-two/assets")]);
        let escape = files.paths().assets().join("../../escape");

        assert!(files.write_atomic(&escape, b"no").is_err());
        assert!(files.ensure_dir(&escape).is_err());
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn nested_roots_resolve_against_the_longest_prefix() {
        let (base, files) = relocated(
            "nested",
            &[
                (DataRoot::Versions, "disk-two"),
                (DataRoot::Libraries, "disk-two/libraries"),
            ],
        );
        let library = files.paths().libraries().join("com/mojang/authlib.jar");
        files.write_atomic(&library, b"jar").unwrap();

        let (dir, relative) = files.resolve(&library).unwrap();
        assert_eq!(relative, std::path::Path::new("com/mojang/authlib.jar"));
        assert_eq!(dir.read(relative).unwrap(), b"jar");
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn reopening_moves_every_clone_onto_the_new_root() {
        let base = std::env::temp_dir().join(format!("basalt-reopen-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let paths = Paths::plain(base.join("data"));
        let files = FileManager::new(paths.clone()).unwrap();
        let clone = files.clone();
        let before = paths.instance_dir("atm10").join("options.txt");
        files.write_atomic(&before, b"fov:90").unwrap();

        let moved = base.join("disk-two/games");
        std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
        std::fs::rename(paths.instances(), &moved).unwrap();
        paths.adopt(BTreeMap::from([(DataRoot::Instances, moved.clone())]));
        files.reopen().unwrap();

        let after = paths.instance_dir("atm10").join("options.txt");
        assert!(after.starts_with(&moved));
        assert_eq!(clone.read(&after).unwrap(), b"fov:90");
        assert!(clone.read(&before).is_err());
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn an_unmounted_location_stops_startup_instead_of_recreating_it() {
        let base = std::env::temp_dir().join(format!("basalt-unmounted-{}", uuid::Uuid::new_v4()));
        let missing = base.join("never-mounted").join("games");
        let paths = Paths::relocated(
            base.join("data"),
            BTreeMap::from([(DataRoot::Instances, missing.clone())]),
        );

        assert!(FileManager::new(paths).is_err());
        assert!(!missing.exists());
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn moving_between_two_locations_is_refused() {
        let (base, files) = relocated("crossing", &[(DataRoot::Versions, "disk-two/versions")]);
        let source = files.paths().version_jar("1.21.1");
        let destination = files.paths().instance_dir("atm10").join("1.21.1.jar");
        files.write_atomic(&source, b"minecraft").unwrap();

        assert!(files.rename(&source, &destination).is_err());
        assert!(!destination.exists());

        files.link_or_copy(&source, &destination).unwrap();
        assert_eq!(files.read(&destination).unwrap(), b"minecraft");
        std::fs::remove_dir_all(base).ok();
    }
}
