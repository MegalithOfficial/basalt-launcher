use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Deserialize;

use crate::{
    config::Instance,
    db::{ContentFile, Db},
    error::{Error, Result},
    files::FileManager,
    search::VersionDependency,
    tasks::TaskHandle,
};

use super::{
    candidate_roots, relative_within, walk_files, LauncherKind, LauncherSource, MigrationCandidate,
    MigrationOutcome, MigrationScan,
};

const DIR_NAMES: [&str; 2] = ["atlauncher", "ATLauncher"];
const MAX_ICON_BYTES: u64 = 4 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;

/// Launcher bookkeeping that means nothing once the instance lives in Basalt.
const SKIP_ENTRIES: [&str; 9] = [
    "instance.json",
    "instance.png",
    "downloads",
    "logs",
    "command_history.txt",
    "debug-profile.json",
    ".mods",
    ".fabric",
    "temp",
];

#[derive(Debug, Deserialize)]
struct InstanceFile {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    launcher: LauncherBlock,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LauncherBlock {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    loader_version: Option<LoaderVersion>,
    #[serde(default)]
    modrinth_project: Option<ModrinthProject>,
    #[serde(default)]
    modrinth_version: Option<ModrinthVersion>,
    #[serde(default)]
    curse_forge_project: Option<CurseForgeProject>,
    #[serde(default)]
    curse_forge_file: Option<CurseForgeFile>,
    #[serde(default)]
    last_played: Option<i64>,
    #[serde(default)]
    mods: Vec<ModEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoaderVersion {
    #[serde(default)]
    version: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModEntry {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    user_added: bool,
    #[serde(default)]
    curse_forge_project_id: Option<i64>,
    #[serde(default)]
    curse_forge_file_id: Option<i64>,
    #[serde(default)]
    modrinth_project: Option<ModrinthProject>,
    #[serde(default)]
    modrinth_version: Option<ModrinthVersion>,
    #[serde(default)]
    curse_forge_project: Option<CurseForgeProject>,
    #[serde(default)]
    curse_forge_file: Option<CurseForgeFile>,
}

#[derive(Debug, Deserialize)]
struct ModrinthProject {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    icon_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModrinthVersion {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    version_number: Option<String>,
    #[serde(default)]
    files: Vec<ModrinthFile>,
    #[serde(default)]
    dependencies: Vec<ModrinthDependency>,
}

#[derive(Debug, Deserialize)]
struct ModrinthFile {
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: ModrinthHashes,
}

#[derive(Debug, Default, Deserialize)]
struct ModrinthHashes {
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    sha512: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModrinthDependency {
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    version_id: Option<String>,
    #[serde(default)]
    dependency_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CurseForgeProject {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    logo: Option<CurseForgeLogo>,
}

#[derive(Debug, Deserialize)]
struct CurseForgeLogo {
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeFile {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    file_fingerprint: Option<i64>,
    #[serde(default)]
    hashes: Vec<CurseForgeHash>,
    #[serde(default)]
    dependencies: Vec<CurseForgeDependency>,
}

#[derive(Debug, Deserialize)]
struct CurseForgeHash {
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    algo: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeDependency {
    #[serde(default)]
    mod_id: Option<i64>,
    #[serde(default)]
    file_id: Option<i64>,
    #[serde(default)]
    relation_type: i64,
}

/// CurseForge relation types worth carrying over; the rest describe bundled or
/// incompatible files, which are not dependencies our resolver acts on.
fn curseforge_relation(relation: i64) -> Option<&'static str> {
    match relation {
        3 => Some("required"),
        2 => Some("optional"),
        _ => None,
    }
}

fn loader_from(kind: &str) -> Option<&'static str> {
    match kind.to_ascii_lowercase().as_str() {
        "fabric" => Some("fabric"),
        "quilt" => Some("quilt"),
        "forge" => Some("forge"),
        "neoforge" => Some("neoforge"),
        _ => None,
    }
}

fn content_kind_from(entry: &ModEntry) -> Option<&'static str> {
    let hint = entry
        .path
        .as_deref()
        .or(entry.kind.as_deref())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match hint.trim_end_matches('s') {
        "mod" => Some("mods"),
        "resourcepack" => Some("resourcepacks"),
        "shaderpack" => Some("shaderpacks"),
        _ => None,
    }
}

/// ATLauncher records the pack an instance came from, which maps onto the same fields
/// Basalt fills in when it installs a modpack itself.
fn pack_source(launcher: &LauncherBlock) -> Option<(&'static str, String, Option<String>)> {
    if let Some(project) = launcher
        .modrinth_project
        .as_ref()
        .and_then(|project| project.id.clone())
    {
        return Some((
            "modrinth",
            project,
            launcher
                .modrinth_version
                .as_ref()
                .and_then(|version| version.id.clone()),
        ));
    }
    let project = launcher
        .curse_forge_project
        .as_ref()
        .and_then(|project| project.id)?;
    Some((
        "curseforge",
        project.to_string(),
        launcher
            .curse_forge_file
            .as_ref()
            .and_then(|file| file.id)
            .map(|id| id.to_string()),
    ))
}

fn instances_dir(root: &Path) -> PathBuf {
    root.join("instances")
}

pub fn detect(files: &FileManager) -> Option<LauncherSource> {
    for root in candidate_roots(&DIR_NAMES) {
        let instances = instances_dir(&root);
        let Ok(entries) = files.read_external_dir(&instances) else {
            continue;
        };
        let count = entries
            .iter()
            .filter(|path| files.is_external_file(path.join("instance.json")))
            .count();
        if count == 0 {
            continue;
        }
        return Some(LauncherSource {
            kind: LauncherKind::Atlauncher,
            label: "ATLauncher".to_string(),
            root: root.display().to_string(),
            instance_count: count,
        });
    }
    None
}

fn read_manifest(files: &FileManager, dir: &Path) -> Result<InstanceFile> {
    let path = dir.join("instance.json");
    let metadata = files.external_symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return Err(Error::other("instance.json is not a readable manifest"));
    }
    let bytes = files.read_external(&path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| Error::other(format!("instance.json could not be read: {error}")))
}

fn read_icon(files: &FileManager, dir: &Path) -> Option<String> {
    let path = dir.join("instance.png");
    let metadata = files.external_symlink_metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_ICON_BYTES {
        return None;
    }
    let bytes = files.read_external(&path).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn skip_entry(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| SKIP_ENTRIES.contains(&name))
        .unwrap_or(false)
}

fn candidate_for(files: &FileManager, dir: &Path) -> MigrationCandidate {
    let id = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();

    let mut warnings = Vec::new();
    let manifest = match read_manifest(files, dir) {
        Ok(manifest) => manifest,
        Err(error) => {
            return MigrationCandidate {
                name: id.clone(),
                id,
                version_id: String::new(),
                loader: None,
                loader_version: None,
                icon_data_url: None,
                pack: None,
                mod_count: 0,
                file_count: 0,
                total_bytes: 0,
                last_played_ms: None,
                warnings: vec![error.to_string()],
                importable: false,
                imported: false,
            };
        }
    };

    let version_id = manifest.id.clone().unwrap_or_default();
    if version_id.is_empty() {
        warnings.push("No Minecraft version recorded.".to_string());
    }

    let mut loader = None;
    let mut loader_version = None;
    if let Some(found) = manifest.launcher.loader_version.as_ref() {
        match found.kind.as_deref().and_then(loader_from) {
            Some(name) => {
                loader = Some(name.to_string());
                loader_version = found.version.clone();
            }
            None => warnings.push(format!(
                "Unsupported loader {}, importing without it.",
                found.kind.as_deref().unwrap_or("unknown")
            )),
        }
    }

    let entries = walk_files(files, dir, &skip_entry).unwrap_or_default();
    let total_bytes = entries.iter().map(|(_, size)| size).sum();

    MigrationCandidate {
        name: manifest
            .launcher
            .name
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| id.clone()),
        id,
        version_id: version_id.clone(),
        loader,
        loader_version,
        icon_data_url: read_icon(files, dir),
        pack: pack_source(&manifest.launcher).map(|(provider, _, _)| provider.to_string()),
        mod_count: manifest.launcher.mods.len(),
        file_count: entries.len(),
        total_bytes,
        last_played_ms: manifest.launcher.last_played.filter(|value| *value > 0),
        importable: !version_id.is_empty(),
        imported: false,
        warnings,
    }
}

pub fn scan(files: &FileManager, root: &Path) -> Result<MigrationScan> {
    let instances = instances_dir(root);
    let entries = files
        .read_external_dir(&instances)
        .map_err(|_| Error::other(format!("no instances folder under {}", root.display())))?;

    let mut candidates: Vec<MigrationCandidate> = entries
        .into_iter()
        .filter(|path| files.is_external_file(path.join("instance.json")))
        .map(|path| candidate_for(files, &path))
        .collect();
    candidates.sort_by(|a, b| {
        b.last_played_ms
            .unwrap_or(0)
            .cmp(&a.last_played_ms.unwrap_or(0))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(MigrationScan {
        kind: LauncherKind::Atlauncher,
        root: root.display().to_string(),
        candidates,
    })
}

fn record_mods(db: &Db, instance_id: &str, manifest: &InstanceFile, pack_version: Option<&str>) {
    let now = chrono::Utc::now().timestamp();
    for entry in &manifest.launcher.mods {
        let (Some(file_name), Some(kind)) = (entry.file.clone(), content_kind_from(entry)) else {
            continue;
        };

        let modrinth_version = entry.modrinth_version.as_ref();
        let modrinth_file = modrinth_version.and_then(|version| {
            version
                .files
                .iter()
                .find(|file| file.filename.as_deref() == Some(file_name.as_str()))
                .or_else(|| version.files.iter().find(|file| file.primary))
                .or_else(|| version.files.first())
        });
        let curse_file = entry.curse_forge_file.as_ref();

        let modrinth_project = entry.modrinth_project.as_ref().and_then(|p| p.id.clone());
        let (provider, project_id, version_id) = if let Some(project) = modrinth_project {
            (
                Some("modrinth"),
                Some(project),
                modrinth_version.and_then(|version| version.id.clone()),
            )
        } else if let Some(project) = entry.curse_forge_project_id {
            (
                Some("curseforge"),
                Some(project.to_string()),
                entry.curse_forge_file_id.map(|id| id.to_string()),
            )
        } else {
            (None, None, None)
        };

        let dependencies = match provider {
            Some("modrinth") => modrinth_version.map(|version| {
                version
                    .dependencies
                    .iter()
                    .filter_map(|dependency| {
                        Some(VersionDependency {
                            project_id: dependency.project_id.clone()?,
                            version_id: dependency.version_id.clone(),
                            dependency_type: dependency
                                .dependency_type
                                .clone()
                                .unwrap_or_else(|| "required".to_string()),
                        })
                    })
                    .collect::<Vec<_>>()
            }),
            Some("curseforge") => curse_file.map(|file| {
                file.dependencies
                    .iter()
                    .filter_map(|dependency| {
                        Some(VersionDependency {
                            project_id: dependency.mod_id?.to_string(),
                            version_id: dependency
                                .file_id
                                .filter(|id| *id > 0)
                                .map(|id| id.to_string()),
                            dependency_type: curseforge_relation(dependency.relation_type)?
                                .to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            }),
            _ => None,
        }
        .filter(|list: &Vec<VersionDependency>| !list.is_empty())
        .and_then(|list| serde_json::to_string(&list).ok());

        let sha1 = modrinth_file
            .and_then(|file| file.hashes.sha1.clone())
            .or_else(|| {
                curse_file.and_then(|file| {
                    file.hashes
                        .iter()
                        .find(|hash| hash.algo == 1)
                        .and_then(|hash| hash.value.clone())
                })
            });

        let record = ContentFile {
            file_name,
            sha1,
            sha512: modrinth_file.and_then(|file| file.hashes.sha512.clone()),
            murmur2: curse_file.and_then(|file| file.file_fingerprint),
            provider: provider.map(str::to_string),
            project_id,
            version_id,
            title: entry
                .modrinth_project
                .as_ref()
                .and_then(|project| project.title.clone())
                .or_else(|| {
                    entry
                        .curse_forge_project
                        .as_ref()
                        .and_then(|project| project.name.clone())
                })
                .or_else(|| entry.name.clone()),
            icon_url: entry
                .modrinth_project
                .as_ref()
                .and_then(|project| project.icon_url.clone())
                .or_else(|| {
                    entry
                        .curse_forge_project
                        .as_ref()
                        .and_then(|project| project.logo.as_ref())
                        .and_then(|logo| logo.url.clone())
                }),
            mod_id: None,
            mod_version: modrinth_version
                .and_then(|version| version.version_number.clone())
                .or_else(|| curse_file.and_then(|file| file.display_name.clone()))
                .or_else(|| entry.version.clone())
                .filter(|value| !value.trim().is_empty()),
            dependencies,
            origin: if entry.user_added { "user" } else { "pack" }.to_string(),
            pack_version_id: if entry.user_added {
                None
            } else {
                pack_version.map(str::to_string)
            },
            installed_at: now,
        };
        let _ = db.record_content_file(instance_id, kind, &record);
    }
}

fn copy_instance(
    files: &FileManager,
    source: &Path,
    destination: &Path,
    entries: &[(PathBuf, u64)],
    task: &TaskHandle,
    done: &mut u64,
    total: u64,
) -> Result<()> {
    for (path, size) in entries {
        if task.token().is_cancelled() {
            return Err(Error::Cancelled);
        }
        let Some(relative) = relative_within(source, path) else {
            continue;
        };
        let target = destination.join(&relative);
        if let Some(parent) = target.parent() {
            files.ensure_dir(parent)?;
        }
        files.copy_external_into_sync(path, &target)?;
        *done += size;
        task.progress(*done, total, *done, total);
    }
    Ok(())
}

fn disabled_target(name: &str) -> String {
    if name.ends_with(".disabled") {
        name.to_string()
    } else {
        format!("{name}.disabled")
    }
}

/// ATLauncher parks turned-off mods in a sibling folder; Basalt marks them in place.
fn fold_disabled_mods(files: &FileManager, source: &Path, destination: &Path) -> Result<()> {
    let from = source.join("disabledmods");
    let Ok(entries) = files.read_external_dir(&from) else {
        return Ok(());
    };
    let into = destination.join("mods");
    if entries.is_empty() {
        return Ok(());
    }
    files.ensure_dir(&into)?;

    for path in entries {
        let Ok(metadata) = files.external_symlink_metadata(&path) else {
            continue;
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        files.copy_external_into_sync(&path, into.join(disabled_target(name)))?;
    }
    Ok(())
}

pub fn import(
    files: &FileManager,
    db: &Db,
    root: &Path,
    ids: &[String],
    task: &TaskHandle,
) -> Result<MigrationOutcome> {
    let instances = instances_dir(root);

    let mut planned = Vec::new();
    let mut total_bytes = 0u64;
    for id in ids {
        let source = instances.join(id);
        if relative_within(&instances, &source).is_none() {
            return Err(Error::other(format!("not an instance folder: {id}")));
        }
        let manifest = read_manifest(files, &source)?;
        let entries = walk_files(files, &source, &skip_entry)?;
        total_bytes += entries.iter().map(|(_, size)| size).sum::<u64>();
        planned.push((id.clone(), source, manifest, entries));
    }

    task.stage("copying");
    let mut outcome = MigrationOutcome {
        imported: Vec::new(),
        failed: Vec::new(),
    };
    let mut done = 0u64;

    for (id, source, manifest, entries) in planned {
        if task.token().is_cancelled() {
            return Err(Error::Cancelled);
        }
        let instance_id = uuid::Uuid::new_v4().to_string();
        let destination = files.paths().instance_dir(&instance_id);

        let pack = pack_source(&manifest.launcher);
        let result = (|| -> Result<()> {
            files.ensure_dir(&destination)?;
            copy_instance(
                files,
                &source,
                &destination,
                &entries,
                task,
                &mut done,
                total_bytes,
            )?;
            fold_disabled_mods(files, &source, &destination)?;

            let loader = manifest
                .launcher
                .loader_version
                .as_ref()
                .and_then(|found| found.kind.as_deref().and_then(loader_from));
            let instance = Instance {
                id: instance_id.clone(),
                name: manifest
                    .launcher
                    .name
                    .clone()
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| id.clone()),
                version_id: manifest
                    .id
                    .clone()
                    .ok_or_else(|| Error::other("instance has no Minecraft version"))?,
                created_at: chrono::Utc::now(),
                min_memory_mb: None,
                max_memory_mb: None,
                java_path: None,
                last_played_at: manifest
                    .launcher
                    .last_played
                    .filter(|value| *value > 0)
                    .map(|value| value / 1000),
                playtime_secs: 0,
                dir: destination.display().to_string(),
                logo: None,
                loader: loader.map(str::to_string),
                loader_version: loader.and(
                    manifest
                        .launcher
                        .loader_version
                        .as_ref()
                        .and_then(|found| found.version.clone()),
                ),
                launch_version_id: None,
                pack_provider: pack.as_ref().map(|(provider, _, _)| provider.to_string()),
                pack_project_id: pack.as_ref().map(|(_, project, _)| project.clone()),
                pack_version_id: pack.as_ref().and_then(|(_, _, version)| version.clone()),
                import_source: Some("atlauncher".to_string()),
                import_source_id: Some(id.clone()),
                jvm_args: None,
                jvm_args_mode: None,
                env_vars: None,
                env_vars_mode: None,
            };
            db.insert_instance(&instance)?;
            record_mods(
                db,
                &instance_id,
                &manifest,
                pack.as_ref().and_then(|(_, _, version)| version.as_deref()),
            );
            Ok(())
        })();

        match result {
            Ok(()) => {
                let icon = source.join("instance.png");
                if files.is_external_file(&icon) {
                    if let Ok(bytes) = files.read_external(&icon) {
                        let _ = crate::meta::media::write_instance_logo_sync(
                            files,
                            &instance_id,
                            "png",
                            &bytes,
                        );
                    }
                }
                tracing::info!(instance = %instance_id, source = %id, "instance migrated");
                outcome.imported.push(instance_id);
            }
            Err(error) => {
                let _ = files.remove_instance_dir(&instance_id);
                let _ = db.delete_instance(&instance_id);
                if matches!(error, Error::Cancelled) {
                    return Err(error);
                }
                tracing::warn!(source = %id, error = %error, "instance migration failed");
                outcome.failed.push((id, error.to_string()));
            }
        }
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "id": "1.21.1",
        "launcher": {
            "name": "Better Adventures++",
            "modrinthProject": { "id": "23niDfW7", "title": "Better Adventures++" },
            "modrinthVersion": { "id": "R03FRq68" },
            "loaderVersion": { "version": "0.16.10", "type": "Fabric" },
            "lastPlayed": 1760199564161,
            "mods": [
                {
                    "name": "Sodium",
                    "file": "sodium.jar",
                    "path": "mods",
                    "type": "mods",
                    "userAdded": false,
                    "modrinthProject": { "id": "AANobbMI", "title": "Sodium" },
                    "modrinthVersion": {
                        "id": "abc123",
                        "version_number": "0.6.0",
                        "files": [
                            {
                                "filename": "sodium.jar",
                                "primary": true,
                                "hashes": { "sha1": "aaa", "sha512": "bbb" }
                            }
                        ],
                        "dependencies": [
                            { "project_id": "P7dR8mSH", "dependency_type": "required" },
                            { "project_id": "9s6osm5g", "dependency_type": "optional" }
                        ]
                    }
                },
                {
                    "name": "Faithful",
                    "file": "faithful.zip",
                    "path": "resourcepacks",
                    "type": "resourcepack",
                    "userAdded": true,
                    "curseForgeProjectId": 1026394,
                    "curseForgeFileId": 8262610,
                    "curseForgeProject": { "name": "Faithful", "logo": { "url": "https://x/y.png" } },
                    "curseForgeFile": {
                        "displayName": "Faithful 1.21",
                        "fileFingerprint": 3626709559,
                        "hashes": [
                            { "value": "ccc", "algo": 1 },
                            { "value": "ddd", "algo": 2 }
                        ],
                        "dependencies": [
                            { "modId": 348521, "fileId": 0, "relationType": 2 },
                            { "modId": 306612, "fileId": 77, "relationType": 3 },
                            { "modId": 999, "fileId": 0, "relationType": 5 }
                        ]
                    }
                }
            ]
        }
    }"#;

    #[test]
    fn manifest_maps_to_instance_fields() {
        let parsed: InstanceFile = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(parsed.id.as_deref(), Some("1.21.1"));
        assert_eq!(parsed.launcher.name.as_deref(), Some("Better Adventures++"));
        let loader = parsed.launcher.loader_version.as_ref().unwrap();
        assert_eq!(loader.kind.as_deref().and_then(loader_from), Some("fabric"));
        assert_eq!(loader.version.as_deref(), Some("0.16.10"));
        assert_eq!(parsed.launcher.mods.len(), 2);
    }

    #[test]
    fn mods_keep_their_provider_identity() {
        let parsed: InstanceFile = serde_json::from_str(SAMPLE).unwrap();
        let sodium = &parsed.launcher.mods[0];
        assert_eq!(content_kind_from(sodium), Some("mods"));
        assert_eq!(
            sodium.modrinth_project.as_ref().unwrap().id.as_deref(),
            Some("AANobbMI")
        );

        let faithful = &parsed.launcher.mods[1];
        assert_eq!(content_kind_from(faithful), Some("resourcepacks"));
        assert_eq!(faithful.curse_forge_project_id, Some(1026394));
        assert!(faithful.user_added);
    }

    #[test]
    fn modrinth_mods_carry_hashes_and_dependencies() {
        let parsed: InstanceFile = serde_json::from_str(SAMPLE).unwrap();
        let version = parsed.launcher.mods[0].modrinth_version.as_ref().unwrap();
        assert_eq!(version.version_number.as_deref(), Some("0.6.0"));
        assert_eq!(version.files[0].hashes.sha1.as_deref(), Some("aaa"));
        assert_eq!(version.files[0].hashes.sha512.as_deref(), Some("bbb"));
        assert_eq!(version.dependencies.len(), 2);
        assert_eq!(
            version.dependencies[0].dependency_type.as_deref(),
            Some("required")
        );
    }

    #[test]
    fn curseforge_relations_map_to_ours() {
        let parsed: InstanceFile = serde_json::from_str(SAMPLE).unwrap();
        let file = parsed.launcher.mods[1].curse_forge_file.as_ref().unwrap();
        assert_eq!(file.file_fingerprint, Some(3626709559));
        assert_eq!(
            file.hashes
                .iter()
                .find(|h| h.algo == 1)
                .unwrap()
                .value
                .as_deref(),
            Some("ccc")
        );
        let kept: Vec<_> = file
            .dependencies
            .iter()
            .filter_map(|d| curseforge_relation(d.relation_type))
            .collect();
        assert_eq!(kept, vec!["optional", "required"]);
    }

    #[test]
    fn a_pack_instance_keeps_its_origin() {
        let parsed: InstanceFile = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(
            pack_source(&parsed.launcher),
            Some((
                "modrinth",
                "23niDfW7".to_string(),
                Some("R03FRq68".to_string())
            ))
        );

        let curseforge: InstanceFile = serde_json::from_str(
            r#"{ "id": "1.20.1", "launcher": {
                "curseForgeProject": { "id": 1237007 },
                "curseForgeFile": { "id": 6728718 }
            } }"#,
        )
        .unwrap();
        assert_eq!(
            pack_source(&curseforge.launcher),
            Some((
                "curseforge",
                "1237007".to_string(),
                Some("6728718".to_string())
            ))
        );

        let vanilla: InstanceFile =
            serde_json::from_str(r#"{ "id": "1.20.1", "launcher": { "name": "Doc SMP" } }"#)
                .unwrap();
        assert_eq!(pack_source(&vanilla.launcher), None);
    }

    #[test]
    fn unknown_loaders_are_not_guessed() {
        assert_eq!(loader_from("NeoForge"), Some("neoforge"));
        assert_eq!(loader_from("legacyfabric"), None);
    }

    #[test]
    fn launcher_bookkeeping_is_left_behind() {
        assert!(skip_entry(Path::new("/x/instance.json")));
        assert!(skip_entry(Path::new("/x/logs")));
        assert!(!skip_entry(Path::new("/x/mods")));
        assert!(!skip_entry(Path::new("/x/saves")));
    }

    #[test]
    fn copies_stay_inside_the_source_tree() {
        let root = Path::new("/data/instances");
        assert_eq!(
            relative_within(root, Path::new("/data/instances/pack/mods/a.jar")),
            Some(PathBuf::from("pack/mods/a.jar"))
        );
        assert_eq!(relative_within(root, Path::new("/etc/passwd")), None);
        assert_eq!(disabled_target("a.jar"), "a.jar.disabled");
        assert_eq!(disabled_target("a.jar.disabled"), "a.jar.disabled");
    }
}
