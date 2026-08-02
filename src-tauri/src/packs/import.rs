use std::{collections::HashMap, io::Read, path::Path};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    config::Instance,
    db::ContentFile,
    error::{Error, Result},
    files::FileManager,
    modpack::{self, ManualDownload, MrFile, MrHashes, MrIndex},
    search::{self, curseforge},
    state::AppState,
    tasks::{TaskKind, TaskSpec},
};

use super::PackFormat;

#[derive(Debug, Clone, Serialize)]
pub struct PackPreview {
    pub format: PackFormat,
    pub name: String,
    pub version: Option<String>,
    pub author: Option<String>,
    pub game_version: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub declared_files: usize,
    pub override_files: usize,
    pub override_bytes: u64,
    pub warnings: Vec<String>,
    pub importable: bool,
}

#[derive(Deserialize)]
struct CfManifest {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    minecraft: CfMinecraft,
    #[serde(default)]
    files: Vec<CfEntry>,
}

#[derive(Deserialize, Default)]
struct CfMinecraft {
    #[serde(default)]
    version: String,
    #[serde(rename = "modLoaders", default)]
    mod_loaders: Vec<CfLoader>,
}

#[derive(Deserialize)]
struct CfLoader {
    #[serde(default)]
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct CfEntry {
    #[serde(rename = "projectID")]
    project_id: i64,
    #[serde(rename = "fileID")]
    file_id: i64,
    #[serde(default = "required_by_default")]
    required: bool,
}

fn required_by_default() -> bool {
    true
}

enum Parsed {
    Mrpack(MrIndex),
    Curseforge(CfManifest),
}

struct Archive {
    parsed: Parsed,
    override_files: usize,
    override_bytes: u64,
}

fn split_loader(id: &str) -> Option<(String, String)> {
    let (name, version) = id.split_once('-')?;
    let name = name.trim().to_lowercase();
    if version.trim().is_empty() {
        return None;
    }
    match name.as_str() {
        "fabric" | "quilt" | "forge" | "neoforge" => Some((name, version.trim().to_string())),
        _ => None,
    }
}

fn read_archive(files: &FileManager, path: &Path) -> Result<Archive> {
    let handle = files.open_external(path)?;
    let mut zip = zip::ZipArchive::new(handle)
        .map_err(|error| Error::other(format!("opening the pack: {error}")))?;

    let mut override_files = 0usize;
    let mut override_bytes = 0u64;
    for index in 0..zip.len() {
        let Ok(entry) = zip.by_index_raw(index) else {
            continue;
        };
        let name = entry.name();
        if name.ends_with('/') {
            continue;
        }
        if name.starts_with("overrides/") || name.starts_with("client-overrides/") {
            override_files += 1;
            override_bytes += entry.size();
        }
    }

    let mut raw = String::new();
    if let Ok(mut entry) = zip.by_name("modrinth.index.json") {
        entry.read_to_string(&mut raw)?;
        let index: MrIndex = serde_json::from_str(&raw)?;
        return Ok(Archive {
            parsed: Parsed::Mrpack(index),
            override_files,
            override_bytes,
        });
    }

    if let Ok(mut entry) = zip.by_name("manifest.json") {
        entry.read_to_string(&mut raw)?;
        let manifest: CfManifest = serde_json::from_str(&raw)?;
        return Ok(Archive {
            parsed: Parsed::Curseforge(manifest),
            override_files,
            override_bytes,
        });
    }

    Err(Error::other(
        "This file is not a modpack. Expected a Modrinth .mrpack or a CurseForge export.",
    ))
}

fn preview(archive: &Archive) -> PackPreview {
    let mut warnings = Vec::new();

    match &archive.parsed {
        Parsed::Mrpack(index) => {
            let game_version = index.dependencies.get("minecraft").cloned();
            let loader = match modpack::loader_from_dependencies(&index.dependencies) {
                Ok(loader) => loader,
                Err(error) => {
                    warnings.push(error.to_string());
                    None
                }
            };
            let declared = index.files.len();
            let unreachable = index
                .files
                .iter()
                .filter(|file| file.downloads.is_empty())
                .count();
            if unreachable > 0 {
                warnings.push(format!(
                    "{unreachable} files list no download and will be skipped"
                ));
            }
            if game_version.is_none() {
                warnings.push("The pack does not declare a Minecraft version.".to_string());
            }

            PackPreview {
                format: PackFormat::Mrpack,
                name: index.name.clone(),
                version: None,
                author: None,
                game_version: game_version.clone().unwrap_or_default(),
                loader: loader.as_ref().map(|(name, _)| name.clone()),
                loader_version: loader.as_ref().map(|(_, version)| version.clone()),
                declared_files: declared,
                override_files: archive.override_files,
                override_bytes: archive.override_bytes,
                importable: game_version.is_some(),
                warnings,
            }
        }
        Parsed::Curseforge(manifest) => {
            let loader = manifest
                .minecraft
                .mod_loaders
                .iter()
                .find(|entry| entry.primary)
                .or_else(|| manifest.minecraft.mod_loaders.first())
                .and_then(|entry| split_loader(&entry.id));
            if loader.is_none() && !manifest.minecraft.mod_loaders.is_empty() {
                warnings.push(format!(
                    "Unsupported loader: {}",
                    manifest.minecraft.mod_loaders[0].id
                ));
            }
            let optional = manifest.files.iter().filter(|file| !file.required).count();
            if optional > 0 {
                warnings.push(format!("{optional} optional files will be installed too"));
            }
            warnings.push(
                "CurseForge packs need an API key, and some authors block third party downloads."
                    .to_string(),
            );

            PackPreview {
                format: PackFormat::Curseforge,
                name: manifest.name.clone(),
                version: manifest.version.clone(),
                author: manifest.author.clone().filter(|text| !text.is_empty()),
                game_version: manifest.minecraft.version.clone(),
                loader: loader.as_ref().map(|(name, _)| name.clone()),
                loader_version: loader.as_ref().map(|(_, version)| version.clone()),
                declared_files: manifest.files.len(),
                override_files: archive.override_files,
                override_bytes: archive.override_bytes,
                importable: !manifest.minecraft.version.is_empty(),
                warnings,
            }
        }
    }
}

pub async fn inspect_pack(state: &AppState, path: &Path) -> Result<PackPreview> {
    let files = state.files.clone();
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        read_archive(&files, &path).map(|archive| preview(&archive))
    })
    .await
    .map_err(|error| Error::other(format!("pack inspection failed: {error}")))?
}

