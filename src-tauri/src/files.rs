use std::io::Write;
use std::path::{Component, Path, PathBuf};

use tokio::io::AsyncWriteExt;

use crate::error::{Error, Result};
use crate::paths::Paths;

#[derive(Clone)]
pub struct FileManager {
    paths: Paths,
    canonical_root: PathBuf,
}

impl FileManager {
    pub fn new(paths: Paths) -> Result<Self> {
        std::fs::create_dir_all(&paths.root)?;
        let canonical_root = paths.root.canonicalize()?;
        Ok(Self {
            paths,
            canonical_root,
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

    fn managed<'a>(&self, path: &'a Path) -> Result<&'a Path> {
        if !path.starts_with(&self.paths.root)
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(Error::other(format!(
                "refusing to access unmanaged path {}",
                path.display()
            )));
        }
        let existing = nearest_existing(path)?;
        if !existing.canonicalize()?.starts_with(&self.canonical_root) {
            return Err(Error::other(format!(
                "refusing to follow a managed path outside {}",
                self.paths.root.display()
            )));
        }
        Ok(path)
    }

    pub fn ensure_dir(&self, path: impl AsRef<Path>) -> Result<()> {
        std::fs::create_dir_all(self.managed(path.as_ref())?)?;
        Ok(())
    }

    pub async fn ensure_dir_async(&self, path: impl AsRef<Path>) -> Result<()> {
        tokio::fs::create_dir_all(self.managed(path.as_ref())?).await?;
        Ok(())
    }

    pub fn read(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        Ok(std::fs::read(self.managed(path.as_ref())?)?)
    }

    pub fn read_external(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        Ok(std::fs::read(path)?)
    }

    pub async fn read_async(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        Ok(tokio::fs::read(self.managed(path.as_ref())?).await?)
    }

    pub async fn read_external_async(&self, path: impl AsRef<Path>) -> Result<Vec<u8>> {
        Ok(tokio::fs::read(path).await?)
    }

    pub async fn read_string_async(&self, path: impl AsRef<Path>) -> Result<String> {
        Ok(tokio::fs::read_to_string(self.managed(path.as_ref())?).await?)
    }

    pub fn write_atomic(&self, path: impl AsRef<Path>, bytes: &[u8]) -> Result<()> {
        let path = self.managed(path.as_ref())?;
        if let Some(parent) = path.parent() {
            self.ensure_dir(parent)?;
        }
        let mut file = atomic_write_file::AtomicWriteFile::open(path)?;
        file.write_all(bytes)?;
        file.commit()?;
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
        let destination = self.managed(destination.as_ref())?;
        if let Some(parent) = destination.parent() {
            self.ensure_dir(parent)?;
        }
        let mut source = std::fs::File::open(source)?;
        let mut target = atomic_write_file::AtomicWriteFile::open(destination)?;
        let size = std::io::copy(&mut source, &mut target)?;
        target.commit()?;
        Ok(size)
    }

    pub fn exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        Ok(self.managed(path.as_ref())?.exists())
    }

    pub fn is_file(&self, path: impl AsRef<Path>) -> Result<bool> {
        Ok(self.managed(path.as_ref())?.is_file())
    }

    pub fn is_external_file(&self, path: impl AsRef<Path>) -> bool {
        path.as_ref().is_file()
    }

    pub fn read_dir(&self, path: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
        let entries = std::fs::read_dir(self.managed(path.as_ref())?)?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .collect();
        Ok(entries)
    }

    pub fn metadata(&self, path: impl AsRef<Path>) -> Result<std::fs::Metadata> {
        Ok(std::fs::metadata(self.managed(path.as_ref())?)?)
    }

    pub fn open(&self, path: impl AsRef<Path>) -> Result<std::fs::File> {
        Ok(std::fs::File::open(self.managed(path.as_ref())?)?)
    }

    pub fn rename(&self, source: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<()> {
        let source = self.managed(source.as_ref())?;
        let destination = self.managed(destination.as_ref())?;
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(source, destination)?;
        Ok(())
    }

    pub fn remove_dir_all_if_exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = self.managed(path.as_ref())?;
        match std::fs::remove_dir_all(path) {
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
        let destination = self.managed(destination.as_ref())?.to_path_buf();
        if let Some(parent) = destination.parent() {
            self.ensure_dir_async(parent).await?;
        }
        let temporary = temporary_path(&destination);
        let file = tokio::fs::File::create(&temporary).await?;
        Ok(StagedFile {
            files: self.clone(),
            destination,
            temporary: Some(temporary),
            file: Some(file),
        })
    }

    async fn commit_staged(&self, temporary: PathBuf, destination: PathBuf) -> Result<()> {
        let files = self.clone();
        tokio::task::spawn_blocking(move || {
            files.copy_external_into_sync(&temporary, destination)?;
            files.remove_file_if_exists(temporary)?;
            Ok(())
        })
        .await
        .map_err(|error| Error::other(format!("commit task failed: {error}")))?
    }

    pub fn remove_file_if_exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = self.managed(path.as_ref())?;
        match std::fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn remove_file_if_exists_async(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = self.managed(path.as_ref())?;
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
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
            let _ = self.files.remove_file_if_exists(path);
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

fn nearest_existing(path: &Path) -> Result<&Path> {
    let mut candidate = path;
    loop {
        match std::fs::symlink_metadata(candidate) {
            Ok(_) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                candidate = candidate
                    .parent()
                    .ok_or_else(|| Error::other("managed path has no existing ancestor"))?;
            }
            Err(error) => return Err(error.into()),
        }
    }
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
        assert!(!temporary.exists());
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
