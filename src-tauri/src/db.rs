use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension};

use crate::auth::account::{Account, AccountStore};
use crate::config::{Instance, LauncherSettings};
use crate::error::Result;
use crate::files::FileManager;

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

#[cfg(test)]
mod tests {
    use super::{column_exists, migrate};
    use rusqlite::Connection;

    #[test]
    fn migrate_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        assert!(column_exists(&conn, "instances", "pack_provider").unwrap());
        assert!(column_exists(&conn, "instances", "loader").unwrap());
        assert!(column_exists(&conn, "content_files", "origin").unwrap());
        assert!(super::table_exists(&conn, "api_cache").unwrap());
    }

    #[test]
    fn migrate_moves_content_sources_forward() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE content_sources(
                instance_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                file_name TEXT NOT NULL,
                provider TEXT NOT NULL,
                project_id TEXT NOT NULL,
                version_id TEXT,
                title TEXT,
                icon_url TEXT,
                PRIMARY KEY (instance_id, kind, file_name)
            );
            INSERT INTO content_sources VALUES
                ('i1', 'mods', 'sodium.jar', 'modrinth', 'AANobbMI', 'abc', 'Sodium', NULL);",
        )
        .unwrap();
        migrate(&conn).unwrap();

        assert!(!super::table_exists(&conn, "content_sources").unwrap());
        let (project, origin): (String, String) = conn
            .query_row(
                "SELECT project_id, origin FROM content_files WHERE file_name = 'sodium.jar'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(project, "AANobbMI");
        assert_eq!(origin, "user");
    }

    #[test]
    fn migrate_heals_partial_state() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE instances(
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                version_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                min_memory_mb INTEGER,
                max_memory_mb INTEGER,
                java_path TEXT,
                last_played_at INTEGER,
                playtime_secs INTEGER NOT NULL DEFAULT 0
            );
            ALTER TABLE instances ADD COLUMN loader TEXT;
            ALTER TABLE instances ADD COLUMN loader_version TEXT;
            ALTER TABLE instances ADD COLUMN launch_version_id TEXT;
            ALTER TABLE instances ADD COLUMN pack_provider TEXT;
            ALTER TABLE instances ADD COLUMN pack_project_id TEXT;
            ALTER TABLE instances ADD COLUMN pack_version_id TEXT;
            PRAGMA user_version = 3;",
        )
        .unwrap();
        migrate(&conn).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, super::SCHEMA_VERSION);
        assert!(column_exists(&conn, "instances", "pack_version_id").unwrap());
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkinRecord {
    pub id: String,
    pub name: String,
    pub variant: String,
    pub file_name: String,
    pub source: Option<String>,
    pub hash: Option<String>,
    pub remote_hash: Option<String>,
    pub added_at: i64,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ContentFile {
    pub file_name: String,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub murmur2: Option<i64>,
    pub provider: Option<String>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub title: Option<String>,
    pub icon_url: Option<String>,
    pub mod_id: Option<String>,
    pub mod_version: Option<String>,
    pub dependencies: Option<String>,
    pub origin: String,
    pub pack_version_id: Option<String>,
    pub installed_at: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PendingOperation {
    pub id: String,
    pub kind: String,
    pub instance_id: Option<String>,
    pub title: String,
    pub payload: Option<String>,
    pub started_at: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContentUpdate {
    pub kind: String,
    pub file_name: String,
    pub latest_version_id: String,
    pub latest_name: String,
    pub latest_file_name: String,
}

pub struct CachedResponse {
    pub body: String,
    pub etag: Option<String>,
    pub fresh: bool,
}

const SCHEMA_VERSION: i64 = 7;

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in names {
        if name? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    declaration: &str,
) -> Result<()> {
    if !column_exists(conn, table, column)? {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {declaration}"),
            [],
        )?;
    }
    Ok(())
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS settings(
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS instances(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            version_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            min_memory_mb INTEGER,
            max_memory_mb INTEGER,
            java_path TEXT,
            last_played_at INTEGER,
            playtime_secs INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS accounts(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            mc_access_token TEXT NOT NULL,
            refresh_token TEXT NOT NULL,
            expires_at INTEGER NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS content_files(
            instance_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            file_name TEXT NOT NULL,
            sha1 TEXT,
            sha512 TEXT,
            murmur2 INTEGER,
            provider TEXT,
            project_id TEXT,
            version_id TEXT,
            title TEXT,
            icon_url TEXT,
            mod_id TEXT,
            mod_version TEXT,
            dependencies TEXT,
            origin TEXT NOT NULL DEFAULT 'user',
            pack_version_id TEXT,
            installed_at INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (instance_id, kind, file_name)
        );
        CREATE INDEX IF NOT EXISTS content_files_project
            ON content_files(instance_id, kind, project_id);
        CREATE INDEX IF NOT EXISTS content_files_sha1 ON content_files(sha1);
        CREATE TABLE IF NOT EXISTS content_updates(
            instance_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            file_name TEXT NOT NULL,
            latest_version_id TEXT NOT NULL,
            latest_name TEXT NOT NULL,
            latest_file_name TEXT NOT NULL,
            checked_at INTEGER NOT NULL,
            PRIMARY KEY (instance_id, kind, file_name)
        );
        CREATE TABLE IF NOT EXISTS pending_operations(
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            instance_id TEXT,
            title TEXT NOT NULL,
            payload TEXT,
            started_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS api_cache(
            key TEXT PRIMARY KEY,
            body TEXT NOT NULL,
            etag TEXT,
            fetched_at INTEGER NOT NULL,
            ttl_secs INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS skins(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            variant TEXT NOT NULL,
            file_name TEXT NOT NULL,
            source TEXT,
            hash TEXT,
            remote_hash TEXT,
            added_at INTEGER NOT NULL
        );",
    )?;

    for (column, declaration) in [
        ("loader", "TEXT"),
        ("loader_version", "TEXT"),
        ("launch_version_id", "TEXT"),
        ("pack_provider", "TEXT"),
        ("pack_project_id", "TEXT"),
        ("pack_version_id", "TEXT"),
    ] {
        add_column_if_missing(conn, "instances", column, declaration)?;
    }
    add_column_if_missing(conn, "skins", "hash", "TEXT")?;
    add_column_if_missing(conn, "skins", "remote_hash", "TEXT")?;

    if table_exists(conn, "content_sources")? {
        conn.execute_batch(
            "INSERT OR IGNORE INTO content_files
                (instance_id, kind, file_name, provider, project_id, version_id,
                 title, icon_url, origin, installed_at)
             SELECT instance_id, kind, file_name, provider, project_id, version_id,
                    title, icon_url, 'user', 0
             FROM content_sources;
             DROP TABLE content_sources;",
        )?;
    }

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

impl Db {
    pub fn open(files: &FileManager) -> Result<Self> {
        let paths = files.paths();
        let path = paths.root.join("basalt.db");
        tracing::info!(path = %path.display(), schema_version = SCHEMA_VERSION, "opening database");
        let conn = Connection::open(&path)?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        migrate(&conn)?;
        let db = Db(Arc::new(Mutex::new(conn)));
        db.import_legacy_json(files)?;
        Ok(db)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        migrate(&conn)?;
        Ok(Db(Arc::new(Mutex::new(conn))))
    }

    fn import_legacy_json(&self, files: &FileManager) -> Result<()> {
        let paths = files.paths();
        let settings_file = paths.settings_file();
        if let Ok(bytes) = files.read(&settings_file) {
            if let Ok(settings) = serde_json::from_slice::<LauncherSettings>(&bytes) {
                self.save_settings(&settings)?;
            }
            let _ = files.rename(&settings_file, settings_file.with_extension("json.migrated"));
        }

        let instances_file = paths.instances_file();
        if let Ok(bytes) = files.read(&instances_file) {
            if let Ok(instances) = serde_json::from_slice::<Vec<Instance>>(&bytes) {
                tracing::info!(count = instances.len(), "importing instances from legacy json");
                for instance in &instances {
                    self.insert_instance(instance)?;
                }
            }
            let _ = files.rename(&instances_file, instances_file.with_extension("json.migrated"));
        }

        let accounts_file = paths.accounts_file();
        if let Ok(bytes) = files.read(&accounts_file) {
            if let Ok(store) = serde_json::from_slice::<AccountStore>(&bytes) {
                self.save_accounts(&store)?;
            }
            let _ = files.rename(&accounts_file, accounts_file.with_extension("json.migrated"));
        }

        Ok(())
    }

    pub fn load_settings(&self) -> Result<LauncherSettings> {
        let conn = self.0.lock().unwrap();
        let value: Option<String> = conn
            .query_row("SELECT value FROM settings WHERE key = 'launcher'", [], |row| {
                row.get(0)
            })
            .optional()?;
        let mut settings: LauncherSettings = match value {
            Some(json) => serde_json::from_str(&json)?,
            None => LauncherSettings::default(),
        };
        if settings.jvm_args.trim().is_empty() {
            settings.jvm_args = crate::config::DEFAULT_JVM_ARGS.to_string();
        }
        Ok(settings)
    }

    pub fn save_settings(&self, settings: &LauncherSettings) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO settings(key, value) VALUES('launcher', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1",
            params![serde_json::to_string(settings)?],
        )?;
        Ok(())
    }

    pub fn put_kv(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = ?2",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_kv(&self, key: &str) -> Result<Option<String>> {
        let conn = self.0.lock().unwrap();
        let value = conn
            .query_row("SELECT value FROM settings WHERE key = ?1", params![key], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;
        Ok(value)
    }

    pub fn list_instances(&self, files: &FileManager) -> Result<Vec<Instance>> {
        let paths = files.paths();
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, version_id, created_at, min_memory_mb, max_memory_mb,
                    java_path, last_played_at, playtime_secs, loader, loader_version,
                    launch_version_id, pack_provider, pack_project_id, pack_version_id
             FROM instances ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let created_at: String = row.get(3)?;
            Ok(Instance {
                dir: paths.instance_dir(&id).display().to_string(),
                logo: crate::meta::media::instance_logo(files, &id),
                id,
                name: row.get(1)?,
                version_id: row.get(2)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                min_memory_mb: row.get(4)?,
                max_memory_mb: row.get(5)?,
                java_path: row.get(6)?,
                last_played_at: row.get(7)?,
                playtime_secs: row.get(8)?,
                loader: row.get(9)?,
                loader_version: row.get(10)?,
                launch_version_id: row.get(11)?,
                pack_provider: row.get(12)?,
                pack_project_id: row.get(13)?,
                pack_version_id: row.get(14)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn insert_instance(&self, instance: &Instance) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO instances
                (id, name, version_id, created_at, min_memory_mb, max_memory_mb,
                 java_path, last_played_at, playtime_secs, loader, loader_version,
                 launch_version_id, pack_provider, pack_project_id, pack_version_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                instance.id,
                instance.name,
                instance.version_id,
                instance.created_at.to_rfc3339(),
                instance.min_memory_mb,
                instance.max_memory_mb,
                instance.java_path,
                instance.last_played_at,
                instance.playtime_secs,
                instance.loader,
                instance.loader_version,
                instance.launch_version_id,
                instance.pack_provider,
                instance.pack_project_id,
                instance.pack_version_id,
            ],
        )?;
        Ok(())
    }

    pub fn update_instance_settings(
        &self,
        instance_id: &str,
        name: &str,
        min_memory_mb: Option<u32>,
        max_memory_mb: Option<u32>,
        java_path: Option<String>,
        loader: Option<String>,
        loader_version: Option<String>,
        version_id: &str,
        reset_launch_version: bool,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        if reset_launch_version {
            conn.execute(
                "UPDATE instances
                 SET name = ?2, min_memory_mb = ?3, max_memory_mb = ?4, java_path = ?5,
                     loader = ?6, loader_version = ?7, version_id = ?8,
                     launch_version_id = NULL
                 WHERE id = ?1",
                params![instance_id, name, min_memory_mb, max_memory_mb, java_path, loader, loader_version, version_id],
            )?;
        } else {
            conn.execute(
                "UPDATE instances
                 SET name = ?2, min_memory_mb = ?3, max_memory_mb = ?4, java_path = ?5,
                     loader = ?6, loader_version = ?7, version_id = ?8
                 WHERE id = ?1",
                params![instance_id, name, min_memory_mb, max_memory_mb, java_path, loader, loader_version, version_id],
            )?;
        }
        Ok(())
    }

    pub fn delete_instance(&self, instance_id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM instances WHERE id = ?1", params![instance_id])?;
        Ok(())
    }

    pub fn set_launch_version(&self, instance_id: &str, launch_version_id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE instances SET launch_version_id = ?2 WHERE id = ?1",
            params![instance_id, launch_version_id],
        )?;
        Ok(())
    }

    pub fn record_playtime(&self, instance_id: &str, played_secs: i64, ended_at: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE instances
             SET playtime_secs = playtime_secs + ?2, last_played_at = ?3
             WHERE id = ?1",
            params![instance_id, played_secs.max(0), ended_at],
        )?;
        Ok(())
    }

    pub fn record_content_file(
        &self,
        instance_id: &str,
        kind: &str,
        file: &ContentFile,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO content_files
                (instance_id, kind, file_name, sha1, sha512, murmur2, provider, project_id,
                 version_id, title, icon_url, mod_id, mod_version, dependencies, origin,
                 pack_version_id, installed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                instance_id,
                kind,
                file.file_name,
                file.sha1,
                file.sha512,
                file.murmur2,
                file.provider,
                file.project_id,
                file.version_id,
                file.title,
                file.icon_url,
                file.mod_id,
                file.mod_version,
                file.dependencies,
                file.origin,
                file.pack_version_id,
                file.installed_at,
            ],
        )?;
        Ok(())
    }

    pub fn merge_identity(
        &self,
        instance_id: &str,
        kind: &str,
        file_name: &str,
        sha1: Option<&str>,
        sha512: Option<&str>,
        murmur2: Option<i64>,
        mod_id: Option<&str>,
        mod_version: Option<&str>,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO content_files
                (instance_id, kind, file_name, sha1, sha512, murmur2, mod_id, mod_version,
                 origin, installed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'manual', 0)
             ON CONFLICT(instance_id, kind, file_name) DO UPDATE SET
                sha1 = coalesce(excluded.sha1, sha1),
                sha512 = coalesce(excluded.sha512, sha512),
                murmur2 = coalesce(excluded.murmur2, murmur2),
                mod_id = coalesce(excluded.mod_id, mod_id),
                mod_version = coalesce(excluded.mod_version, mod_version)",
            params![instance_id, kind, file_name, sha1, sha512, murmur2, mod_id, mod_version],
        )?;
        Ok(())
    }

    pub fn merge_provider_identity(
        &self,
        instance_id: &str,
        kind: &str,
        file_name: &str,
        provider: &str,
        project_id: &str,
        version_id: Option<&str>,
        title: Option<&str>,
        icon_url: Option<&str>,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE content_files SET
                provider = ?4,
                project_id = ?5,
                version_id = coalesce(?6, version_id),
                title = coalesce(?7, title),
                icon_url = coalesce(?8, icon_url)
             WHERE instance_id = ?1 AND kind = ?2 AND file_name = ?3",
            params![
                instance_id, kind, file_name, provider, project_id, version_id, title, icon_url
            ],
        )?;
        Ok(())
    }

    pub fn set_fallback_title(
        &self,
        instance_id: &str,
        kind: &str,
        file_name: &str,
        title: &str,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE content_files SET title = ?4
             WHERE instance_id = ?1 AND kind = ?2 AND file_name = ?3 AND title IS NULL",
            params![instance_id, kind, file_name, title],
        )?;
        Ok(())
    }

    fn read_content_file(row: &rusqlite::Row) -> rusqlite::Result<ContentFile> {
        Ok(ContentFile {
            file_name: row.get(0)?,
            sha1: row.get(1)?,
            sha512: row.get(2)?,
            murmur2: row.get(3)?,
            provider: row.get(4)?,
            project_id: row.get(5)?,
            version_id: row.get(6)?,
            title: row.get(7)?,
            icon_url: row.get(8)?,
            mod_id: row.get(9)?,
            mod_version: row.get(10)?,
            dependencies: row.get(11)?,
            origin: row.get(12)?,
            pack_version_id: row.get(13)?,
            installed_at: row.get(14)?,
        })
    }

    pub fn content_files(&self, instance_id: &str, kind: &str) -> Result<Vec<ContentFile>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT file_name, sha1, sha512, murmur2, provider, project_id, version_id,
                    title, icon_url, mod_id, mod_version, dependencies, origin,
                    pack_version_id, installed_at
             FROM content_files WHERE instance_id = ?1 AND kind = ?2",
        )?;
        let rows = stmt.query_map(params![instance_id, kind], Self::read_content_file)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn content_file(
        &self,
        instance_id: &str,
        kind: &str,
        file_name: &str,
    ) -> Result<Option<ContentFile>> {
        let conn = self.0.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT file_name, sha1, sha512, murmur2, provider, project_id, version_id,
                        title, icon_url, mod_id, mod_version, dependencies, origin,
                        pack_version_id, installed_at
                 FROM content_files WHERE instance_id = ?1 AND kind = ?2 AND file_name = ?3",
                params![instance_id, kind, file_name],
                Self::read_content_file,
            )
            .optional()?;
        Ok(result)
    }

    pub fn installed_project_file(
        &self,
        instance_id: &str,
        kind: &str,
        project_id: &str,
    ) -> Result<Option<(Option<String>, String)>> {
        let conn = self.0.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT version_id, file_name FROM content_files
                 WHERE instance_id = ?1 AND kind = ?2 AND project_id = ?3
                 ORDER BY installed_at DESC LIMIT 1",
                params![instance_id, kind, project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(result)
    }

    pub fn delete_content_file(
        &self,
        instance_id: &str,
        kind: &str,
        file_name: &str,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM content_files
             WHERE instance_id = ?1 AND kind = ?2 AND file_name = ?3",
            params![instance_id, kind, file_name],
        )?;
        conn.execute(
            "DELETE FROM content_updates
             WHERE instance_id = ?1 AND kind = ?2 AND file_name = ?3",
            params![instance_id, kind, file_name],
        )?;
        Ok(())
    }

    pub fn delete_instance_content_files(&self, instance_id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        for table in ["content_files", "content_updates"] {
            conn.execute(
                &format!("DELETE FROM {table} WHERE instance_id = ?1"),
                params![instance_id],
            )?;
        }
        Ok(())
    }

    pub fn replace_content_updates(
        &self,
        instance_id: &str,
        updates: &[ContentUpdate],
        checked_at: i64,
    ) -> Result<()> {
        let mut guard = self.0.lock().unwrap();
        let tx = guard.transaction()?;
        tx.execute(
            "DELETE FROM content_updates WHERE instance_id = ?1",
            params![instance_id],
        )?;
        for update in updates {
            tx.execute(
                "INSERT OR REPLACE INTO content_updates
                    (instance_id, kind, file_name, latest_version_id, latest_name,
                     latest_file_name, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    instance_id,
                    update.kind,
                    update.file_name,
                    update.latest_version_id,
                    update.latest_name,
                    update.latest_file_name,
                    checked_at
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn content_updates(&self, instance_id: &str) -> Result<Vec<ContentUpdate>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT kind, file_name, latest_version_id, latest_name, latest_file_name
             FROM content_updates WHERE instance_id = ?1",
        )?;
        let rows = stmt.query_map(params![instance_id], |row| {
            Ok(ContentUpdate {
                kind: row.get(0)?,
                file_name: row.get(1)?,
                latest_version_id: row.get(2)?,
                latest_name: row.get(3)?,
                latest_file_name: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn updates_checked_at(&self, instance_id: &str) -> Result<Option<i64>> {
        let conn = self.0.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT max(checked_at) FROM content_updates WHERE instance_id = ?1",
                params![instance_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?;
        Ok(result.flatten())
    }

    pub fn begin_operation(&self, op: &PendingOperation) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO pending_operations
                (id, kind, instance_id, title, payload, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![op.id, op.kind, op.instance_id, op.title, op.payload, op.started_at],
        )?;
        Ok(())
    }

    pub fn end_operation(&self, id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM pending_operations WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn pending_operations(&self) -> Result<Vec<PendingOperation>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, kind, instance_id, title, payload, started_at
             FROM pending_operations ORDER BY started_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PendingOperation {
                id: row.get(0)?,
                kind: row.get(1)?,
                instance_id: row.get(2)?,
                title: row.get(3)?,
                payload: row.get(4)?,
                started_at: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn clear_pending_operations(&self) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM pending_operations", [])?;
        Ok(())
    }

    pub fn cache_get(&self, key: &str, now: i64) -> Result<Option<CachedResponse>> {
        let conn = self.0.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT body, etag, fetched_at, ttl_secs FROM api_cache WHERE key = ?1",
                params![key],
                |row| {
                    let fetched_at: i64 = row.get(2)?;
                    let ttl_secs: i64 = row.get(3)?;
                    Ok(CachedResponse {
                        body: row.get(0)?,
                        etag: row.get(1)?,
                        fresh: now - fetched_at < ttl_secs,
                    })
                },
            )
            .optional()?;
        Ok(result)
    }

    pub fn cache_put(
        &self,
        key: &str,
        body: &str,
        etag: Option<&str>,
        fetched_at: i64,
        ttl_secs: i64,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO api_cache(key, body, etag, fetched_at, ttl_secs)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![key, body, etag, fetched_at, ttl_secs],
        )?;
        Ok(())
    }

    pub fn cache_touch(&self, key: &str, fetched_at: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE api_cache SET fetched_at = ?2 WHERE key = ?1",
            params![key, fetched_at],
        )?;
        Ok(())
    }

    pub fn list_skins(&self) -> Result<Vec<SkinRecord>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, variant, file_name, source, hash, remote_hash, added_at
             FROM skins ORDER BY added_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SkinRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                variant: row.get(2)?,
                file_name: row.get(3)?,
                source: row.get(4)?,
                hash: row.get(5)?,
                remote_hash: row.get(6)?,
                added_at: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn insert_skin(&self, skin: &SkinRecord) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO skins
                (id, name, variant, file_name, source, hash, remote_hash, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                skin.id,
                skin.name,
                skin.variant,
                skin.file_name,
                skin.source,
                skin.hash,
                skin.remote_hash,
                skin.added_at
            ],
        )?;
        Ok(())
    }

    pub fn find_skin(&self, id: &str) -> Result<Option<SkinRecord>> {
        let conn = self.0.lock().unwrap();
        let found = conn
            .query_row(
                "SELECT id, name, variant, file_name, source, hash, remote_hash, added_at
                 FROM skins WHERE id = ?1",
                params![id],
                |row| {
                    Ok(SkinRecord {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        variant: row.get(2)?,
                        file_name: row.get(3)?,
                        source: row.get(4)?,
                        hash: row.get(5)?,
                        remote_hash: row.get(6)?,
                        added_at: row.get(7)?,
                    })
                },
            )
            .optional()?;
        Ok(found)
    }

    pub fn update_skin_hash(&self, id: &str, hash: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("UPDATE skins SET hash = ?2 WHERE id = ?1", params![id, hash])?;
        Ok(())
    }

    pub fn enforce_unique_skin_hashes(&self) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS skins_hash_unique ON skins(hash)",
        )?;
        Ok(())
    }

    pub fn rename_skin(&self, id: &str, name: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("UPDATE skins SET name = ?2 WHERE id = ?1", params![id, name])?;
        Ok(())
    }

    pub fn set_skin_variant(&self, id: &str, variant: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE skins SET variant = ?2 WHERE id = ?1",
            params![id, variant],
        )?;
        Ok(())
    }

    pub fn find_skin_by_hash(&self, hash: &str) -> Result<Option<String>> {
        let conn = self.0.lock().unwrap();
        let found = conn
            .query_row(
                "SELECT id FROM skins WHERE hash = ?1 OR remote_hash = ?1",
                params![hash],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(found)
    }

    pub fn set_skin_remote_hash(&self, id: &str, hash: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE skins SET remote_hash = ?2 WHERE id = ?1",
            params![id, hash],
        )?;
        Ok(())
    }

    pub fn delete_skin(&self, id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM skins WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn load_accounts(&self) -> Result<AccountStore> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, mc_access_token, refresh_token, expires_at, is_active
             FROM accounts",
        )?;
        let mut active_id = None;
        let mut accounts = Vec::new();
        let rows = stmt.query_map([], |row| {
            let is_active: bool = row.get(5)?;
            Ok((
                Account {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    mc_access_token: row.get(2)?,
                    refresh_token: row.get(3)?,
                    expires_at: row.get(4)?,
                },
                is_active,
            ))
        })?;
        for row in rows {
            let (account, is_active) = row?;
            if is_active {
                active_id = Some(account.id.clone());
            }
            accounts.push(account);
        }
        Ok(AccountStore { accounts, active_id })
    }

    pub fn save_accounts(&self, store: &AccountStore) -> Result<()> {
        let mut guard = self.0.lock().unwrap();
        let tx = guard.transaction()?;
        tx.execute("DELETE FROM accounts", [])?;
        for account in &store.accounts {
            tx.execute(
                "INSERT INTO accounts(id, name, mc_access_token, refresh_token, expires_at, is_active)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    account.id,
                    account.name,
                    account.mc_access_token,
                    account.refresh_token,
                    account.expires_at,
                    store.active_id.as_deref() == Some(account.id.as_str()),
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
