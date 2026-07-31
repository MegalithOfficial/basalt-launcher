use std::{
    collections::HashMap,
    io::Read,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    config::Instance,
    db::ContentFile,
    download::{self, DownloadSpec},
    error::{Error, Result},
    install, loaders, packs,
    search::{self, Provider},
    state::AppState,
};

const MODRINTH: &str = "https://api.modrinth.com/v2";

#[derive(Deserialize)]
pub(crate) struct MrIndex {
    pub name: String,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    #[serde(default)]
    pub files: Vec<MrFile>,
}

#[derive(Deserialize)]
pub(crate) struct MrFile {
    pub path: String,
    #[serde(default)]
    pub hashes: MrHashes,
    #[serde(default)]
    pub downloads: Vec<String>,
    #[serde(rename = "fileSize", default)]
    pub file_size: Option<u64>,
    #[serde(default)]
    pub env: Option<MrEnv>,
    #[serde(skip)]
    pub local_source: Option<PathBuf>,
}

#[derive(Deserialize, Default)]
pub(crate) struct MrHashes {
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct MrEnv {
    #[serde(default)]
    pub client: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManualDownload {
    pub project_id: String,
    pub file_id: String,
    pub file_name: String,
    pub download_page_url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub instance_path: String,
    pub pack_archive: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManualDownloadSource {
    pub project_id: String,
    pub file_id: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct ModpackInstallPlan {
    pub manual_downloads: Vec<ManualDownload>,
}

pub(crate) fn loader_from_dependencies(
    deps: &HashMap<String, String>,
) -> Result<Option<(String, String)>> {
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

fn unavailable_curseforge_files(files: &[String]) -> Error {
    let shown = files.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
    let remaining = files.len().saturating_sub(5);
    let suffix = if remaining == 0 {
        String::new()
    } else {
        format!(" and {remaining} more")
    };
    Error::other(format!(
        "CurseForge did not provide downloads for {} pack file{}. Their authors may have disabled third-party downloads, so Basalt cannot install this pack completely. Use the CurseForge app or choose another pack. Affected: {shown}{suffix}.",
        files.len(),
        if files.len() == 1 { "" } else { "s" }
    ))
}

fn manual_key(project_id: &str, file_id: &str) -> String {
    format!("{project_id}:{file_id}")
}

fn matches_download_name(expected: &str, actual: &str) -> bool {
    if actual == expected {
        return true;
    }
    let expected = Path::new(expected);
    let actual = Path::new(actual);
    let Some(expected_stem) = expected.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    if actual.extension() != expected.extension() {
        return false;
    }
    let Some(actual_stem) = actual.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    let Some(number) = actual_stem
        .strip_prefix(expected_stem)
        .and_then(|rest| rest.strip_prefix(" (").or_else(|| rest.strip_prefix('(')))
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return false;
    };
    !number.is_empty() && number.chars().all(|character| character.is_ascii_digit())
}

fn sha1_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha1_smol::Sha1::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.digest().to_string())
}

fn validate_manual_download(
    downloads_dir: &Path,
    requirement: &ManualDownload,
    path: &Path,
) -> Result<()> {
    let downloads_dir = downloads_dir.canonicalize()?;
    let path = path.canonicalize()?;
    if path.parent() != Some(downloads_dir.as_path()) {
        return Err(Error::other(
            "Manual downloads must come from the OS Downloads folder.",
        ));
    }
    let actual_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::other("The downloaded file name is not valid UTF-8."))?;
    if !matches_download_name(&requirement.file_name, actual_name) {
        return Err(Error::other(format!(
            "Expected {}, found {actual_name}.",
            requirement.file_name
        )));
    }
    let actual_size = path.metadata()?.len();
    if let Some(expected) = requirement.size {
        if actual_size != expected {
            return Err(Error::SizeMismatch {
                path: path.display().to_string(),
                expected,
                actual: actual_size,
            });
        }
    }
    if let Some(expected) = &requirement.sha1 {
        let actual = sha1_file(&path)?;
        if &actual != expected {
            return Err(Error::Checksum {
                path: path.display().to_string(),
                expected: expected.clone(),
                actual,
            });
        }
    }
    Ok(())
}

pub(crate) fn extract_overrides(
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

struct PreparedPack {
    target: search::ProjectVersion,
    archive_path: PathBuf,
    index: MrIndex,
    curseforge_links: Vec<(String, String, ContentFile)>,
    consumed_sources: Vec<PathBuf>,
}

enum PreparePackOutcome {
    Ready(Box<PreparedPack>),
    NeedsDownloads(Vec<ManualDownload>),
}

async fn archive_requirement(
    state: &AppState,
    project_id: &str,
    version_id: &str,
    archive: &search::VersionFile,
) -> Result<ManualDownload> {
    let page = search::curseforge::project_download_pages(state, &[project_id.to_string()])
        .await?
        .remove(project_id)
        .ok_or_else(|| Error::other("CurseForge did not provide the pack's project page."))?;
    Ok(ManualDownload {
        project_id: project_id.to_string(),
        file_id: version_id.to_string(),
        file_name: archive.file_name.clone(),
        download_page_url: format!("{}/download/{version_id}", page.trim_end_matches('/')),
        sha1: archive.sha1.clone(),
        size: archive.size,
        instance_path: String::new(),
        pack_archive: true,
    })
}

async fn prepare_pack(
    app: &AppHandle,
    state: &AppState,
    provider: Provider,
    project_id: &str,
    version_id: &str,
    manual_sources: &[ManualDownloadSource],
    stage_manual_files: bool,
) -> Result<PreparePackOutcome> {
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
    let archive = target
        .primary_file()
        .cloned()
        .ok_or_else(|| Error::other("This pack version has no downloadable file."))?;
    let archive_path = state
        .paths
        .root
        .join("cache")
        .join("modpacks")
        .join(&archive.file_name);
    let downloads_dir = app.path().download_dir()?;
    let by_key: HashMap<String, &ManualDownloadSource> = manual_sources
        .iter()
        .map(|source| (manual_key(&source.project_id, &source.file_id), source))
        .collect();
    let mut consumed_sources = Vec::new();
    let archive_read_path = if let Some(url) = archive.url.clone() {
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
        archive_path.clone()
    } else if provider == Provider::Curseforge {
        let requirement = archive_requirement(state, project_id, version_id, &archive).await?;
        let Some(source) = by_key.get(&manual_key(project_id, version_id)) else {
            return Ok(PreparePackOutcome::NeedsDownloads(vec![requirement]));
        };
        let source_path = PathBuf::from(&source.path);
        validate_manual_download(&downloads_dir, &requirement, &source_path)?;
        if stage_manual_files {
            download::copy_verified(
                &state.files,
                &source_path,
                &archive_path,
                requirement.sha1.as_deref(),
                requirement.size,
            )
            .await?;
            consumed_sources.push(source_path);
            archive_path.clone()
        } else {
            source_path
        }
    } else {
        return Err(search::download_url(&target).unwrap_err());
    };

    let (index, curseforge_links, skipped, manual_downloads) = match provider {
        Provider::Modrinth => {
            let index = {
                let path = archive_read_path.clone();
                let files = state.files.clone();
                tokio::task::spawn_blocking(move || read_index(&files, &path))
                    .await
                    .map_err(|error| {
                        Error::other(format!("modpack parse task failed: {error}"))
                    })??
            };
            (index, Vec::new(), Vec::new(), Vec::new())
        }
        Provider::Curseforge => packs::plan_curseforge_archive(state, &archive_read_path).await?,
    };
    if !skipped.is_empty() {
        return Err(unavailable_curseforge_files(&skipped));
    }

    let missing: Vec<ManualDownload> = manual_downloads
        .iter()
        .filter(|requirement| {
            !by_key.contains_key(&manual_key(&requirement.project_id, &requirement.file_id))
        })
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Ok(PreparePackOutcome::NeedsDownloads(missing));
    }

    let mut index = index;
    if stage_manual_files {
        for requirement in &manual_downloads {
            let source = by_key
                .get(&manual_key(&requirement.project_id, &requirement.file_id))
                .expect("manual source checked above");
            let source_path = PathBuf::from(&source.path);
            validate_manual_download(&downloads_dir, requirement, &source_path)?;
            index.files.push(MrFile {
                path: requirement.instance_path.clone(),
                hashes: MrHashes {
                    sha1: requirement.sha1.clone(),
                },
                downloads: Vec::new(),
                file_size: requirement.size,
                env: None,
                local_source: Some(source_path.clone()),
            });
            consumed_sources.push(source_path);
        }
    }

    Ok(PreparePackOutcome::Ready(Box::new(PreparedPack {
        target,
        archive_path,
        index,
        curseforge_links,
        consumed_sources,
    })))
}

pub async fn plan_modpack_install(
    app: &AppHandle,
    state: &AppState,
    provider: Provider,
    project_id: &str,
    version_id: &str,
    manual_sources: &[ManualDownloadSource],
) -> Result<ModpackInstallPlan> {
    let outcome = prepare_pack(
        app,
        state,
        provider,
        project_id,
        version_id,
        manual_sources,
        false,
    )
    .await?;
    Ok(ModpackInstallPlan {
        manual_downloads: match outcome {
            PreparePackOutcome::Ready(_) => Vec::new(),
            PreparePackOutcome::NeedsDownloads(downloads) => downloads,
        },
    })
}

pub async fn find_manual_download(
    app: &AppHandle,
    requirement: &ManualDownload,
    started_at_ms: u64,
) -> Result<Option<String>> {
    if Path::new(&requirement.file_name)
        .file_name()
        .and_then(|value| value.to_str())
        != Some(requirement.file_name.as_str())
    {
        return Err(Error::other("CurseForge returned an unsafe file name."));
    }
    let downloads_dir = app.path().download_dir()?;
    let requirement = requirement.clone();
    tokio::task::spawn_blocking(move || {
        let threshold = std::time::UNIX_EPOCH
            + std::time::Duration::from_millis(started_at_ms.saturating_sub(2_000));
        for entry in std::fs::read_dir(&downloads_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if !matches_download_name(&requirement.file_name, name) {
                continue;
            }
            let metadata = path.metadata()?;
            if metadata
                .modified()
                .is_ok_and(|modified| modified < threshold)
            {
                continue;
            }
            if validate_manual_download(&downloads_dir, &requirement, &path).is_ok() {
                return Ok(Some(path.display().to_string()));
            }
        }
        Ok(None)
    })
    .await
    .map_err(|error| Error::other(format!("Downloads scan failed: {error}")))?
}

#[tracing::instrument(skip(app, state), fields(provider = provider.as_str()), err)]
pub async fn install_modpack(
    app: &AppHandle,
    state: &AppState,
    provider: Provider,
    project_id: &str,
    version_id: &str,
    manual_sources: &[ManualDownloadSource],
) -> Result<Instance> {
    let outcome = prepare_pack(
        app,
        state,
        provider,
        project_id,
        version_id,
        manual_sources,
        true,
    )
    .await?;
    let PreparePackOutcome::Ready(prepared) = outcome else {
        return Err(Error::other(
            "Download all requested CurseForge files before continuing.",
        ));
    };
    let PreparedPack {
        target,
        archive_path,
        index,
        curseforge_links,
        consumed_sources,
    } = *prepared;

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

    let name = unique_instance_name(state, &index.name)?;

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
        Some((provider, project_id)),
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

    for (kind, _, file) in &curseforge_links {
        let _ = state.db.record_content_file(&instance.id, kind, file);
    }
    for source in consumed_sources {
        if let Err(error) = std::fs::remove_file(&source) {
            tracing::warn!(
                path = %source.display(),
                %error,
                "could not remove consumed CurseForge download"
            );
        }
    }
    state
        .db
        .list_instances(&state.files)?
        .into_iter()
        .find(|i| i.id == instance.id)
        .ok_or_else(|| Error::other("instance vanished after pack install"))
}

pub(crate) fn unique_instance_name(state: &AppState, base: &str) -> Result<String> {
    let taken: Vec<String> = state
        .db
        .list_instances(&state.files)?
        .into_iter()
        .map(|instance| instance.name)
        .collect();
    let mut name = base.to_string();
    let mut counter = 2;
    while taken.contains(&name) {
        name = format!("{base} ({counter})");
        counter += 1;
    }
    Ok(name)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn install_pack_body(
    app: &AppHandle,
    state: &AppState,
    icon_source: Option<(Provider, &str)>,
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
    let mut local_specs = Vec::new();
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
        let relative = sanitize_relative(&file.path)?;
        let destination = instance_dir.join(relative);
        if let Some(source) = &file.local_source {
            local_specs.push((
                source.clone(),
                destination,
                file.hashes.sha1.clone(),
                file.file_size,
            ));
        } else if let Some(url) = file.downloads.first() {
            specs.push(DownloadSpec {
                url: url.clone(),
                dest: destination,
                sha1: file.hashes.sha1.clone(),
                size: file.file_size,
            });
        } else {
            continue;
        }
        if let Some(sha1) = &file.hashes.sha1 {
            linkable.push((file.path.clone(), sha1.clone()));
        }
    }
    let concurrency = state.db.load_settings()?.concurrent_downloads;
    let total_files = specs.len() + local_specs.len();
    let total_bytes = specs.iter().filter_map(|spec| spec.size).sum::<u64>()
        + local_specs
            .iter()
            .filter_map(|(_, _, _, size)| *size)
            .sum::<u64>();
    let network_known_bytes = specs.iter().filter_map(|spec| spec.size).sum::<u64>();
    task.set_total(total_files as u64, total_bytes);
    let downloaded = download::download_many_cancellable(
        &state.network,
        &state.files,
        specs,
        concurrency,
        |progress| {
            task.progress(
                progress.completed as u64,
                total_files as u64,
                progress.downloaded_bytes,
                total_bytes,
            );
        },
        Some(task.token()),
        Some(task.written()),
        Some(&|attempt, max, reason| task.note_retry(attempt, max, reason)),
    )
    .await;

    downloaded?;

    let downloaded_files = total_files - local_specs.len();
    let mut copied_bytes = network_known_bytes;
    for (index, (source, destination, sha1, size)) in local_specs.iter().enumerate() {
        if task.token().is_cancelled() {
            return Err(Error::Cancelled);
        }
        let copied =
            download::copy_verified(&state.files, source, destination, sha1.as_deref(), *size)
                .await?;
        copied_bytes += copied;
        task.progress(
            (downloaded_files + index + 1) as u64,
            total_files as u64,
            copied_bytes,
            total_bytes,
        );
    }

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

    if let Some((provider, project_id)) = icon_source {
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
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{
        kind_for_path, loader_from_dependencies, matches_download_name, sanitize_relative,
        unavailable_curseforge_files,
    };

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

    #[test]
    fn explains_unavailable_curseforge_files() {
        let error = unavailable_curseforge_files(&[
            "first.jar (project 1, file 2)".to_string(),
            "second.jar (project 3, file 4)".to_string(),
        ]);
        let message = error.to_string();

        assert!(message.contains("authors may have disabled third-party downloads"));
        assert!(message.contains("first.jar"));
        assert!(message.contains("second.jar"));
        assert!(message.contains("cannot install this pack completely"));
    }

    #[test]
    fn accepts_only_expected_browser_download_names() {
        assert!(matches_download_name("example.jar", "example.jar"));
        assert!(matches_download_name("example.jar", "example (1).jar"));
        assert!(matches_download_name("example.jar", "example(1).jar"));
        assert!(matches_download_name("example.jar", "example (42).jar"));
        assert!(!matches_download_name("example.jar", "example.part"));
        assert!(!matches_download_name("example.jar", "other.jar"));
        assert!(!matches_download_name("example.jar", "example (copy).jar"));
    }
}
