pub mod meta;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    config::Instance,
    db::Db,
    error::{Error, Result},
    files::FileManager,
    paths::Paths,
};

use meta::Compatibility;

const DISABLED_SUFFIX: &str = ".disabled";
const MAX_FOLDER_ENTRIES: usize = 20_000;

#[derive(Debug, Clone, Serialize)]
pub struct Datapack {
    pub file_name: String,
    pub enabled: bool,
    pub off_in_game: bool,
    pub directory: bool,
    pub size: u64,
    pub title: Option<String>,
    pub min_format: Option<u32>,
    pub max_format: Option<u32>,
    pub compatibility: Compatibility,
    pub provider: Option<String>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub icon_url: Option<String>,
    pub latest_version_id: Option<String>,
    pub latest_file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldPacks {
    pub world: String,
    pub display_name: String,
    pub loose: bool,
    pub packs: Vec<Datapack>,
}

pub fn world_datapacks_dir(paths: &Paths, instance_id: &str, world: &str) -> Result<PathBuf> {
    if world.trim().is_empty()
        || world.contains('/')
        || world.contains('\\')
        || world.contains("..")
    {
        return Err(Error::other(format!("not a world folder: {world}")));
    }
    let saves = paths
        .instance_saves_dir_checked(instance_id)
        .ok_or_else(|| Error::other("invalid instance id"))?;
    let dir = saves.join(world);
    if dir.parent() != Some(saves.as_path()) {
        return Err(Error::other(format!("not a world folder: {world}")));
    }
    Ok(dir.join("datapacks"))
}

fn loose_datapacks_dir(paths: &Paths, instance_id: &str) -> Result<PathBuf> {
    Ok(paths
        .instance_dir_checked(instance_id)
        .ok_or_else(|| Error::other("invalid instance id"))?
        .join("datapacks"))
}

fn target_dir(paths: &Paths, instance_id: &str, world: &str) -> Result<PathBuf> {
    if world.is_empty() {
        loose_datapacks_dir(paths, instance_id)
    } else {
        world_datapacks_dir(paths, instance_id, world)
    }
}

fn validate_file_name(file_name: &str) -> Result<()> {
    if file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || file_name.contains("..")
    {
        return Err(Error::other("invalid file name"));
    }
    Ok(())
}

fn folder_size(files: &FileManager, root: &Path) -> u64 {
    let mut pending = vec![root.to_path_buf()];
    let mut seen = 0usize;
    let mut bytes = 0u64;

    while let Some(directory) = pending.pop() {
        let Ok(entries) = files.read_dir(&directory) else {
            continue;
        };
        for path in entries {
            seen += 1;
            if seen > MAX_FOLDER_ENTRIES {
                return bytes;
            }
            let Ok(metadata) = files.symlink_metadata(&path) else {
                continue;
            };
            if metadata.is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                bytes += metadata.len();
            }
        }
    }

    bytes
}

fn read_dir_packs(
    files: &FileManager,
    db: &Db,
    instance: &Instance,
    world: &str,
    dir: &Path,
    off_in_game: &HashSet<String>,
    expected: Option<u32>,
) -> Vec<Datapack> {
    let Ok(entries) = files.read_dir(dir) else {
        return Vec::new();
    };
    let recorded = db.world_datapacks(&instance.id, world).unwrap_or_default();
    let mut packs = Vec::new();

    for path in entries {
        let Ok(metadata) = files.symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_symlink() {
            continue;
        }
        let directory = metadata.is_dir();
        if !directory && !metadata.is_file() {
            continue;
        }
        let Some(raw) = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        if raw == "pack.mcmeta" {
            continue;
        }
        let enabled = !raw.ends_with(DISABLED_SUFFIX);
        let file_name = raw
            .strip_suffix(DISABLED_SUFFIX)
            .unwrap_or(&raw)
            .to_string();
        if !directory && !file_name.to_ascii_lowercase().ends_with(".zip") {
            continue;
        }

        let found = meta::read(files, &path, directory);
        let source = recorded.iter().find(|row| row.file_name == file_name);

        packs.push(Datapack {
            compatibility: found.against(expected),
            title: found
                .description
                .clone()
                .or_else(|| source.and_then(|row| row.title.clone())),
            min_format: found.min_format,
            max_format: found.max_format,
            size: if directory {
                folder_size(files, &path)
            } else {
                metadata.len()
            },
            off_in_game: enabled && off_in_game.contains(&file_name),
            provider: source.and_then(|row| row.provider.clone()),
            project_id: source.and_then(|row| row.project_id.clone()),
            version_id: source.and_then(|row| row.version_id.clone()),
            icon_url: source.and_then(|row| row.icon_url.clone()),
            latest_version_id: source.and_then(|row| row.latest_version_id.clone()),
            latest_file_name: source.and_then(|row| row.latest_file_name.clone()),
            file_name,
            enabled,
            directory,
        });
    }

    packs.sort_by_key(|pack| pack.file_name.to_lowercase());
    packs
}

