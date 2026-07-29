use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use cap_std::ambient_authority;
use cap_std::fs::{Dir, Metadata, OpenOptions};
use tokio::io::AsyncWriteExt;

use crate::error::{Error, Result};
use crate::paths::Paths;

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
            Error::other(format!("refusing to access unmanaged path {}", path.display()))
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
            Ok(files
                .root
                .read_to_string(files.relative(&path)?)?)
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

    pub fn read_dir(&self, path: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
        let path = path.as_ref();
        let entries = self
            .root
            .read_dir(self.relative(path)?)?
            .filter_map(|entry| entry.ok().map(|entry| path.join(entry.file_name())))
            .collect();
        Ok(entries)
    }

    pub fn metadata(&self, path: impl AsRef<Path>) -> Result<Metadata> {
        Ok(self.root.metadata(self.relative(path.as_ref())?)?)
    }

    pub fn open(&self, path: impl AsRef<Path>) -> Result<std::fs::File> {
        Ok(self.root.open(self.relative(path.as_ref())?)?.into_std())
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

    pub fn remove_dir_all_if_exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = self.relative(path.as_ref())?;
        match self.root.remove_dir_all(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub fn remove_instance_dir(&self, instance_id: &str) -> Result<bool> {
        let path = self
            .paths
            .instance_dir_checked(instance_id)
            .ok_or_else(|| Error::other("invalid instance id"))?;
        self.remove_dir_all_if_exists(path)
    }

    pub async fn begin_staged_write(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<StagedFile> {
        let destination = destination.as_ref().to_path_buf();
        self.relative(&destination)?;
        if let Some(parent) = destination.parent() {
            self.ensure_dir_async(parent).await?;
        }
        let temporary = temporary_path(&destination);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let file = self
            .root
            .open_with(self.relative(&temporary)?, &options)?
            .into_std();
        let file = tokio::fs::File::from_std(file);
        Ok(StagedFile {
            files: self.clone(),
            destination,
            temporary: Some(temporary),
            file: Some(file),
        })
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

pub struct StagedFile {
    files: FileManager,
    destination: PathBuf,
    temporary: Option<PathBuf>,
    file: Option<tokio::fs::File>,
}

impl StagedFile {
    pub fn writer(&mut self) -> &mut tokio::fs::File {
        self.file
            .as_mut()
            .expect("staged file writer is unavailable after commit")
    }

    pub async fn commit(mut self) -> Result<()> {
        let mut file = self
            .file
            .take()
            .expect("staged file writer is unavailable after commit");
        file.flush().await?;
        file.sync_all().await?;
        drop(file);

        let temporary = self
            .temporary
            .as_ref()
            .expect("staged file path is unavailable after commit")
            .clone();
        self.files
            .commit_staged(temporary, self.destination.clone())
            .await?;
        self.temporary = None;
        Ok(())
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        drop(self.file.take());
        if let Some(path) = self.temporary.take() {
            let files = self.files.clone();
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn_blocking(move || cleanup_staged_file(&files, &path));
            } else {
                cleanup_staged_file(&files, &path);
            }
        }
    }
}

fn cleanup_staged_file(files: &FileManager, path: &Path) {
    for attempt in 0..5 {
        if files.remove_file_if_exists(path).is_ok() {
            return;
        }
        if attempt < 4 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
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
    use tokio::io::AsyncWriteExt;

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

    #[tokio::test]
    async fn abandoned_staged_write_preserves_destination_and_removes_partial() {
        let root =
            std::env::temp_dir().join(format!("basalt-staged-test-{}", uuid::Uuid::new_v4()));
        let files = FileManager::new(Paths { root: root.clone() }).unwrap();
        let path = root.join("value");
        files.write_atomic(&path, b"original").unwrap();

        let mut staged = files.begin_staged_write(&path).await.unwrap();
        let temporary = staged.temporary.as_ref().unwrap().clone();
        staged.writer().write_all(b"replacement").await.unwrap();
        drop(staged);

        assert_eq!(files.read(&path).unwrap(), b"original");
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while files.exists(&temporary).unwrap() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
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
