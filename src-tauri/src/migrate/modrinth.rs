use std::path::{Path, PathBuf};

use base64::Engine;
use rusqlite::{Connection, OpenFlags};

use crate::{
    config::Instance,
    db::{ContentFile, Db},
    error::{Error, Result},
    files::FileManager,
    tasks::TaskHandle,
};

use super::{
    candidate_roots, relative_within, walk_files, LauncherKind, LauncherSource, MigrationCandidate,
    MigrationOutcome, MigrationScan,
};

const DIR_NAMES: [&str; 3] = ["ModrinthApp", "com.modrinth.theseus", "theseus"];
const MAX_ICON_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
struct Profile {
    id: String,
    path: String,
    name: String,
    icon_path: Option<String>,
    last_played: Option<i64>,
    playtime_secs: i64,
    game_version: String,
    loader: Option<String>,
    loader_version: Option<String>,
    pack_project: Option<String>,
    pack_version: Option<String>,
}

fn loader_from(value: &str) -> Option<&'static str> {
    match value.to_ascii_lowercase().as_str() {
        "fabric" => Some("fabric"),
        "quilt" => Some("quilt"),
        "forge" => Some("forge"),
        "neoforge" => Some("neoforge"),
        _ => None,
    }
}

fn database(root: &Path) -> PathBuf {
    root.join("app.db")
}

fn custom_dir(root: &Path) -> Option<PathBuf> {
    let conn = open(root).ok()?;
    let configured: Option<String> = conn
        .query_row("SELECT custom_dir FROM settings", [], |row| row.get(0))
        .ok()?;
    configured
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
}

fn profiles_dir(root: &Path) -> PathBuf {
    custom_dir(root)
        .unwrap_or_else(|| root.to_path_buf())
        .join("profiles")
}

fn open(root: &Path) -> Result<Connection> {
    Connection::open_with_flags(
        database(root),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|error| Error::other(format!("reading the Modrinth database: {error}")))
}

fn read_profiles(root: &Path) -> Result<Vec<Profile>> {
    let conn = open(root)?;
    let mut statement = conn
        .prepare(
            "SELECT i.id, i.path, i.name, i.icon_path, i.last_played,
                    i.submitted_time_played + i.recent_time_played,
                    s.game_version, s.loader, s.loader_version,
                    l.modrinth_project_id, l.modrinth_version_id
             FROM instances i
             LEFT JOIN instance_content_sets s ON s.instance_id = i.id
             LEFT JOIN instance_links l
                    ON l.instance_id = i.id AND l.link_kind = 'modrinth_modpack'
             GROUP BY i.id",
        )
        .map_err(|error| Error::other(format!("reading Modrinth instances: {error}")))?;

    let rows = statement
        .query_map([], |row| {
            let loader: Option<String> = row.get(7)?;
            Ok(Profile {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                icon_path: row.get(3)?,
                last_played: row.get(4)?,
                playtime_secs: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
                game_version: row.get::<_, Option<String>>(6)?.unwrap_or_default(),
                loader: loader.as_deref().and_then(loader_from).map(str::to_string),
                loader_version: row.get(8)?,
                pack_project: row.get(9)?,
                pack_version: row.get(10)?,
            })
        })
        .map_err(|error| Error::other(format!("reading Modrinth instances: {error}")))?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| Error::other(format!("reading a Modrinth instance row: {error}")))
}

fn read_files(root: &Path, instance_id: &str) -> Result<Vec<(String, String, bool)>> {
    let conn = open(root)?;
    let mut statement = conn
        .prepare("SELECT relative_path, sha1, enabled FROM instance_files WHERE instance_id = ?1")
        .map_err(|error| Error::other(format!("reading Modrinth instance files: {error}")))?;
    let rows = statement
        .query_map([instance_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
            ))
        })
        .map_err(|error| Error::other(format!("reading Modrinth instance files: {error}")))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| Error::other(format!("reading a Modrinth instance file row: {error}")))
}

