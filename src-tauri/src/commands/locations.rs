use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::{
    error::{Error, Result},
    paths::{self, DataRoot},
    state::AppState,
    sysinfo_probe::{self, DiskInfo},
    tasks::{TaskKind, TaskSpec},
};

#[derive(Serialize)]
pub struct DataLocation {
    pub slot: DataRoot,
    pub label: &'static str,
    pub summary: &'static str,
    pub path: String,
    pub default_path: String,
    pub custom: bool,
    pub exists: bool,
    pub disk: Option<DiskInfo>,
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn get_data_locations(state: State<AppState>) -> Result<Vec<DataLocation>> {
    let paths = state.files.paths();
    Ok(DataRoot::ALL
        .into_iter()
        .map(|slot| {
            let path = paths.located_at(slot);
            let default_path = paths.default_for(slot);
            DataLocation {
                slot,
                label: slot.label(),
                summary: slot.summary(),
                custom: path != default_path,
                exists: path.is_dir(),
                disk: sysinfo_probe::disk_for(&path),
                path: path.display().to_string(),
                default_path: default_path.display().to_string(),
            }
        })
        .collect())
}

#[derive(Serialize)]
pub struct LocationCandidate {
    pub path: String,
    pub usable: bool,
    pub problem: Option<String>,
    pub occupied: bool,
    pub disk: Option<DiskInfo>,
}

#[tauri::command]
#[tracing::instrument(skip_all, err)]
pub fn inspect_data_location(
    state: State<AppState>,
    slot: DataRoot,
    path: String,
) -> Result<LocationCandidate> {
    let target = PathBuf::from(path.trim());
    let problem = validate(state.files.paths(), slot, &target).err();
    Ok(LocationCandidate {
        occupied: target
            .read_dir()
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false),
        disk: sysinfo_probe::disk_for(&target),
        usable: problem.is_none(),
        problem: problem.map(|error| error.to_string()),
        path: target.display().to_string(),
    })
}

#[tauri::command]
#[tracing::instrument(skip(app, state), err)]
pub async fn set_data_location(
    app: AppHandle,
    state: State<'_, AppState>,
    slot: DataRoot,
    path: Option<String>,
    move_existing: bool,
) -> Result<()> {
    if !state.running.lock().unwrap().is_empty() {
        return Err(Error::other("Close the game before moving data folders."));
    }
    if state
        .tasks
        .list()
        .iter()
        .any(|task| !task.state.is_finished())
    {
        return Err(Error::other(
            "Wait for the downloads to finish before moving data folders.",
        ));
    }

    let paths = state.files.paths().clone();
    let current = paths.located_at(slot);
    let target = match path.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        Some(value) => PathBuf::from(value),
        None => paths.default_for(slot),
    };
    if target == current {
        return Ok(());
    }
    validate(&paths, slot, &target)?;

    if move_existing && current.is_dir() {
        let task = state.tasks.start(
            &app,
            TaskKind::DataMove,
            TaskSpec {
                title: format!("Moving {}", slot.label().to_lowercase()),
                subtitle: Some(target.display().to_string()),
                ..Default::default()
            },
        )?;
        let source = current.clone();
        let destination = target.clone();
        let result = tauri::async_runtime::spawn_blocking(move || relocate(&source, &destination))
            .await
            .map_err(|error| Error::other(format!("move task failed: {error}")))?;
        task.finish(&result);
        result?;
    } else {
        std::fs::create_dir_all(&target)?;
    }

    let mut overrides = paths.overrides();
    if target == paths.default_for(slot) {
        overrides.remove(&slot);
    } else {
        overrides.insert(slot, target);
    }
    paths::write_overrides(&paths.root, &overrides)?;
    paths.adopt(overrides);
    state.files.reopen()?;
    state.files.ensure_base_dirs()?;
    let _ = app.emit("data:relocated", slot);
    Ok(())
}

