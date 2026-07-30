use std::{
    collections::HashMap,
    io::Read,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use tauri::AppHandle;

use crate::{
    config::Instance,
    download::{self, DownloadSpec},
    error::{Error, Result},
    install, loaders,
    search::{self, Provider},
    state::AppState,
};

const MODRINTH: &str = "https://api.modrinth.com/v2";

#[derive(Deserialize)]
struct MrIndex {
    name: String,
    #[serde(default)]
    dependencies: HashMap<String, String>,
    #[serde(default)]
    files: Vec<MrFile>,
}

#[derive(Deserialize)]
struct MrFile {
    path: String,
    #[serde(default)]
    hashes: MrHashes,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(rename = "fileSize", default)]
    file_size: Option<u64>,
    #[serde(default)]
    env: Option<MrEnv>,
}

#[derive(Deserialize, Default)]
struct MrHashes {
    #[serde(default)]
    sha1: Option<String>,
}

#[derive(Deserialize)]
struct MrEnv {
    #[serde(default)]
    client: Option<String>,
}

fn loader_from_dependencies(deps: &HashMap<String, String>) -> Result<Option<(String, String)>> {
    for (key, loader) in [
        ("fabric-loader", "fabric"),
        ("quilt-loader", "quilt"),
        ("neoforge", "neoforge"),
        ("forge", "forge"),
    ] {
        if let Some(version) = deps.get(key) {
            return Ok(Some((loader.to_string(), version.clone())));
        }
    }
    let known = [
        "minecraft",
        "fabric-loader",
        "quilt-loader",
        "neoforge",
        "forge",
    ];
    if let Some(unknown) = deps.keys().find(|k| !known.contains(&k.as_str())) {
        return Err(Error::other(format!(
            "This pack needs an unsupported loader: {unknown}"
        )));
    }
    Ok(None)
}

fn sanitize_relative(path: &str) -> Result<PathBuf> {
    let p = Path::new(path);
    if p.is_absolute() {
        return Err(Error::other(format!("unsafe path in pack: {path}")));
    }
    let mut out = PathBuf::new();
    for component in p.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return Err(Error::other(format!("unsafe path in pack: {path}"))),
        }
    }
    if out.as_os_str().is_empty() {
        return Err(Error::other(format!("unsafe path in pack: {path}")));
    }
    Ok(out)
}

fn kind_for_path(path: &str) -> Option<&'static str> {
    if path.starts_with("mods/") {
        Some("mods")
    } else if path.starts_with("resourcepacks/") {
        Some("resourcepacks")
    } else if path.starts_with("shaderpacks/") {
        Some("shaderpacks")
    } else {
        None
    }
}

fn extract_overrides(
    files: &crate::files::FileManager,
    archive_path: &Path,
    dest: &Path,
) -> Result<()> {
    let file = files.open(archive_path)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| Error::other(format!("opening modpack archive: {e}")))?;

    for prefix in ["overrides/", "client-overrides/"] {
        for i in 0..zip.len() {
            let mut entry = zip
                .by_index(i)
                .map_err(|e| Error::other(format!("reading modpack entry: {e}")))?;
            let name = entry.name().to_string();
            let Some(rest) = name.strip_prefix(prefix) else {
                continue;
            };
            if rest.is_empty() || name.ends_with('/') {
                continue;
            }
            let relative = sanitize_relative(rest)?;
            let target = dest.join(relative);
            let mut buffer = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buffer)?;
            files.write_atomic(target, &buffer)?;
        }
    }
    Ok(())
}

fn read_index(files: &crate::files::FileManager, archive_path: &Path) -> Result<MrIndex> {
    let file = files.open(archive_path)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| Error::other(format!("opening modpack archive: {e}")))?;
    let mut entry = zip
        .by_name("modrinth.index.json")
        .map_err(|_| Error::other("Not a Modrinth pack: modrinth.index.json missing."))?;
    let mut raw = String::new();
    entry.read_to_string(&mut raw)?;
    Ok(serde_json::from_str(&raw)?)
}

#[derive(Deserialize)]
struct HashVersion {
    project_id: String,
    id: String,
}