pub fn list(files: &FileManager, db: &Db, instance: &Instance) -> Result<Vec<WorldPacks>> {
    let paths = files.paths();
    let expected = meta::expected_format(files, paths, instance);
    let mut groups = Vec::new();

    for world in crate::worlds::list(files, &instance.id)? {
        let dir = world_datapacks_dir(paths, &instance.id, &world.folder_name)?;
        if !files.exists(&dir).unwrap_or(false) {
            continue;
        }
        let world_dir = dir.parent().map(Path::to_path_buf).unwrap_or_default();
        let (_, disabled) = crate::worlds::pack_state(files, &world_dir);
        let off_in_game: HashSet<String> = disabled
            .iter()
            .filter_map(|entry| entry.strip_prefix("file/"))
            .map(str::to_string)
            .collect();

        let packs = read_dir_packs(
            files,
            db,
            instance,
            &world.folder_name,
            &dir,
            &off_in_game,
            expected,
        );
        if packs.is_empty() {
            continue;
        }
        groups.push(WorldPacks {
            world: world.folder_name,
            display_name: world.name,
            loose: false,
            packs,
        });
    }

    let loose = loose_datapacks_dir(paths, &instance.id)?;
    if files.exists(&loose).unwrap_or(false) {
        let packs = read_dir_packs(files, db, instance, "", &loose, &HashSet::new(), expected);
        if !packs.is_empty() {
            groups.push(WorldPacks {
                world: String::new(),
                display_name: "Not in a world".to_string(),
                loose: true,
                packs,
            });
        }
    }

    Ok(groups)
}

pub fn toggle(
    files: &FileManager,
    instance_id: &str,
    world: &str,
    file_name: &str,
) -> Result<bool> {
    validate_file_name(file_name)?;
    let dir = target_dir(files.paths(), instance_id, world)?;
    let enabled = dir.join(file_name);
    let disabled = dir.join(format!("{file_name}{DISABLED_SUFFIX}"));

    if files.exists(&enabled)? {
        files.rename(&enabled, &disabled)?;
        Ok(false)
    } else if files.exists(&disabled)? {
        files.rename(&disabled, &enabled)?;
        Ok(true)
    } else {
        Err(Error::NotFound(format!("datapack {file_name}")))
    }
}

pub fn delete(files: &FileManager, instance_id: &str, world: &str, file_name: &str) -> Result<()> {
    validate_file_name(file_name)?;
    let dir = target_dir(files.paths(), instance_id, world)?;
    for candidate in [
        dir.join(file_name),
        dir.join(format!("{file_name}{DISABLED_SUFFIX}")),
    ] {
        let Ok(metadata) = files.symlink_metadata(&candidate) else {
            continue;
        };
        if metadata.is_dir() {
            files.remove_managed_dir_all_if_exists(&candidate)?;
        } else {
            files.remove_file_if_exists(&candidate)?;
        }
    }
    Ok(())
}

pub fn add(
    files: &FileManager,
    instance_id: &str,
    world: &str,
    sources: &[String],
) -> Result<usize> {
    let dir = target_dir(files.paths(), instance_id, world)?;
    files.ensure_dir(&dir)?;
    let mut added = 0;

    for source in sources {
        let path = PathBuf::from(source);
        let Some(name) = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
        else {
            continue;
        };
        validate_file_name(&name)?;
        let metadata = files.external_symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(Error::other(format!(
                "{name} is not a file Basalt can copy"
            )));
        }
        let mut reader = files.open_external(&path)?;
        files.copy_reader_into_sync(&mut reader, dir.join(&name))?;
        added += 1;
    }

    Ok(added)
}

