use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension};

use crate::auth::account::{Account, AccountStore};
use crate::config::{Instance, LauncherSettings};
use crate::error::Result;
use crate::paths::Paths;

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContentSource {
    pub provider: String,
    pub project_id: String,
    pub version_id: Option<String>,
    pub title: Option<String>,
    pub icon_url: Option<String>,
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < 1 {
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
            PRAGMA user_version = 1;",
        )?;
    }
    if version < 2 {
        conn.execute_batch(
            "ALTER TABLE instances ADD COLUMN loader TEXT;
            ALTER TABLE instances ADD COLUMN loader_version TEXT;
            ALTER TABLE instances ADD COLUMN launch_version_id TEXT;
            PRAGMA user_version = 2;",
        )?;
    }
    if version < 3 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS content_sources(
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
            PRAGMA user_version = 3;",
        )?;
    }
    Ok(())
}

impl Db {
    pub fn open(paths: &Paths) -> Result<Self> {
        let conn = Connection::open(paths.root.join("basalt.db"))?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        migrate(&conn)?;
        let db = Db(Arc::new(Mutex::new(conn)));
        db.import_legacy_json(paths)?;
        Ok(db)
    }

    fn import_legacy_json(&self, paths: &Paths) -> Result<()> {
        let settings_file = paths.settings_file();
        if let Ok(bytes) = std::fs::read(&settings_file) {
            if let Ok(settings) = serde_json::from_slice::<LauncherSettings>(&bytes) {
                self.save_settings(&settings)?;
            }
            let _ = std::fs::rename(&settings_file, settings_file.with_extension("json.migrated"));
        }

        let instances_file = paths.instances_file();
        if let Ok(bytes) = std::fs::read(&instances_file) {
            if let Ok(instances) = serde_json::from_slice::<Vec<Instance>>(&bytes) {
                for instance in &instances {
                    self.insert_instance(instance)?;
                }
            }
            let _ = std::fs::rename(&instances_file, instances_file.with_extension("json.migrated"));
        }

        let accounts_file = paths.accounts_file();
        if let Ok(bytes) = std::fs::read(&accounts_file) {
            if let Ok(store) = serde_json::from_slice::<AccountStore>(&bytes) {
                self.save_accounts(&store)?;
            }
            let _ = std::fs::rename(&accounts_file, accounts_file.with_extension("json.migrated"));
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
        Ok(match value {
            Some(json) => serde_json::from_str(&json)?,
            None => LauncherSettings::default(),
        })
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

    pub fn list_instances(&self, paths: &Paths) -> Result<Vec<Instance>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, version_id, created_at, min_memory_mb, max_memory_mb,
                    java_path, last_played_at, playtime_secs, loader, loader_version,
                    launch_version_id
             FROM instances ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let created_at: String = row.get(3)?;
            Ok(Instance {
                dir: paths.instance_dir(&id).display().to_string(),
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
                 launch_version_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE instances
             SET name = ?2, min_memory_mb = ?3, max_memory_mb = ?4, java_path = ?5
             WHERE id = ?1",
            params![instance_id, name, min_memory_mb, max_memory_mb, java_path],
        )?;
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

    pub fn record_content_source(
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
            "INSERT OR REPLACE INTO content_sources
                (instance_id, kind, file_name, provider, project_id, version_id, title, icon_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![instance_id, kind, file_name, provider, project_id, version_id, title, icon_url],
        )?;
        Ok(())
    }

    pub fn content_sources(
        &self,
        instance_id: &str,
        kind: &str,
    ) -> Result<std::collections::HashMap<String, ContentSource>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT file_name, provider, project_id, version_id, title, icon_url
             FROM content_sources WHERE instance_id = ?1 AND kind = ?2",
        )?;
        let rows = stmt.query_map(params![instance_id, kind], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ContentSource {
                    provider: row.get(1)?,
                    project_id: row.get(2)?,
                    version_id: row.get(3)?,
                    title: row.get(4)?,
                    icon_url: row.get(5)?,
                },
            ))
        })?;
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let (file_name, source) = row?;
            map.insert(file_name, source);
        }
        Ok(map)
    }

    pub fn delete_content_source(
        &self,
        instance_id: &str,
        kind: &str,
        file_name: &str,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM content_sources
             WHERE instance_id = ?1 AND kind = ?2 AND file_name = ?3",
            params![instance_id, kind, file_name],
        )?;
        Ok(())
    }

    pub fn delete_instance_content_sources(&self, instance_id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM content_sources WHERE instance_id = ?1",
            params![instance_id],
        )?;
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
