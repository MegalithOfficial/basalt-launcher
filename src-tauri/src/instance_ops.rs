use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::Serialize;
use sha2::{Digest, Sha512};
use tauri::AppHandle;

use crate::{
    config::Instance,
    content,
    download::{self, DownloadSpec},
    error::{Error, Result},
    install, loaders,
    search::{self, ContentKind, Provider},
    state::AppState,
    tasks::{TaskHandle, TaskKind, TaskSpec},
};

const CONTENT_KINDS: &[&str] = &["mods", "resourcepacks", "shaderpacks", "schematics"];

#[derive(Debug, Clone, Serialize)]
pub struct RepairReport {
    pub checked_content: u64,
    pub repaired_content: u64,
    pub unresolved: Vec<String>,
}

fn instance_busy(state: &AppState, instance_id: &str) -> bool {
    state.tasks.list().iter().any(|task| {
        task.instance_id.as_deref() == Some(instance_id)
            && task.state == crate::tasks::TaskState::Running
    }) || state
        .running
        .lock()
        .unwrap()
        .values()
        .any(|run| run.instance_id == instance_id && run.status.lock().unwrap().state == "running")
}

async fn content_valid(
    files: &crate::files::FileManager,
    path: &Path,
    sha1: Option<&str>,
    sha512: Option<&str>,
) -> Result<bool> {
    if !files.is_file(path)? {
        return Ok(false);
    }
    if sha1.is_none() && sha512.is_none() {
        return Ok(true);
    }
    let files = files.clone();
    let path = path.to_path_buf();
    let expected_sha1 = sha1.map(str::to_string);
    let expected_sha512 = sha512.map(str::to_string);
    tokio::task::spawn_blocking(move || {
        let mut reader = files.open(path)?;
        let mut sha1 = expected_sha1.as_ref().map(|_| sha1_smol::Sha1::new());
        let mut sha512 = expected_sha512.as_ref().map(|_| Sha512::new());
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            if let Some(hasher) = &mut sha1 {
                hasher.update(&buffer[..read]);
            }
            if let Some(hasher) = &mut sha512 {
                hasher.update(&buffer[..read]);
            }
        }
        if expected_sha1
            .as_deref()
            .zip(sha1.map(|hasher| hasher.digest().to_string()))
            .is_some_and(|(expected, actual)| expected != actual)
        {
            return Ok(false);
        }
        if expected_sha512
            .as_deref()
            .zip(sha512.map(|hasher| format!("{:x}", hasher.finalize())))
            .is_some_and(|(expected, actual)| expected != actual)
        {
            return Ok(false);
        }
        Ok(true)
    })
    .await
    .map_err(|error| Error::other(format!("checksum task failed: {error}")))?
}

