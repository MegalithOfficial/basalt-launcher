use rusqlite::{params, OptionalExtension};

use crate::{config::LauncherSettings, error::Result};

use super::Db;

impl Db {
    pub fn load_settings(&self) -> Result<LauncherSettings> {
        let conn = self.0.lock().unwrap();
        let value: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'launcher'",
                [],
                |row| row.get(0),
            )
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
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(value)
    }
}
