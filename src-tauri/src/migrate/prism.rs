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

const DIR_NAMES: [&str; 6] = [
    "PrismLauncher",
    "prismlauncher",
    "PolyMC",
    "polymc",
    "MultiMC",
    "multimc",
];

const GAME_DIRS: [&str; 2] = [".minecraft", "minecraft"];

const LAUNCHER_CONFIGS: [&str; 3] = ["prismlauncher.cfg", "polymc.cfg", "multimc.cfg"];

const ICON_EXTENSIONS: [&str; 4] = ["png", "webp", "jpg", "jpeg"];
const MAX_ICON_BYTES: u64 = 4 * 1024 * 1024;
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Default)]
struct Config(Vec<(String, String)>);

impl Config {
    fn parse(text: &str) -> Self {
        Self(
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('[') && !line.starts_with('#'))
                .filter_map(|line| line.split_once('='))
                .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
                .collect(),
        )
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
            .filter(|value| !value.is_empty())
    }

    fn flag(&self, key: &str) -> bool {
        matches!(self.get(key), Some("true"))
    }

    fn number(&self, key: &str) -> Option<i64> {
        self.get(key)?.parse().ok()
    }

    fn overridden(&self, flag: &str, key: &str) -> Option<&str> {
        self.flag(flag).then(|| self.get(key)).flatten()
    }
}

#[derive(Debug, Deserialize)]
struct ComponentPack {
    #[serde(default)]
    components: Vec<Component>,
}