async fn repair_content(
    state: &AppState,
    instance: &Instance,
    task: &TaskHandle,
) -> Result<RepairReport> {
    let mut report = RepairReport {
        checked_content: 0,
        repaired_content: 0,
        unresolved: Vec::new(),
    };
    let tracked = CONTENT_KINDS
        .iter()
        .map(|kind| {
            state
                .db
                .content_files(&instance.id, kind)
                .map(|files| files.into_iter().map(|file| ((*kind).to_string(), file)))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let tracked_total = tracked.len() as u64;
    task.set_total(tracked_total, 0);

    for (kind, file) in tracked {
        if task.token().is_cancelled() {
            return Err(Error::Cancelled);
        }
        report.checked_content += 1;
        task.progress(report.checked_content, tracked_total, 0, 0);

        let directory = content::dir_for(&state.paths, &instance.id, &kind)?;
        let enabled = directory.join(&file.file_name);
        let disabled = directory.join(format!("{}.disabled", file.file_name));
        let path = if state.files.is_file(&disabled)? {
            disabled
        } else {
            enabled
        };
        if content_valid(
            &state.files,
            &path,
            file.sha1.as_deref(),
            file.sha512.as_deref(),
        )
        .await?
        {
            continue;
        }

        let repaired = async {
            let provider = Provider::parse(
                file.provider
                    .as_deref()
                    .ok_or_else(|| Error::other("file is not linked to a provider"))?,
            )?;
            let project_id = file
                .project_id
                .as_deref()
                .ok_or_else(|| Error::other("file has no project identity"))?;
            let version_id = file
                .version_id
                .as_deref()
                .ok_or_else(|| Error::other("file has no recorded version"))?;
            let content_kind = ContentKind::parse(&kind)?;
            let version = search::fetch_version(
                state,
                provider,
                project_id,
                content_kind,
                &instance.version_id,
                instance.loader.as_deref(),
                Some(version_id),
            )
            .await?;
            let (url, remote) = search::download_url(&version)?;
            state.files.ensure_dir(&directory)?;
            let retry = |attempt, max, reason: &str| task.note_retry(attempt, max, reason);
            let spec = DownloadSpec {
                url,
                dest: path.clone(),
                sha1: remote.sha1.clone().or_else(|| file.sha1.clone()),
                sha256: None,
                size: remote.size,
            };
            let download = download::download_one_reporting(
                &state.network,
                &state.files,
                &spec,
                Some(&retry),
                None,
            );
            tokio::pin!(download);
            let cancellation = task.token();
            tokio::select! {
                result = &mut download => { result?; }
                () = cancellation.cancelled() => return Err(Error::Cancelled),
            }
            if !content_valid(
                &state.files,
                &path,
                remote.sha1.as_deref().or(file.sha1.as_deref()),
                remote.sha512.as_deref().or(file.sha512.as_deref()),
            )
            .await?
            {
                let _ = state.files.remove_file_if_exists(&path);
                return Err(Error::other("downloaded file failed verification"));
            }
            Result::<()>::Ok(())
        }
        .await;

        match repaired {
            Ok(()) => report.repaired_content += 1,
            Err(Error::Cancelled) => return Err(Error::Cancelled),
            Err(error) => report
                .unresolved
                .push(format!("{}: {error}", file.file_name)),
        }
    }
    Ok(report)
}

pub async fn repair(app: &AppHandle, state: &AppState, instance: Instance) -> Result<RepairReport> {
    if instance_busy(state, &instance.id) {
        return Err(Error::other(
            "Stop the instance and wait for its current task before repairing it.",
        ));
    }
    let task = Arc::new(state.tasks.start(
        app,
        TaskKind::InstanceRepair,
        TaskSpec {
            title: format!("Repair {}", instance.name),
            subtitle: Some("Verifying game and content files".to_string()),
            instance_id: Some(instance.id.clone()),
            ..Default::default()
        },
    ));

    let result = async {
        task.stage("loader-profile");
        let launch_id = match (&instance.loader, &instance.launch_version_id) {
            (Some(_), Some(id)) if install::load_version_json(state, id).await.is_ok() => {
                id.clone()
            }
            (Some(_), _) => {
                let id = loaders::install_loader(app, state, &instance, &task).await?;
                state.db.set_launch_version(&instance.id, &id)?;
                id
            }
            (None, _) => instance.version_id.clone(),
        };

        task.stage("game-files");
        install::install_version(app, state, &instance.id, &launch_id, &task).await?;
        task.stage("content-files");
        repair_content(state, &instance, &task).await
    }
    .await;
    task.finish(&result);
    result
}

#[derive(Debug)]
struct CopyFile {
    source: PathBuf,
    destination: PathBuf,
    size: u64,
}

fn collect_copy_files(
    files: &crate::files::FileManager,
    source: &Path,
    destination: &Path,
    output: &mut Vec<CopyFile>,
) -> Result<()> {
    for path in files.read_dir(source)? {
        let metadata = files.symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(Error::other(format!(
                "Cannot duplicate an instance containing a symbolic link: {}",
                path.display()
            )));
        }
        let target = destination.join(path.file_name().unwrap_or_default());
        if metadata.is_dir() {
            collect_copy_files(files, &path, &target, output)?;
        } else if metadata.is_file() {
            output.push(CopyFile {
                source: path,
                destination: target,
                size: metadata.len(),
            });
        }
    }
    Ok(())
}

fn copy_instance_files(
    files: &crate::files::FileManager,
    source: &Path,
    destination: &Path,
    task: &TaskHandle,
) -> Result<()> {
    let mut entries = Vec::new();
    collect_copy_files(files, source, destination, &mut entries)?;
    let total_files = entries.len() as u64;
    let total_bytes = entries.iter().map(|entry| entry.size).sum();
    task.set_total(total_files, total_bytes);
    files.ensure_dir(destination)?;
    let mut copied_bytes = 0;
    for (index, entry) in entries.into_iter().enumerate() {
        if task.token().is_cancelled() {
            return Err(Error::Cancelled);
        }
        let mut reader = files.open(&entry.source)?;
        let mut writer = files.create(&entry.destination)?;
        let mut buffer = [0_u8; 1024 * 1024];
        loop {
            if task.token().is_cancelled() {
                return Err(Error::Cancelled);
            }
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            writer.write_all(&buffer[..read])?;
            copied_bytes += read as u64;
            task.progress(index as u64, total_files, copied_bytes, total_bytes);
        }
        writer.sync_all()?;
        task.progress((index + 1) as u64, total_files, copied_bytes, total_bytes);
    }
    Ok(())
}

fn copy_name(state: &AppState, original: &str) -> Result<String> {
    let names: Vec<String> = state
        .db
        .list_instances(&state.files)?
        .into_iter()
        .map(|instance| instance.name)
        .collect();
    for index in 1.. {
        let suffix = if index == 1 {
            "Copy".to_string()
        } else {
            format!("Copy {index}")
        };
        let candidate = format!("{original} {suffix}");
        if !names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&candidate))
        {
            return Ok(candidate);
        }
    }
    unreachable!()
}