#[tracing::instrument(skip_all, fields(files = files.len()))]
async fn link_pack_files(state: &AppState, instance_id: &str, files: &[(String, String)]) {
    let hashes: Vec<String> = files.iter().map(|(_, sha1)| sha1.clone()).collect();
    if hashes.is_empty() {
        return;
    }

    let request = state
        .network
        .post(format!("{MODRINTH}/version_files"))
        .json(&serde_json::json!({ "hashes": hashes, "algorithm": "sha1" }));
    let Ok(resp) = state.network.send(request).await else {
        return;
    };
    let Ok(by_hash) = resp.json::<HashMap<String, HashVersion>>().await else {
        tracing::warn!("could not match pack files to modrinth projects");
        return;
    };

    let project_ids: Vec<String> = {
        let mut ids: Vec<String> = by_hash.values().map(|v| v.project_id.clone()).collect();
        ids.sort();
        ids.dedup();
        ids
    };
    let projects = search::resolve_projects(state, Provider::Modrinth, &project_ids)
        .await
        .unwrap_or_default();
    let project_info: HashMap<String, &search::ProjectSummary> =
        projects.iter().map(|p| (p.id.clone(), p)).collect();

    let now = chrono::Utc::now().timestamp();
    for (path, sha1) in files {
        let Some(version) = by_hash.get(sha1) else {
            continue;
        };
        let Some(kind) = kind_for_path(path) else {
            continue;
        };
        let Some(file_name) = Path::new(path).file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        let info = project_info.get(&version.project_id);
        let _ = state.db.record_content_file(
            instance_id,
            kind,
            &crate::db::ContentFile {
                file_name: file_name.to_string(),
                sha1: Some(sha1.clone()),
                provider: Some("modrinth".to_string()),
                project_id: Some(version.project_id.clone()),
                version_id: Some(version.id.clone()),
                title: info.map(|i| i.title.clone()),
                icon_url: info.and_then(|i| i.icon_url.clone()),
                origin: "pack".to_string(),
                installed_at: now,
                ..Default::default()
            },
        );
    }
}

#[tracing::instrument(skip(app, state), fields(provider = provider.as_str()), err)]
pub async fn install_modpack(
    app: &AppHandle,
    state: &AppState,
    provider: Provider,
    project_id: &str,
    version_id: &str,
) -> Result<Instance> {
    if !matches!(provider, Provider::Modrinth) {
        return Err(Error::other(
            "CurseForge modpacks are not supported yet. Use a Modrinth pack.",
        ));
    }

    let target = search::fetch_version(
        state,
        provider,
        project_id,
        search::ContentKind::Modpack,
        "",
        None,
        Some(version_id),
    )
    .await?;
    let (url, archive) = search::download_url(&target)?;

    let cache_dir = state.paths.root.join("cache").join("modpacks");
    let archive_path = cache_dir.join(&archive.file_name);
    download::download_one(
        &state.network,
        &state.files,
        &DownloadSpec {
            url,
            dest: archive_path.clone(),
            sha1: archive.sha1.clone(),
            size: archive.size,
        },
    )
    .await?;

    let index = {
        let path = archive_path.clone();
        let files = state.files.clone();
        tokio::task::spawn_blocking(move || read_index(&files, &path))
            .await
            .map_err(|e| Error::other(format!("modpack parse task failed: {e}")))??
    };

    let game_version = index
        .dependencies
        .get("minecraft")
        .cloned()
        .ok_or_else(|| Error::other("Pack index does not declare a Minecraft version."))?;
    let loader = loader_from_dependencies(&index.dependencies)?;
    tracing::info!(
        pack = %index.name,
        game_version = %game_version,
        loader = ?loader,
        pack_files = index.files.len(),
        "modpack index parsed"
    );

    let existing_names: Vec<String> = state
        .db
        .list_instances(&state.files)?
        .into_iter()
        .map(|i| i.name)
        .collect();
    let mut name = index.name.clone();
    let mut counter = 2;
    while existing_names.contains(&name) {
        name = format!("{} ({counter})", index.name);
        counter += 1;
    }

    let id = uuid::Uuid::new_v4().to_string();
    let instance = Instance {
        dir: state.paths.instance_dir(&id).display().to_string(),
        logo: None,
        id,
        name,
        version_id: game_version.clone(),
        created_at: chrono::Utc::now(),
        min_memory_mb: None,
        max_memory_mb: None,
        java_path: None,
        last_played_at: None,
        playtime_secs: 0,
        loader: loader.as_ref().map(|(l, _)| l.clone()),
        loader_version: loader.as_ref().map(|(_, v)| v.clone()),
        launch_version_id: None,
        pack_provider: Some(provider.as_str().to_string()),
        pack_project_id: Some(project_id.to_string()),
        pack_version_id: Some(target.id.clone()),
        import_source: None,
        import_source_id: None,
        jvm_args: None,
        jvm_args_mode: None,
        env_vars: None,
        env_vars_mode: None,
    };
    let instance_dir = state.paths.instance_dir(&instance.id);
    state.files.ensure_dir(&instance_dir)?;
    state.db.insert_instance(&instance)?;
    tracing::info!(instance_id = %instance.id, name = %instance.name, "modpack instance created");

    let task = state.tasks.start(
        app,
        crate::tasks::TaskKind::ModpackInstall,
        crate::tasks::TaskSpec {
            title: index.name.clone(),
            subtitle: Some(format!(
                "{}{}",
                instance.version_id,
                instance
                    .loader
                    .as_deref()
                    .map(|l| format!(" · {l}"))
                    .unwrap_or_default()
            )),
            instance_id: Some(instance.id.clone()),
            project_id: Some(project_id.to_string()),
            ..Default::default()
        },
    );

    let outcome = install_pack_body(
        app,
        state,
        provider,
        project_id,
        &instance,
        &instance_dir,
        &archive_path,
        &index,
        &task,
    )
    .await;

    if let Err(e) = outcome {
        let _ = state.db.delete_instance_content_files(&instance.id);
        let _ = state.db.delete_instance(&instance.id);
        let _ = state.files.remove_instance_dir(&instance.id);
        match &e {
            Error::Cancelled => task.cancelled(),
            other => task.fail(other),
        }
        return Err(e);
    }

    task.succeed();

    state
        .db
        .list_instances(&state.files)?
        .into_iter()
        .find(|i| i.id == instance.id)
        .ok_or_else(|| Error::other("instance vanished after pack install"))
}