fn directory_for(class_id: Option<u32>, file_name: &str) -> &'static str {
    match class_id {
        Some(12) => "resourcepacks",
        Some(6552) => "shaderpacks",
        Some(6945) => "datapacks",
        Some(6) => "mods",
        _ if file_name.ends_with(".jar") => "mods",
        _ => "resourcepacks",
    }
}

struct Resolved {
    index: MrIndex,
    links: Vec<(String, String, ContentFile)>,
    skipped: Vec<String>,
    manual_downloads: Vec<ManualDownload>,
}

async fn resolve_curseforge(state: &AppState, manifest: &CfManifest) -> Result<Resolved> {
    let loader = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|entry| entry.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first())
        .and_then(|entry| split_loader(&entry.id));

    let mut dependencies = HashMap::new();
    dependencies.insert("minecraft".to_string(), manifest.minecraft.version.clone());
    if let Some((name, version)) = &loader {
        let key = match name.as_str() {
            "fabric" => "fabric-loader",
            "quilt" => "quilt-loader",
            other => other,
        };
        dependencies.insert(key.to_string(), version.clone());
    }

    let file_ids: Vec<i64> = manifest.files.iter().map(|file| file.file_id).collect();
    let resolved = curseforge::files(state, &file_ids).await?;
    let by_id: HashMap<u64, &curseforge::File> =
        resolved.iter().map(|file| (file.id, file)).collect();

    let project_ids: Vec<String> = manifest
        .files
        .iter()
        .map(|file| file.project_id.to_string())
        .collect();
    let classes = curseforge::project_classes(state, &project_ids)
        .await
        .unwrap_or_default();
    let download_pages = curseforge::project_download_pages(state, &project_ids)
        .await
        .unwrap_or_default();
    let project_info: HashMap<String, search::ProjectSummary> =
        curseforge::resolve_projects(state, &project_ids)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|project| (project.id.clone(), project))
            .collect();

    let now = chrono::Utc::now().timestamp();
    let mut files = Vec::new();
    let mut links = Vec::new();
    let mut skipped = Vec::new();
    let mut manual_downloads = Vec::new();

    for entry in &manifest.files {
        let Some(file) = by_id.get(&(entry.file_id as u64)) else {
            skipped.push(format!(
                "project {} (file {})",
                entry.project_id, entry.file_id
            ));
            continue;
        };
        let sha1 = file
            .hashes
            .iter()
            .find(|hash| hash.algo == 1)
            .map(|hash| hash.value.clone());
        let kind = directory_for(
            classes.get(&entry.project_id.to_string()).copied(),
            &file.file_name,
        );
        let path = format!("{kind}/{}", file.file_name);
        let info = project_info.get(&entry.project_id.to_string());

        links.push((
            kind.to_string(),
            path.clone(),
            ContentFile {
                file_name: file.file_name.clone(),
                sha1: sha1.clone(),
                provider: Some("curseforge".to_string()),
                project_id: Some(entry.project_id.to_string()),
                version_id: Some(entry.file_id.to_string()),
                title: info
                    .map(|project| project.title.clone())
                    .or_else(|| Some(file.display_name.clone())),
                icon_url: info.and_then(|project| project.icon_url.clone()),
                origin: "pack".to_string(),
                installed_at: now,
                ..Default::default()
            },
        ));

        if let Some(url) = file.download_url.clone() {
            files.push(MrFile {
                path,
                hashes: MrHashes { sha1 },
                downloads: vec![url],
                file_size: file.file_length,
                env: None,
                local_source: None,
            });
        } else if let Some(page) = download_pages.get(&entry.project_id.to_string()) {
            manual_downloads.push(ManualDownload {
                project_id: entry.project_id.to_string(),
                file_id: entry.file_id.to_string(),
                file_name: file.file_name.clone(),
                download_page_url: format!(
                    "{}/download/{}",
                    page.trim_end_matches('/'),
                    entry.file_id
                ),
                sha1,
                size: file.file_length,
                instance_path: path,
                pack_archive: false,
            });
        } else {
            skipped.push(format!(
                "{} (project {}, file {})",
                file.file_name, entry.project_id, entry.file_id
            ));
        }
    }

    Ok(Resolved {
        index: MrIndex {
            name: if manifest.name.is_empty() {
                "Imported pack".to_string()
            } else {
                manifest.name.clone()
            },
            dependencies,
            files,
        },
        links,
        skipped,
        manual_downloads,
    })
}