#[derive(Debug, Deserialize)]
struct Component {
    #[serde(default)]
    uid: String,
    #[serde(default)]
    version: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct PackwizMod {
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    download: Option<PackwizDownload>,
    #[serde(default)]
    update: Option<PackwizUpdate>,
    #[serde(default, rename = "x-prismlauncher-version-number")]
    version_number: Option<String>,
    #[serde(default, rename = "x-prismlauncher-dependencies")]
    dependencies: Vec<PackwizDependency>,
}

#[derive(Debug, Deserialize)]
struct PackwizDownload {
    #[serde(default)]
    hash: Option<String>,
    #[serde(default, rename = "hash-format")]
    hash_format: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct PackwizUpdate {
    #[serde(default)]
    modrinth: Option<ModrinthUpdate>,
    #[serde(default)]
    curseforge: Option<CurseForgeUpdate>,
}

#[derive(Debug, Deserialize)]
struct ModrinthUpdate {
    #[serde(default, rename = "mod-id")]
    mod_id: Option<String>,
    #[serde(default)]
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CurseForgeUpdate {
    #[serde(default, rename = "project-id")]
    project_id: Option<i64>,
    #[serde(default, rename = "file-id")]
    file_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct PackwizDependency {
    #[serde(default, rename = "addonId")]
    addon_id: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

fn loader_from(uid: &str) -> Option<&'static str> {
    match uid {
        "net.fabricmc.fabric-loader" => Some("fabric"),
        "org.quiltmc.quilt-loader" => Some("quilt"),
        "net.minecraftforge" => Some("forge"),
        "net.neoforged" => Some("neoforge"),
        _ => None,
    }
}

fn instances_dir(files: &FileManager, root: &Path) -> PathBuf {
    for name in LAUNCHER_CONFIGS {
        let Ok(config) = read_config(files, &root.join(name)) else {
            continue;
        };
        let Some(configured) = config.get("InstanceDir").map(PathBuf::from) else {
            continue;
        };
        return if configured.is_absolute() {
            configured
        } else {
            root.join(configured)
        };
    }
    root.join("instances")
}

fn game_dir(files: &FileManager, instance: &Path) -> Option<PathBuf> {
    GAME_DIRS
        .iter()
        .map(|name| instance.join(name))
        .find(|path| {
            files
                .external_symlink_metadata(path)
                .map(|meta| meta.is_dir())
                .unwrap_or(false)
        })
}

fn read_config(files: &FileManager, path: &Path) -> Result<Config> {
    let metadata = files.external_symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_CONFIG_BYTES {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("config");
        return Err(Error::other(format!("{name} is not a readable config")));
    }
    let bytes = files.read_external(path)?;
    Ok(Config::parse(&String::from_utf8_lossy(&bytes)))
}

fn read_instance_config(files: &FileManager, dir: &Path) -> Result<Config> {
    read_config(files, &dir.join("instance.cfg"))
}

fn read_components(files: &FileManager, dir: &Path) -> Option<ComponentPack> {
    let bytes = files.read_external(dir.join("mmc-pack.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn read_icon(files: &FileManager, root: &Path, key: Option<&str>) -> Option<String> {
    let key = key.filter(|key| *key != "default")?;
    for extension in ICON_EXTENSIONS {
        let path = root.join("icons").join(format!("{key}.{extension}"));
        let Ok(metadata) = files.external_symlink_metadata(&path) else {
            continue;
        };
        if !metadata.is_file() || metadata.len() > MAX_ICON_BYTES {
            continue;
        }
        let Ok(bytes) = files.read_external(&path) else {
            continue;
        };
        let mime = if extension == "webp" { "webp" } else { "png" };
        return Some(format!(
            "data:image/{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ));
    }
    None
}

fn pack_source(config: &Config) -> Option<(&'static str, String, Option<String>)> {
    let provider = match config.get("ManagedPackType")? {
        "modrinth" => "modrinth",
        "flame" | "curseforge" => "curseforge",
        _ => return None,
    };
    Some((
        provider,
        config.get("ManagedPackID")?.to_string(),
        config.get("ManagedPackVersionID").map(str::to_string),
    ))
}

pub fn detect(files: &FileManager) -> Option<LauncherSource> {
    for root in candidate_roots(&DIR_NAMES) {
        let Ok(entries) = files.read_external_dir(instances_dir(files, &root)) else {
            continue;
        };
        let count = entries
            .iter()
            .filter(|path| files.is_external_file(path.join("instance.cfg")))
            .count();
        if count == 0 {
            continue;
        }
        let label = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Prism Launcher")
            .to_string();
        return Some(LauncherSource {
            kind: LauncherKind::Prism,
            label: if label.eq_ignore_ascii_case("prismlauncher") {
                "Prism Launcher".to_string()
            } else {
                label
            },
            root: root.display().to_string(),
            instance_count: count,
        });
    }
    None
}

fn candidate_for(files: &FileManager, root: &Path, dir: &Path) -> MigrationCandidate {
    let id = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();

    let mut warnings = Vec::new();
    let config = match read_instance_config(files, dir) {
        Ok(config) => config,
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

    let components = read_components(files, dir);
    let mut version_id = String::new();
    let mut loader = None;
    let mut loader_version = None;

    match components {
        Some(pack) => {
            for component in &pack.components {
                if component.uid == "net.minecraft" {
                    version_id = component.version.clone().unwrap_or_default();
                } else if let Some(name) = loader_from(&component.uid) {
                    loader = Some(name.to_string());
                    loader_version = component.version.clone();
                }
            }
        }
        None => warnings.push("mmc-pack.json is missing or unreadable.".to_string()),
    }
    if version_id.is_empty() {
        warnings.push("No Minecraft version recorded.".to_string());
    }

    let game = game_dir(files, dir);
    if game.is_none() {
        warnings.push("No game folder found inside this instance.".to_string());
    }
    let entries = game
        .as_ref()
        .map(|path| walk_files(files, path, &|_| false).unwrap_or_default())
        .unwrap_or_default();
    let mod_count = game
        .as_ref()
        .and_then(|path| files.read_external_dir(path.join("mods")).ok())
        .map(|mods| {
            mods.iter()
                .filter(|path| files.is_external_file(path))
                .count()
        })
        .unwrap_or(0);

    MigrationCandidate {
        name: config.get("name").unwrap_or(&id).to_string(),
        id,
        version_id: version_id.clone(),
        loader,
        loader_version,
        icon_data_url: read_icon(files, root, config.get("iconKey")),
        pack: pack_source(&config).map(|(provider, _, _)| provider.to_string()),
        mod_count,
        file_count: entries.len(),
        total_bytes: entries.iter().map(|(_, size)| size).sum(),
        last_played_ms: config.number("lastLaunchTime").filter(|value| *value > 0),
        importable: !version_id.is_empty() && game.is_some(),
        imported: false,
        warnings,
    }
}

pub fn scan(files: &FileManager, root: &Path) -> Result<MigrationScan> {
    let instances = instances_dir(files, root);
    let entries = files
        .read_external_dir(&instances)
        .map_err(|_| Error::other(format!("no instances folder under {}", root.display())))?;

    let mut candidates: Vec<MigrationCandidate> = entries
        .into_iter()
        .filter(|path| files.is_external_file(path.join("instance.cfg")))
        .map(|path| candidate_for(files, root, &path))
        .collect();
    candidates.sort_by(|a, b| {
        b.last_played_ms
            .unwrap_or(0)
            .cmp(&a.last_played_ms.unwrap_or(0))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(MigrationScan {
        kind: LauncherKind::Prism,
        root: root.display().to_string(),
        candidates,
    })
}

fn record_index(db: &Db, files: &FileManager, instance_id: &str, game: &Path, pack: Option<&str>) {
    let now = chrono::Utc::now().timestamp();
    for kind in ["mods", "resourcepacks", "shaderpacks"] {
        let Ok(entries) = files.read_external_dir(game.join(kind).join(".index")) else {
            continue;
        };
        for path in entries {
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            let Ok(bytes) = files.read_external(&path) else {
                continue;
            };
            let Ok(entry) = toml::from_str::<PackwizMod>(&String::from_utf8_lossy(&bytes)) else {
                continue;
            };
            let Some(file_name) = entry.filename.clone() else {
                continue;
            };

            let update = entry.update.unwrap_or_default();
            let (provider, version_id): (Option<(&str, String)>, Option<String>) =
                if let Some(modrinth) = update.modrinth {
                    (modrinth.mod_id.map(|id| ("modrinth", id)), modrinth.version)
                } else if let Some(curseforge) = update.curseforge {
                    (
                        curseforge
                            .project_id
                            .map(|id| ("curseforge", id.to_string())),
                        curseforge.file_id.map(|id| id.to_string()),
                    )
                } else {
                    (None, None)
                };

            let hash = entry.download.as_ref().and_then(|download| {
                let format = download.hash_format.as_deref()?;
                Some((format.to_string(), download.hash.clone()?))
            });
            let dependencies: Vec<VersionDependency> = entry
                .dependencies
                .iter()
                .filter_map(|dependency| {
                    Some(VersionDependency {
                        project_id: dependency.addon_id.clone()?,
                        version_id: None,
                        dependency_type: match dependency.kind.as_deref() {
                            Some("REQUIRED") | Some("required") => "required".to_string(),
                            Some("OPTIONAL") | Some("optional") => "optional".to_string(),
                            _ => return None,
                        },
                    })
                })
                .collect();

            let record = ContentFile {
                file_name,
                sha1: hash
                    .as_ref()
                    .filter(|(format, _)| format == "sha1")
                    .map(|(_, value)| value.clone()),
                sha512: hash
                    .as_ref()
                    .filter(|(format, _)| format == "sha512")
                    .map(|(_, value)| value.clone()),
                murmur2: hash
                    .as_ref()
                    .filter(|(format, _)| format == "murmur2")
                    .and_then(|(_, value)| value.parse().ok()),
                provider: provider.as_ref().map(|(name, _)| name.to_string()),
                project_id: provider.map(|(_, id)| id),
                version_id,
                title: entry.name.clone(),
                icon_url: None,
                mod_id: None,
                mod_version: entry.version_number.clone(),
                dependencies: (!dependencies.is_empty())
                    .then(|| serde_json::to_string(&dependencies).ok())
                    .flatten(),
                origin: if pack.is_some() { "pack" } else { "user" }.to_string(),
                pack_version_id: pack.map(str::to_string),
                installed_at: now,
            };
            let _ = db.record_content_file(instance_id, kind, &record);
        }
    }
}

pub fn import(
    files: &FileManager,
    db: &Db,
    root: &Path,
    ids: &[String],
    task: &TaskHandle,
) -> Result<MigrationOutcome> {
    let instances = instances_dir(files, root);

    let mut planned = Vec::new();
    let mut total_bytes = 0u64;
    for id in ids {
        let source = instances.join(id);
        if relative_within(&instances, &source).is_none() {
            return Err(Error::other(format!("not an instance folder: {id}")));
        }
        let config = read_instance_config(files, &source)?;
        let game = game_dir(files, &source)
            .ok_or_else(|| Error::other(format!("{id} has no game folder")))?;
        let entries = walk_files(files, &game, &|_| false)?;
        total_bytes += entries.iter().map(|(_, size)| size).sum::<u64>();
        planned.push((id.clone(), source, game, config, entries));
    }

    task.stage("copying");
    let mut outcome = MigrationOutcome {
        imported: Vec::new(),
        failed: Vec::new(),
    };
    let mut done = 0u64;

    for (id, source, game, config, entries) in planned {
        if task.token().is_cancelled() {
            return Err(Error::Cancelled);
        }
        let instance_id = uuid::Uuid::new_v4().to_string();
        let destination = files.paths().instance_dir(&instance_id);
        let pack = pack_source(&config);

        let result = (|| -> Result<()> {
            files.ensure_dir(&destination)?;
            for (path, size) in &entries {
                if task.token().is_cancelled() {
                    return Err(Error::Cancelled);
                }
                let Some(relative) = relative_within(&game, path) else {
                    continue;
                };
                let target = destination.join(&relative);
                if let Some(parent) = target.parent() {
                    files.ensure_dir(parent)?;
                }
                files.copy_external_into_sync(path, &target)?;
                done += size;
                task.progress(done, total_bytes, done, total_bytes);
            }

            let components = read_components(files, &source);
            let mut version_id = String::new();
            let mut loader = None;
            let mut loader_version = None;
            if let Some(pack) = components {
                for component in &pack.components {
                    if component.uid == "net.minecraft" {
                        version_id = component.version.clone().unwrap_or_default();
                    } else if let Some(name) = loader_from(&component.uid) {
                        loader = Some(name.to_string());
                        loader_version = component.version.clone();
                    }
                }
            }
            if version_id.is_empty() {
                return Err(Error::other("instance has no Minecraft version"));
            }

            let instance = Instance {
                id: instance_id.clone(),
                name: config.get("name").unwrap_or(&id).to_string(),
                version_id,
                created_at: chrono::Utc::now(),
                min_memory_mb: config
                    .overridden("OverrideMemory", "MinMemAlloc")
                    .and_then(|value| value.parse().ok()),
                max_memory_mb: config
                    .overridden("OverrideMemory", "MaxMemAlloc")
                    .and_then(|value| value.parse().ok()),
                java_path: config
                    .overridden("OverrideJavaLocation", "JavaPath")
                    .map(str::to_string),
                last_played_at: config
                    .number("lastLaunchTime")
                    .filter(|value| *value > 0)
                    .map(|value| value / 1000),
                playtime_secs: config.number("totalTimePlayed").unwrap_or(0),
                dir: destination.display().to_string(),
                logo: None,
                loader,
                loader_version,
                launch_version_id: None,
                pack_provider: pack.as_ref().map(|(provider, _, _)| provider.to_string()),
                pack_project_id: pack.as_ref().map(|(_, project, _)| project.clone()),
                pack_version_id: pack.as_ref().and_then(|(_, _, version)| version.clone()),
                import_source: Some("prism".to_string()),
                import_source_id: Some(id.clone()),
                banner_id: None,
                notes: None,
                wrapper_command: None,
                pre_launch_command: None,
                post_exit_command: None,
                jvm_args: None,
                jvm_args_mode: None,
                env_vars: None,
                env_vars_mode: None,
            };
            db.insert_instance(&instance)?;
            record_index(
                db,
                files,
                &instance_id,
                &game,
                pack.as_ref().and_then(|(_, _, version)| version.as_deref()),
            );
            Ok(())
        })();

        match result {
            Ok(()) => {
                if let Some(key) = config.get("iconKey").filter(|key| *key != "default") {
                    for extension in ICON_EXTENSIONS {
                        let icon = root.join("icons").join(format!("{key}.{extension}"));
                        if !files.is_external_file(&icon) {
                            continue;
                        }
                        if let Ok(bytes) = files.read_external(&icon) {
                            let _ = crate::meta::media::write_instance_logo_sync(
                                files,
                                &instance_id,
                                extension,
                                &bytes,
                            );
                        }
                        break;
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

    const MANAGED_CFG: &str = r#"[General]
ConfigVersion=1.3
InstanceType=OneSix
ManagedPack=true
ManagedPackID=1KVo5zza
ManagedPackName=Fabulously Optimized
ManagedPackType=modrinth
ManagedPackVersionID=Sq7Ilgdn
ManagedPackVersionName=14.0.0-beta.3 for 26.2
iconKey=modrinth_fabulously-optimized
name=Fabulously Optimized 14.0.0-beta.3 for 26.2
"#;

    const PLAIN_CFG: &str = "[General]\nInstanceType=OneSix\niconKey=default\nname=26.2\n";

    const PACK_JSON: &str = r#"{
        "components": [
            { "cachedName": "Minecraft", "uid": "net.minecraft", "version": "26.2" },
            { "uid": "net.fabricmc.fabric-loader", "version": "0.19.3" }
        ],
        "formatVersion": 1
    }"#;

    const PACKWIZ: &str = r#"
filename = 'animatica-0.6.3+26.2.jar'
name = 'Animatica Refabricated'
side = 'client'
x-prismlauncher-version-number = '0.6.3+26.2'

[download]
hash = '318b29f8'
hash-format = 'sha512'
mode = 'url'
url = 'https://cdn.modrinth.com/data/xEyZuswh/versions/SjuTFuhz/animatica.jar'

[update.modrinth]
mod-id = 'xEyZuswh'
version = 'SjuTFuhz'

[[x-prismlauncher-dependencies]]
addonId = 'P7dR8mSH'
type = 'REQUIRED'
"#;

    #[test]
    fn instance_folder_follows_the_launcher_config() {
        use crate::paths::Paths;

        let base = std::env::temp_dir().join(format!("basalt-prism-{}", uuid::Uuid::new_v4()));
        let root = base.join("PrismLauncher");
        let moved = base.join("games").join("prism-instances");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&moved).unwrap();
        let files = FileManager::new(Paths::plain(base.join("data"))).unwrap();

        assert_eq!(instances_dir(&files, &root), root.join("instances"));

        std::fs::write(
            root.join("prismlauncher.cfg"),
            format!(
                "[General]\nConfigVersion=1.3\nInstanceDir={}\n",
                moved.display()
            ),
        )
        .unwrap();
        assert_eq!(instances_dir(&files, &root), moved);

        std::fs::write(
            root.join("prismlauncher.cfg"),
            "[General]\nInstanceDir=instances\n",
        )
        .unwrap();
        assert_eq!(instances_dir(&files, &root), root.join("instances"));

        std::fs::remove_dir_all(base).ok();
    }

    #[test]
    fn config_reads_flat_keys_under_a_heading() {
        let config = Config::parse(MANAGED_CFG);
        assert_eq!(
            config.get("name"),
            Some("Fabulously Optimized 14.0.0-beta.3 for 26.2")
        );
        assert_eq!(config.get("iconKey"), Some("modrinth_fabulously-optimized"));
        assert_eq!(config.get("missing"), None);
    }

    #[test]
    fn overrides_only_count_when_switched_on() {
        let config =
            Config::parse("OverrideMemory=false\nMaxMemAlloc=8192\nJavaPath=/usr/bin/java");
        assert_eq!(config.overridden("OverrideMemory", "MaxMemAlloc"), None);

        let config = Config::parse("OverrideMemory=true\nMaxMemAlloc=8192");
        assert_eq!(
            config.overridden("OverrideMemory", "MaxMemAlloc"),
            Some("8192")
        );
    }

    #[test]
    fn managed_packs_map_to_our_pack_fields() {
        assert_eq!(
            pack_source(&Config::parse(MANAGED_CFG)),
            Some((
                "modrinth",
                "1KVo5zza".to_string(),
                Some("Sq7Ilgdn".to_string())
            ))
        );
        assert_eq!(pack_source(&Config::parse(PLAIN_CFG)), None);
        assert_eq!(
            pack_source(&Config::parse("ManagedPackType=flame\nManagedPackID=123")),
            Some(("curseforge", "123".to_string(), None))
        );
    }

    #[test]
    fn components_name_the_version_and_loader() {
        let pack: ComponentPack = serde_json::from_str(PACK_JSON).unwrap();
        let minecraft = pack
            .components
            .iter()
            .find(|c| c.uid == "net.minecraft")
            .unwrap();
        assert_eq!(minecraft.version.as_deref(), Some("26.2"));

        let loader = pack
            .components
            .iter()
            .find_map(|c| loader_from(&c.uid).map(|name| (name, c.version.clone())))
            .unwrap();
        assert_eq!(loader.0, "fabric");
        assert_eq!(loader.1.as_deref(), Some("0.19.3"));
        assert_eq!(loader_from("org.lwjgl3"), None);
    }

    #[test]
    fn packwiz_entries_carry_identity_and_dependencies() {
        let entry: PackwizMod = toml::from_str(PACKWIZ).unwrap();
        assert_eq!(entry.filename.as_deref(), Some("animatica-0.6.3+26.2.jar"));
        assert_eq!(entry.name.as_deref(), Some("Animatica Refabricated"));
        assert_eq!(entry.version_number.as_deref(), Some("0.6.3+26.2"));

        let download = entry.download.unwrap();
        assert_eq!(download.hash_format.as_deref(), Some("sha512"));
        assert_eq!(download.hash.as_deref(), Some("318b29f8"));

        let modrinth = entry.update.unwrap().modrinth.unwrap();
        assert_eq!(modrinth.mod_id.as_deref(), Some("xEyZuswh"));
        assert_eq!(modrinth.version.as_deref(), Some("SjuTFuhz"));

        assert_eq!(entry.dependencies.len(), 1);
        assert_eq!(entry.dependencies[0].addon_id.as_deref(), Some("P7dR8mSH"));
        assert_eq!(entry.dependencies[0].kind.as_deref(), Some("REQUIRED"));
    }
}