#[allow(clippy::too_many_arguments)]
async fn install_pack_body(
    app: &AppHandle,
    state: &AppState,
    provider: Provider,
    project_id: &str,
    instance: &Instance,
    instance_dir: &Path,
    archive_path: &Path,
    index: &MrIndex,
    task: &crate::tasks::TaskHandle,
) -> Result<()> {
    let launch_id = if instance.loader.is_some() {
        let launch_id = loaders::install_loader(app, state, instance, task).await?;
        state.db.set_launch_version(&instance.id, &launch_id)?;
        launch_id
    } else {
        instance.version_id.clone()
    };
    install::install_version(app, state, &instance.id, &launch_id, task).await?;

    task.stage("modpack-files");
    let mut specs = Vec::new();
    let mut linkable: Vec<(String, String)> = Vec::new();
    for file in &index.files {
        if file
            .env
            .as_ref()
            .and_then(|e| e.client.as_deref())
            .is_some_and(|c| c == "unsupported")
        {
            continue;
        }
        let Some(url) = file.downloads.first() else {
            continue;
        };
        let relative = sanitize_relative(&file.path)?;
        specs.push(DownloadSpec {
            url: url.clone(),
            dest: instance_dir.join(relative),
            sha1: file.hashes.sha1.clone(),
            size: file.file_size,
        });
        if let Some(sha1) = &file.hashes.sha1 {
            linkable.push((file.path.clone(), sha1.clone()));
        }
    }
    let concurrency = state.db.load_settings()?.concurrent_downloads;
    task.set_total(
        specs.len() as u64,
        specs.iter().filter_map(|s| s.size).sum(),
    );
    let downloaded = download::download_many_cancellable(
        &state.network,
        &state.files,
        specs,
        concurrency,
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
        Some(&|attempt, max, reason| task.note_retry(attempt, max, reason)),
    )
    .await;

    downloaded?;

    task.stage("modpack-overrides");
    {
        let archive = archive_path.to_path_buf();
        let dest = instance_dir.to_path_buf();
        let files = state.files.clone();
        tokio::task::spawn_blocking(move || extract_overrides(&files, &archive, &dest))
            .await
            .map_err(|e| Error::other(format!("override extraction task failed: {e}")))??;
    }

    link_pack_files(state, &instance.id, &linkable).await;

    if let Some(icon_url) = search::resolve_projects(state, provider, &[project_id.to_string()])
        .await
        .ok()
        .and_then(|mut list| list.pop())
        .and_then(|summary| summary.icon_url)
    {
        crate::meta::media::fetch_instance_logo(
            &state.network,
            &state.files,
            &instance.id,
            &icon_url,
        )
        .await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{kind_for_path, loader_from_dependencies, sanitize_relative};

    #[test]
    fn rejects_unsafe_paths() {
        assert!(sanitize_relative("mods/sodium.jar").is_ok());
        assert!(sanitize_relative("config/deep/nested.toml").is_ok());
        assert!(sanitize_relative("../escape.jar").is_err());
        assert!(sanitize_relative("mods/../../escape.jar").is_err());
        assert!(sanitize_relative("/etc/passwd").is_err());
        assert!(sanitize_relative("").is_err());
    }

    #[test]
    fn maps_loaders_and_kinds() {
        let mut deps = HashMap::new();
        deps.insert("minecraft".to_string(), "26.2".to_string());
        deps.insert("fabric-loader".to_string(), "0.19.3".to_string());
        assert_eq!(
            loader_from_dependencies(&deps).unwrap(),
            Some(("fabric".to_string(), "0.19.3".to_string()))
        );
        assert_eq!(kind_for_path("mods/a.jar"), Some("mods"));
        assert_eq!(kind_for_path("shaderpacks/b.zip"), Some("shaderpacks"));
        assert_eq!(kind_for_path("config/c.toml"), None);
    }
}