fn validate(paths: &crate::paths::Paths, slot: DataRoot, target: &Path) -> Result<()> {
    if target.as_os_str().is_empty() {
        return Err(Error::other("Pick a folder first."));
    }
    if !target.is_absolute() {
        return Err(Error::other("Pick a full path, not a relative one."));
    }
    if target.exists() && !target.is_dir() {
        return Err(Error::other("That path is a file, not a folder."));
    }
    if !paths::parent_exists(target) {
        return Err(Error::other(
            "The drive holding that folder is not mounted right now.",
        ));
    }
    if target.starts_with(&paths.root) && target != paths.default_for(slot) {
        return Err(Error::other(
            "Pick a folder outside the Basalt data directory.",
        ));
    }
    for other in DataRoot::ALL.into_iter().filter(|other| *other != slot) {
        let taken = paths.located_at(other);
        if target == taken {
            return Err(Error::other(format!(
                "{} already lives there.",
                other.label()
            )));
        }
    }
    Ok(())
}

fn relocate(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() && destination.read_dir()?.next().is_some() {
        return Err(Error::other("That folder already has something in it."));
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::rename(source, destination) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {}
        Err(error) if error.raw_os_error() == Some(18) => {}
        Err(error) => return Err(error.into()),
    }
    copy_tree(source, destination)?;
    std::fs::remove_dir_all(source)?;
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&from, &to)?;
        } else if kind.is_symlink() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(std::fs::read_link(&from)?, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::paths::Paths;

    fn scratch(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("basalt-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn a_relocated_slot_must_be_absolute_mounted_and_unclaimed() {
        let base = scratch("validate");
        let paths = Paths::relocated(
            base.join("data"),
            BTreeMap::from([(DataRoot::Versions, base.join("disk-two/versions"))]),
        );

        assert!(validate(&paths, DataRoot::Instances, Path::new("games")).is_err());
        assert!(validate(
            &paths,
            DataRoot::Instances,
            &base.join("never/mounted/games")
        )
        .is_err());
        assert!(validate(&paths, DataRoot::Instances, &paths.root.join("instances-2")).is_err());
        assert!(validate(&paths, DataRoot::Instances, &base.join("disk-two/versions")).is_err());
        assert!(validate(&paths, DataRoot::Instances, &base.join("games")).is_ok());

        std::fs::write(base.join("occupied"), b"file").unwrap();
        assert!(validate(&paths, DataRoot::Instances, &base.join("occupied")).is_err());
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn the_default_location_stays_valid_for_its_own_slot() {
        let base = scratch("default");
        let paths = Paths::plain(base.join("data"));
        std::fs::create_dir_all(&paths.root).unwrap();

        assert!(validate(
            &paths,
            DataRoot::Instances,
            &paths.default_for(DataRoot::Instances)
        )
        .is_ok());
        assert!(validate(
            &paths,
            DataRoot::Instances,
            &paths.default_for(DataRoot::Versions)
        )
        .is_err());
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn a_move_refuses_to_write_into_an_occupied_folder() {
        let base = scratch("occupied-move");
        let source = base.join("from");
        let destination = base.join("to");
        std::fs::create_dir_all(source.join("instance")).unwrap();
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(destination.join("stranger"), b"keep me").unwrap();

        assert!(relocate(&source, &destination).is_err());
        assert!(source.join("instance").is_dir());
        assert_eq!(
            std::fs::read(destination.join("stranger")).unwrap(),
            b"keep me"
        );
        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn copying_across_drives_keeps_the_whole_tree() {
        let base = scratch("copy-tree");
        let source = base.join("from");
        let destination = base.join("to");
        std::fs::create_dir_all(source.join("atm10/mods")).unwrap();
        std::fs::write(source.join("atm10/mods/jei.jar"), b"jar").unwrap();
        std::fs::write(source.join("atm10/options.txt"), b"fov:90").unwrap();

        copy_tree(&source, &destination).unwrap();

        assert_eq!(
            std::fs::read(destination.join("atm10/mods/jei.jar")).unwrap(),
            b"jar"
        );
        assert_eq!(
            std::fs::read(destination.join("atm10/options.txt")).unwrap(),
            b"fov:90"
        );
        std::fs::remove_dir_all(base).ok();
    }
}