pub(crate) async fn plan_curseforge_archive(
    state: &AppState,
    path: &Path,
) -> Result<(
    MrIndex,
    Vec<(String, String, ContentFile)>,
    Vec<String>,
    Vec<ManualDownload>,
)> {
    let archive = {
        let files = state.files.clone();
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || read_archive(&files, &path))
            .await
            .map_err(|error| Error::other(format!("pack parse task failed: {error}")))??
    };
    let Parsed::Curseforge(manifest) = archive.parsed else {
        return Err(Error::other(
            "The selected CurseForge version is not a CurseForge pack.",
        ));
    };
    let resolved = resolve_curseforge(state, &manifest).await?;
    Ok((
        resolved.index,
        resolved.links,
        resolved.skipped,
        resolved.manual_downloads,
    ))
}

pub struct PreparedImport {
    instance: Instance,
    instance_dir: std::path::PathBuf,
    staged: std::path::PathBuf,
    index: MrIndex,
    links: Vec<(String, String, ContentFile)>,
    skipped: Vec<String>,
    task: crate::tasks::TaskHandle,
}

impl PreparedImport {
    pub fn instance(&self) -> &Instance {
        &self.instance
    }
}

pub async fn prepare_import(
    app: &AppHandle,
    state: &AppState,
    path: &Path,
    name_override: Option<String>,
) -> Result<PreparedImport> {
    let archive = {
        let files = state.files.clone();
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || read_archive(&files, &path))
            .await
            .map_err(|error| Error::other(format!("pack parse task failed: {error}")))??
    };

    let (index, links, skipped) = match archive.parsed {
        Parsed::Mrpack(index) => (index, Vec::new(), Vec::new()),
        Parsed::Curseforge(manifest) => {
            let resolved = resolve_curseforge(state, &manifest).await?;
            let mut skipped = resolved.skipped;
            skipped.extend(
                resolved
                    .manual_downloads
                    .iter()
                    .map(|download| download.file_name.clone()),
            );
            (resolved.index, resolved.links, skipped)
        }
    };

    let game_version = index
        .dependencies
        .get("minecraft")
        .filter(|version| !version.is_empty())
        .cloned()
        .ok_or_else(|| Error::other("The pack does not declare a Minecraft version."))?;
    let loader = modpack::loader_from_dependencies(&index.dependencies)?;

    let staged = state
        .paths
        .root
        .join("cache")
        .join("modpacks")
        .join(format!("import-{}.zip", uuid::Uuid::new_v4()));
    state.files.copy_external_into(path, &staged).await?;

    let base = name_override
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| index.name.clone());
    let name = modpack::unique_instance_name(state, &base)?;

    let id = uuid::Uuid::new_v4().to_string();
    let instance = Instance {
        dir: state.paths.instance_dir(&id).display().to_string(),
        logo: None,
        id,
        name,
        version_id: game_version,
        created_at: chrono::Utc::now(),
        min_memory_mb: None,
        max_memory_mb: None,
        java_path: None,
        last_played_at: None,
        playtime_secs: 0,
        loader: loader.as_ref().map(|(name, _)| name.clone()),
        loader_version: loader.as_ref().map(|(_, version)| version.clone()),
        launch_version_id: None,
        pack_provider: None,
        pack_project_id: None,
        pack_version_id: None,
        import_source: None,
        import_source_id: None,
        banner_id: None,
        jvm_args: None,
        jvm_args_mode: None,
        env_vars: None,
        env_vars_mode: None,
    };
    let instance_dir = state.paths.instance_dir(&instance.id);
    state.files.ensure_dir(&instance_dir)?;
    state.db.insert_instance(&instance)?;

    let task = state.tasks.start(
        app,
        TaskKind::ModpackInstall,
        TaskSpec {
            title: instance.name.clone(),
            subtitle: Some(format!(
                "{}{}",
                instance.version_id,
                instance
                    .loader
                    .as_deref()
                    .map(|loader| format!(" · {loader}"))
                    .unwrap_or_default()
            )),
            instance_id: Some(instance.id.clone()),
            ..Default::default()
        },
    );

    Ok(PreparedImport {
        instance,
        instance_dir,
        staged,
        index,
        links,
        skipped,
        task,
    })
}

