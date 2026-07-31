use std::{
    io::Write,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use crate::{
    error::{Error, Result},
    paths::Paths,
};
use cap_std::{
    ambient_authority,
    fs::{Dir, Metadata, OpenOptions},
};

#[derive(Clone)]
pub struct FileManager {
    paths: Paths,
    root: Arc<Dir>,
}

impl FileManager {
    pub fn new(paths: Paths) -> Result<Self> {
        Dir::create_ambient_dir_all(&paths.root, ambient_authority())?;
        let root = Dir::open_ambient_dir(&paths.root, ambient_authority())?;
        Ok(Self {
            paths,
            root: Arc::new(root),
        })
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
            self.paths.logs(),
            self.paths.skins(),
        ] {
            self.ensure_dir(directory)?;
        }
        Ok(())
    }

    fn relative<'a>(&self, path: &'a Path) -> Result<&'a Path> {
        let relative = path.strip_prefix(&self.paths.root).map_err(|_| {
            Error::other(format!(
                "refusing to access unmanaged path {}",
                path.display()
            ))
        })?;
        if relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(Error::other(format!(
                "refusing to access unmanaged path {}",
                path.display()
            )));
        }
        if relative.as_os_str().is_empty() {
            return Ok(Path::new("."));
        }
        Ok(relative)
    }

    pub fn ensure_dir(&self, path: impl AsRef<Path>) -> Result<()> {
        self.root.create_dir_all(self.relative(path.as_ref())?)?;
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
        Ok(self.root.read(self.relative(path.as_ref())?)?)
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

    pub async fn read_external_async(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        Ok(tokio::fs::read(path).await?)
    }

    pub async fn read_string_async(&self, path: impl AsRef<Path>) -> Result<String> {
        let files = self.clone();
        let path = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || {
            Ok(files.root.read_to_string(files.relative(&path)?)?)
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

        let source_relative = self.relative(source)?;
        self.relative(destination)?;
        if let Some(parent) = destination.parent() {
            self.ensure_dir(parent)?;
        }

        let temporary = temporary_path(destination);
        let temporary_relative = self.relative(&temporary)?;
        match self
            .root
            .hard_link(source_relative, &self.root, temporary_relative)
        {
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
        match self.root.metadata(self.relative(path.as_ref())?) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub fn is_file(&self, path: impl AsRef<Path>) -> Result<bool> {
        match self.root.metadata(self.relative(path.as_ref())?) {
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
        let entries = self
            .root
            .read_dir(self.relative(path)?)?
            .map(|entry| entry.map(|entry| path.join(entry.file_name())))
            .collect::<std::io::Result<Vec<_>>>()?;
        Ok(entries)
    }

    pub fn metadata(&self, path: impl AsRef<Path>) -> Result<Metadata> {
        Ok(self.root.metadata(self.relative(path.as_ref())?)?)
    }

    pub fn symlink_metadata(&self, path: impl AsRef<Path>) -> Result<Metadata> {
        Ok(self.root.symlink_metadata(self.relative(path.as_ref())?)?)
    }

    pub fn open(&self, path: impl AsRef<Path>) -> Result<std::fs::File> {
        Ok(self.root.open(self.relative(path.as_ref())?)?.into_std())
    }

    pub fn create(&self, path: impl AsRef<Path>) -> Result<std::fs::File> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            self.ensure_dir(parent)?;
        }
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        Ok(self
            .root
            .open_with(self.relative(path)?, &options)?
            .into_std())
    }

    pub fn copy_reader_into_sync(
        &self,
        reader: &mut impl std::io::Read,
        destination: impl AsRef<Path>,
    ) -> Result<u64> {
        self.write_atomic_with(destination.as_ref(), |target| std::io::copy(reader, target))
    }

    pub fn rename(&self, source: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<()> {
        let source = self.relative(source.as_ref())?;
        let destination_path = destination.as_ref();
        if let Some(parent) = destination_path.parent() {
            self.ensure_dir(parent)?;
        }
        let destination = self.relative(destination_path)?;
        self.root.rename(source, &self.root, destination)?;
        Ok(())
    }

    fn remove_dir_all_if_exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = self.relative(path.as_ref())?;
        if path == Path::new(".") {
            return Err(Error::other("refusing to remove the managed root"));
        }
        match self.root.remove_dir_all(path) {
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
        self.relative(&path)?;
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
        let file = self
            .root
            .open_with(self.relative(&path)?, &options)?
            .into_std();
        Ok(tokio::fs::File::from_std(file))
    }

    pub async fn commit_download_part(
        &self,
        partial: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<()> {
        let partial = partial.as_ref().to_path_buf();
        let destination = destination.as_ref().to_path_buf();
        self.relative(&partial)?;
        self.relative(&destination)?;
        self.commit_staged(partial, destination).await
    }

    async fn commit_staged(&self, temporary: PathBuf, destination: PathBuf) -> Result<()> {
        let files = self.clone();
        tokio::task::spawn_blocking(move || files.replace_staged(&temporary, &destination))
            .await
            .map_err(|error| Error::other(format!("commit task failed: {error}")))?
    }

    pub fn remove_file_if_exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = self.relative(path.as_ref())?;
        match self.root.remove_file(path) {
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
        self.relative(destination)?;
        if let Some(parent) = destination.parent() {
            self.ensure_dir(parent)?;
        }
        let temporary = temporary_path(destination);
        let temporary_relative = self.relative(&temporary)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = self
            .root
            .open_with(temporary_relative, &options)?
            .into_std();

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
        let temporary = self.relative(temporary)?;
        let destination = self.relative(destination)?;
        self.root.rename(temporary, &self.root, destination)?;
        #[cfg(unix)]
        {
            let parent = destination
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            self.root.open(parent)?.into_std().sync_all()?;
        }
        Ok(())
    }
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
    use super::FileManager;
    use crate::paths::Paths;

    #[test]
    fn rejects_paths_outside_the_managed_root() {
        let files = FileManager::new(Paths {
            root: std::env::temp_dir().join("basalt-files-test"),
        })
        .unwrap();
        assert!(files.read("/tmp/not-basalt").is_err());
        assert!(files
            .write_atomic(files.paths().root.join("../escape"), b"no")
            .is_err());
        assert!(files.remove_dir_all_if_exists(&files.paths().root).is_err());
    }

    #[test]
    fn atomic_write_creates_parents_and_replaces_content() {
        let root = std::env::temp_dir().join(format!("basalt-files-test-{}", uuid::Uuid::new_v4()));
        let files = FileManager::new(Paths { root: root.clone() }).unwrap();
        let path = root.join("nested").join("value");
        files.write_atomic(&path, b"first").unwrap();
        files.write_atomic(&path, b"second").unwrap();
        assert_eq!(files.read(&path).unwrap(), b"second");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn create_truncates_managed_files() {
        let root = std::env::temp_dir().join(format!("basalt-files-test-{}", uuid::Uuid::new_v4()));
        let files = FileManager::new(Paths { root: root.clone() }).unwrap();
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
        let files = FileManager::new(Paths { root: root.clone() }).unwrap();
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
        let files = FileManager::new(Paths { root: root.clone() }).unwrap();
        symlink(&outside, root.join("escape")).unwrap();

        assert!(files
            .write_atomic(root.join("escape/value"), b"no")
            .is_err());
        assert!(!outside.join("value").exists());

        std::fs::remove_dir_all(root).ok();
        std::fs::remove_dir_all(outside).ok();
    }
}