pub async fn duplicate(app: &AppHandle, state: &AppState, source: Instance) -> Result<Instance> {
    if instance_busy(state, &source.id) {
        return Err(Error::other(
            "Stop the instance and wait for its current task before duplicating it.",
        ));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let mut duplicate = source.clone();
    duplicate.id = id.clone();
    duplicate.name = copy_name(state, &source.name)?;
    duplicate.dir = state.paths.instance_dir(&id).display().to_string();
    duplicate.logo = None;
    duplicate.created_at = chrono::Utc::now();
    duplicate.last_played_at = None;
    duplicate.playtime_secs = 0;
    duplicate.import_source = None;
    duplicate.import_source_id = None;

    let task = Arc::new(state.tasks.start(
        app,
        TaskKind::InstanceDuplicate,
        TaskSpec {
            title: format!("Duplicate {}", source.name),
            subtitle: Some(format!("Creating {}", duplicate.name)),
            instance_id: Some(source.id.clone()),
            ..Default::default()
        },
    ));
    let destination = state.paths.instance_dir(&id);
    let result = match tokio::task::spawn_blocking({
        let files = state.files.clone();
        let source_dir = state.paths.instance_dir(&source.id);
        let destination = destination.clone();
        let task = Arc::clone(&task);
        move || copy_instance_files(&files, &source_dir, &destination, &task)
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(Error::other(format!("instance copy task failed: {error}"))),
    };

    if let Err(error) = result {
        let _ = state.files.remove_instance_dir(&id);
        match &error {
            Error::Cancelled => task.cancelled(),
            _ => task.fail(&error),
        }
        return Err(error);
    }

    let persist = (|| {
        state.db.insert_instance(&duplicate)?;
        state.db.clone_instance_content(&source.id, &id)?;
        state.db.clone_instance_placement(&source.id, &id)?;
        if let Some(logo) = source.logo.as_deref() {
            let path = Path::new(logo);
            let extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("png");
            let bytes = state.files.read(path)?;
            crate::meta::media::write_instance_logo_sync(&state.files, &id, extension, &bytes)?;
        }
        Result::<()>::Ok(())
    })();
    if let Err(error) = persist {
        let _ = state.db.delete_instance_content_files(&id);
        let _ = state.db.delete_instance(&id);
        let _ = state.files.remove_instance_dir(&id);
        task.fail(&error);
        return Err(error);
    }

    task.succeed();
    state
        .db
        .list_instances(&state.files)?
        .into_iter()
        .find(|instance| instance.id == id)
        .ok_or_else(|| Error::NotFound("duplicated instance".to_string()))
}