pub async fn install(
    app: &tauri::AppHandle,
    state: &crate::state::AppState,
    provider: crate::search::Provider,
    project_id: &str,
    instance: &Instance,
    world: &str,
    version_id: Option<&str>,
) -> Result<Vec<String>> {
    use crate::search::{model::ContentKind, resolve};
    use crate::tasks::{TaskKind, TaskSpec};

    let plan = resolve::plan(
        state,
        provider,
        project_id,
        &instance.id,
        ContentKind::DataPack,
        &instance.version_id,
        None,
        version_id,
        false,
    )
    .await?;

    let Some(file) = plan.primary.clone() else {
        return Ok(Vec::new());
    };

    let dir = target_dir(&state.paths, &instance.id, world)?;
    state.files.ensure_dir_async(&dir).await?;

    let task = state.tasks.start(
        app,
        TaskKind::DatapackInstall,
        TaskSpec {
            title: file.title.clone(),
            subtitle: Some(if world.is_empty() {
                instance.name.clone()
            } else {
                format!("{} · {}", instance.name, world)
            }),
            icon_url: file.icon_url.clone(),
            instance_id: Some(instance.id.clone()),
            project_id: Some(project_id.to_string()),
            total: 1,
            total_bytes: file.size.unwrap_or(0),
        },
    )?;

    let spec = crate::download::DownloadSpec {
        url: file.url.clone(),
        dest: dir.join(&file.file_name),
        sha1: file.sha1.clone(),
        sha256: None,
        size: file.size,
    };

    let outcome = crate::download::download_many_cancellable(
        &state.network,
        &state.files,
        vec![spec],
        1,
        |progress| {
            task.progress(
                progress.completed as u64,
                progress.total as u64,
                progress.downloaded_bytes,
                progress.total_bytes,
            );
        },
        Some(task.token()),
        Some(task.written()),
        None,
    )
    .await;

    if let Err(error) = outcome {
        resolve::rollback_written(&state.files, &task);
        task.finish::<()>(&Err(error));
        return Err(Error::other("The datapack could not be downloaded."));
    }

    state.db.record_world_datapack(
        &instance.id,
        world,
        &crate::db::DatapackRecord {
            file_name: file.file_name.clone(),
            sha1: file.sha1.clone(),
            provider: Some(provider.as_str().to_string()),
            project_id: Some(file.project_id.clone()),
            version_id: Some(file.version_id.clone()),
            title: Some(file.title.clone()),
            icon_url: file.icon_url.clone(),
            installed_at: chrono::Utc::now().timestamp(),
            latest_version_id: None,
            latest_file_name: None,
        },
    )?;

    task.finish::<()>(&Ok(()));
    Ok(vec![file.file_name])
}

pub async fn check_updates(state: &crate::state::AppState, instance: &Instance) -> Result<usize> {
    use crate::search::{self, model::ContentKind};

    let recorded = state.db.all_world_datapacks(&instance.id)?;
    let mut found = 0;

    let hashes: Vec<String> = recorded
        .iter()
        .filter(|(_, row)| row.provider.as_deref() == Some("modrinth"))
        .filter_map(|(_, row)| row.sha1.clone())
        .collect();

    let latest = if hashes.is_empty() {
        Default::default()
    } else {
        search::modrinth::latest_versions_by_hash(
            state,
            &hashes,
            &[],
            std::slice::from_ref(&instance.version_id),
        )
        .await
        .unwrap_or_default()
    };

    for (world, row) in &recorded {
        let newer = match row.provider.as_deref() {
            Some("modrinth") => row
                .sha1
                .as_ref()
                .and_then(|sha1| latest.get(sha1))
                .filter(|version| Some(&version.id) != row.version_id.as_ref())
                .and_then(|version| {
                    let file = version
                        .files
                        .iter()
                        .find(|entry| entry.primary)
                        .or_else(|| version.files.first())?;
                    Some((version.id.clone(), file.filename.clone()))
                }),
            Some("curseforge") => {
                let Some(project_id) = row.project_id.as_deref() else {
                    continue;
                };
                let versions = search::curseforge::project_versions(
                    state,
                    project_id,
                    ContentKind::DataPack,
                    &instance.version_id,
                    None,
                )
                .await
                .unwrap_or_default();
                search::pick_best(versions)
                    .filter(|version| Some(&version.id) != row.version_id.as_ref())
                    .filter(|version| version.file_name != row.file_name)
                    .map(|version| (version.id, version.file_name))
            }
            _ => None,
        };

        let (id, file_name) = match newer {
            Some(pair) => (Some(pair.0), Some(pair.1)),
            None => (None, None),
        };
        if id.is_some() {
            found += 1;
        }
        let _ = state.db.set_world_datapack_latest(
            &instance.id,
            world,
            &row.file_name,
            id.as_deref(),
            file_name.as_deref(),
        );
    }

    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    #[ignore]
    fn report_on_the_real_data_directory() {
        let root = PathBuf::from(std::env::var("HOME").unwrap())
            .join(".local/share/com.megalithofficial.basalt-launcher");
        let paths = Paths { root };
        let files = FileManager::new(paths.clone()).unwrap();
        let db = Db::open(&files).unwrap();

        for instance in db.list_instances(&files).unwrap() {
            let groups = list(&files, &db, &instance).unwrap();
            if groups.is_empty() {
                continue;
            }
            println!("{} ({})", instance.name, instance.version_id);
            for group in groups {
                println!("  {} [{}]", group.display_name, group.world);
                for pack in group.packs {
                    println!(
                        "      {:<52} {:>7} KB  fmt {:?}-{:?}  {:?}{}{}",
                        pack.file_name,
                        pack.size / 1024,
                        pack.min_format,
                        pack.max_format,
                        pack.compatibility,
                        if pack.enabled { "" } else { "  DISABLED" },
                        if pack.off_in_game {
                            "  OFF-IN-GAME"
                        } else {
                            ""
                        },
                    );
                }
            }
        }
    }
}