pub async fn finish_import(app: &AppHandle, state: &AppState, prepared: PreparedImport) {
    let PreparedImport {
        instance,
        instance_dir,
        staged,
        index,
        links,
        skipped,
        task,
    } = prepared;

    let outcome = modpack::install_pack_body(
        app,
        state,
        None,
        &instance,
        &instance_dir,
        &staged,
        &index,
        &task,
    )
    .await;

    let _ = state.files.remove_file_if_exists(&staged);

    let artifacts = match outcome {
        Ok(artifacts) => artifacts,
        Err(error) => {
            let _ = state.db.delete_instance_content_files(&instance.id);
            let _ = state.db.delete_instance(&instance.id);
            let _ = state.files.remove_instance_dir(&instance.id);
            match &error {
                Error::Cancelled => task.cancelled(),
                other => task.fail(other),
            }
            return;
        }
    };

    let persist = (|| {
        state
            .db
            .set_launch_version(&instance.id, &artifacts.launch_id)?;
        for (kind, _, file) in &links {
            state.db.record_content_file(&instance.id, kind, file)?;
        }
        Result::<()>::Ok(())
    })();
    if let Err(error) = persist {
        let _ = state.db.delete_instance_content_files(&instance.id);
        let _ = state.db.delete_instance(&instance.id);
        let _ = state.files.remove_instance_dir(&instance.id);
        task.fail(&error);
        return;
    }
    modpack::link_pack_files(state, &instance.id, &artifacts.linkable).await;
    if !skipped.is_empty() {
        tracing::warn!(count = skipped.len(), "pack files without a download url");
    }

    task.succeed();
}

#[cfg(test)]
mod tests {
    use super::{directory_for, split_loader};

    #[test]
    fn parses_curseforge_loader_ids() {
        assert_eq!(
            split_loader("forge-14.23.5.2847"),
            Some(("forge".to_string(), "14.23.5.2847".to_string()))
        );
        assert_eq!(
            split_loader("neoforge-21.0.14"),
            Some(("neoforge".to_string(), "21.0.14".to_string()))
        );
        assert_eq!(split_loader("liteloader-1.2"), None);
        assert_eq!(split_loader("forge"), None);
    }

    #[test]
    fn routes_files_by_class() {
        assert_eq!(directory_for(Some(12), "pack.zip"), "resourcepacks");
        assert_eq!(directory_for(Some(6552), "shader.zip"), "shaderpacks");
        assert_eq!(directory_for(None, "sodium.jar"), "mods");
    }
}