fn read_icon(files: &FileManager, icon: Option<&str>) -> Option<String> {
    let path = Path::new(icon?);
    let metadata = files.external_symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_ICON_BYTES {
        return None;
    }
    let bytes = files.read_external(path).ok()?;
    let mime = match path.extension().and_then(|ext| ext.to_str()) {
        Some("webp") => "webp",
        Some("jpg") | Some("jpeg") => "jpeg",
        Some("gif") => "gif",
        _ => "png",
    };
    Some(format!(
        "data:image/{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn kind_of(relative: &str) -> Option<&'static str> {
    let folder = relative.split('/').next()?;
    match folder {
        "mods" => Some("mods"),
        "resourcepacks" => Some("resourcepacks"),
        "shaderpacks" => Some("shaderpacks"),
        _ => None,
    }
}

pub fn detect(files: &FileManager) -> Option<LauncherSource> {
    for root in candidate_roots(&DIR_NAMES) {
        if !files.is_external_file(database(&root)) {
            continue;
        }
        let count = read_profiles(&root).map(|list| list.len()).unwrap_or(0);
        if count == 0 {
            continue;
        }
        return Some(LauncherSource {
            kind: LauncherKind::Modrinth,
            label: "Modrinth App".to_string(),
            root: root.display().to_string(),
            instance_count: count,
        });
    }
    None
}

pub fn scan(files: &FileManager, root: &Path) -> Result<MigrationScan> {
    let profiles = profiles_dir(root);
    let mut candidates: Vec<MigrationCandidate> = read_profiles(root)?
        .into_iter()
        .map(|profile| -> Result<MigrationCandidate> {
            let dir = profiles.join(&profile.path);
            let present = files
                .external_symlink_metadata(&dir)
                .map(|meta| meta.is_dir())
                .unwrap_or(false);
            let entries = if present {
                walk_files(files, &dir, &|_| false).unwrap_or_default()
            } else {
                Vec::new()
            };
            let mut warnings = Vec::new();
            if !present {
                warnings.push("The profile folder is missing.".to_string());
            }
            if profile.game_version.is_empty() {
                warnings.push("No Minecraft version recorded.".to_string());
            }

            let mod_count = read_files(root, &profile.id)?
                .iter()
                .filter(|(path, _, _)| path.starts_with("mods/"))
                .count();

            Ok(MigrationCandidate {
                name: profile.name.clone(),
                id: profile.id.clone(),
                version_id: profile.game_version.clone(),
                loader: profile.loader.clone(),
                loader_version: profile.loader_version.clone(),
                icon_data_url: read_icon(files, profile.icon_path.as_deref()),
                pack: profile
                    .pack_project
                    .as_ref()
                    .map(|_| "modrinth".to_string()),
                mod_count,
                file_count: entries.len(),
                total_bytes: entries.iter().map(|(_, size)| size).sum(),
                last_played_ms: profile.last_played.map(|value| value * 1000),
                importable: present && !profile.game_version.is_empty(),
                imported: false,
                warnings,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    candidates.sort_by(|a, b| {
        b.last_played_ms
            .unwrap_or(0)
            .cmp(&a.last_played_ms.unwrap_or(0))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(MigrationScan {
        kind: LauncherKind::Modrinth,
        root: root.display().to_string(),
        candidates,
    })
}

fn record_files(
    db: &Db,
    root: &Path,
    source_id: &str,
    instance_id: &str,
    pack: Option<&str>,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    for (relative, sha1, _enabled) in read_files(root, source_id)? {
        let Some(kind) = kind_of(&relative) else {
            continue;
        };
        let Some(file_name) = relative.split('/').next_back() else {
            continue;
        };
        let record = ContentFile {
            file_name: file_name.trim_end_matches(".disabled").to_string(),
            sha1: Some(sha1),
            sha512: None,
            murmur2: None,
            provider: None,
            project_id: None,
            version_id: None,
            title: None,
            icon_url: None,
            mod_id: None,
            mod_version: None,
            dependencies: None,
            origin: if pack.is_some() { "pack" } else { "user" }.to_string(),
            pack_version_id: pack.map(str::to_string),
            installed_at: now,
        };
        db.record_content_file(instance_id, kind, &record)?;
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
    let profiles_root = profiles_dir(root);
    let known = read_profiles(root)?;

    let mut planned = Vec::new();
    let mut total_bytes = 0u64;
    for id in ids {
        let profile = known
            .iter()
            .find(|profile| &profile.id == id)
            .ok_or_else(|| Error::other(format!("unknown Modrinth instance {id}")))?
            .clone();
        let source = profiles_root.join(&profile.path);
        if relative_within(&profiles_root, &source).is_none() {
            return Err(Error::other(format!("not a profile folder: {id}")));
        }
        let entries = walk_files(files, &source, &|_| false)?;
        total_bytes += entries.iter().map(|(_, size)| size).sum::<u64>();
        planned.push((profile, source, entries));
    }

    task.stage("copying");
    let mut outcome = MigrationOutcome {
        imported: Vec::new(),
        failed: Vec::new(),
    };
    let mut done = 0u64;

    for (profile, source, entries) in planned {
        if task.token().is_cancelled() {
            return Err(Error::Cancelled);
        }
        let instance_id = uuid::Uuid::new_v4().to_string();
        let destination = files.paths().instance_dir(&instance_id);

        let result = (|| -> Result<()> {
            files.ensure_dir(&destination)?;
            for (path, size) in &entries {
                if task.token().is_cancelled() {
                    return Err(Error::Cancelled);
                }
                let Some(relative) = relative_within(&source, path) else {
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

            if profile.game_version.is_empty() {
                return Err(Error::other("instance has no Minecraft version"));
            }

            let instance = Instance {
                id: instance_id.clone(),
                name: profile.name.clone(),
                version_id: profile.game_version.clone(),
                created_at: chrono::Utc::now(),
                min_memory_mb: None,
                max_memory_mb: None,
                java_path: None,
                last_played_at: profile.last_played,
                playtime_secs: profile.playtime_secs,
                dir: destination.display().to_string(),
                logo: None,
                loader: profile.loader.clone(),
                loader_version: profile.loader.as_ref().and(profile.loader_version.clone()),
                launch_version_id: None,
                pack_provider: profile
                    .pack_project
                    .as_ref()
                    .map(|_| "modrinth".to_string()),
                pack_project_id: profile.pack_project.clone(),
                pack_version_id: profile.pack_version.clone(),
                import_source: Some("modrinth".to_string()),
                import_source_id: Some(profile.id.clone()),
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
            record_files(
                db,
                root,
                &profile.id,
                &instance_id,
                profile.pack_version.as_deref(),
            )?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                if let Some(icon) = profile.icon_path.as_deref() {
                    let path = Path::new(icon);
                    let extension = path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or("png");
                    if files.is_external_file(path) {
                        if let Ok(bytes) = files.read_external(path) {
                            let _ = crate::meta::media::write_instance_logo_sync(
                                files,
                                &instance_id,
                                extension,
                                &bytes,
                            );
                        }
                    }
                }
                tracing::info!(instance = %instance_id, source = %profile.id, "instance migrated");
                outcome.imported.push(instance_id);
            }
            Err(error) => {
                let _ = files.remove_instance_dir(&instance_id);
                let _ = db.delete_instance(&instance_id);
                if matches!(error, Error::Cancelled) {
                    return Err(error);
                }
                tracing::warn!(source = %profile.id, error = %error, "instance migration failed");
                outcome.failed.push((profile.id, error.to_string()));
            }
        }
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{profiles_dir, read_profiles};

    #[test]
    fn a_corrupt_profile_row_is_reported() {
        let root = std::env::temp_dir().join(format!("basalt-modrinth-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let connection = Connection::open(root.join("app.db")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE instances (
                    id TEXT,
                    path TEXT,
                    name TEXT,
                    icon_path TEXT,
                    last_played INTEGER,
                    submitted_time_played INTEGER,
                    recent_time_played INTEGER
                );
                CREATE TABLE instance_content_sets (
                    instance_id TEXT,
                    game_version TEXT,
                    loader TEXT,
                    loader_version TEXT
                );
                CREATE TABLE instance_links (
                    instance_id TEXT,
                    link_kind TEXT,
                    modrinth_project_id TEXT,
                    modrinth_version_id TEXT
                );
                INSERT INTO instances VALUES (
                    'broken', 'profile', x'FF', NULL, NULL, 0, 0
                );",
            )
            .unwrap();
        drop(connection);

        let error = read_profiles(&root).unwrap_err();
        assert!(error.to_string().contains("Modrinth instance row"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn profile_folder_follows_the_custom_directory() {
        let root = std::env::temp_dir().join(format!("basalt-modrinth-{}", uuid::Uuid::new_v4()));
        let moved = root.join("games").join("modrinth");
        std::fs::create_dir_all(&root).unwrap();

        assert_eq!(profiles_dir(&root), root.join("profiles"));

        let connection = Connection::open(root.join("app.db")).unwrap();
        connection
            .execute_batch("CREATE TABLE settings (id INTEGER, custom_dir TEXT);")
            .unwrap();
        connection
            .execute("INSERT INTO settings VALUES (0, NULL)", [])
            .unwrap();
        drop(connection);
        assert_eq!(profiles_dir(&root), root.join("profiles"));

        let connection = Connection::open(root.join("app.db")).unwrap();
        connection
            .execute(
                "UPDATE settings SET custom_dir = ?1",
                [moved.display().to_string()],
            )
            .unwrap();
        drop(connection);
        assert_eq!(profiles_dir(&root), moved.join("profiles"));

        std::fs::remove_dir_all(root).unwrap();
    }
}
