use std::path::{Component, Path, PathBuf};

use crate::error::{Error, Result};
use crate::paths::Paths;

#[derive(Clone)]
pub struct FileManager {
    paths: Paths,
}

impl FileManager {
    pub fn new(paths: Paths) -> Self {
        Self { paths }
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

    pub async fn read_string_async(&self, path: impl AsRef<Path>) -> Result<String> {
        Ok(tokio::fs::read_to_string(self.managed(path.as_ref())?).await?)
    }

    pub fn write_atomic(&self, path: impl AsRef<Path>, bytes: &[u8]) -> Result<()> {
        let path = self.managed(path.as_ref())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = temporary_path(path);
        std::fs::write(&temporary, bytes)?;
        replace(&temporary, path)?;
        Ok(())
    }

    pub async fn write_atomic_async(
        &self,
        path: impl AsRef<Path>,
        bytes: impl AsRef<[u8]>,
    ) -> Result<()> {
        let path = self.managed(path.as_ref())?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let temporary = temporary_path(path);
        tokio::fs::write(&temporary, bytes).await?;
        if let Err(error) = tokio::fs::rename(&temporary, path).await {
            if error.kind() != std::io::ErrorKind::AlreadyExists {
                let _ = tokio::fs::remove_file(&temporary).await;
                return Err(error.into());
            }
            tokio::fs::remove_file(path).await?;
            tokio::fs::rename(&temporary, path).await?;
        }
        Ok(())
    }

    pub async fn copy_external_into(
        &self,
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<u64> {
        let bytes = tokio::fs::read(source).await?;
        let size = bytes.len() as u64;
        self.write_atomic_async(destination, bytes).await?;
        Ok(size)
    }

    pub fn copy_external_into_sync(
        &self,
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<u64> {
        let bytes = std::fs::read(source)?;
        let size = bytes.len() as u64;
        self.write_atomic(destination, &bytes)?;
        Ok(size)
    }

    pub fn exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        Ok(self.managed(path.as_ref())?.exists())
    }

    pub fn is_file(&self, path: impl AsRef<Path>) -> Result<bool> {
        Ok(self.managed(path.as_ref())?.is_file())
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

    pub fn temporary_for(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        Ok(temporary_path(self.managed(path.as_ref())?))
    }

    pub async fn commit_temporary(
        &self,
        temporary: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<()> {
        let temporary = self.managed(temporary.as_ref())?;
        let destination = self.managed(destination.as_ref())?;
        if let Err(error) = tokio::fs::rename(temporary, destination).await {
            if error.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(error.into());
            }
            tokio::fs::remove_file(destination).await?;
            tokio::fs::rename(temporary, destination).await?;
        }
        Ok(())
    }

    pub fn remove_file_if_exists(&self, path: impl AsRef<Path>) -> Result<bool> {
        let path = self.managed(path.as_ref())?;
        match std::fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn remove_file_if_exists_async(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<bool> {
        let path = self.managed(path.as_ref())?;
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
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

fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    match std::fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(destination)?;
            std::fs::rename(source, destination)
        }
        Err(error) => {
            let _ = std::fs::remove_file(source);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FileManager;
    use crate::paths::Paths;

    #[test]
    fn rejects_paths_outside_the_managed_root() {
        let files = FileManager::new(Paths {
            root: std::env::temp_dir().join("basalt-files-test"),
        });
        assert!(files.read("/tmp/not-basalt").is_err());
        assert!(files
            .write_atomic(files.paths().root.join("../escape"), b"no")
            .is_err());
    }

    #[test]
    fn atomic_write_creates_parents_and_replaces_content() {
        let root =
            std::env::temp_dir().join(format!("basalt-files-test-{}", uuid::Uuid::new_v4()));
        let files = FileManager::new(Paths { root: root.clone() });
        let path = root.join("nested").join("value");
        files.write_atomic(&path, b"first").unwrap();
        files.write_atomic(&path, b"second").unwrap();
        assert_eq!(files.read(&path).unwrap(), b"second");
        std::fs::remove_dir_all(root).ok();
    }
}
