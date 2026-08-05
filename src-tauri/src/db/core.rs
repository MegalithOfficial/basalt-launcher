use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::{
    config::{Instance, LauncherSettings},
    error::Result,
    files::FileManager,
};

use super::{migrate, Db, SCHEMA_VERSION};

const WAL_SIZE_LIMIT: i64 = 8 * 1024 * 1024;
const VACUUM_THRESHOLD: i64 = 2 * 1024 * 1024;

fn compact(conn: &Connection) {
    let _ = conn.pragma_update(None, "journal_size_limit", WAL_SIZE_LIMIT);
    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");

    let free_bytes = (|| -> rusqlite::Result<i64> {
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |row| row.get(0))?;
        let free: i64 = conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
        Ok(page_size * free)
    })()
    .unwrap_or(0);

    if free_bytes >= VACUUM_THRESHOLD {
        match conn.execute_batch("VACUUM;") {
            Ok(()) => tracing::info!(free_bytes, "compacted the database"),
            Err(error) => tracing::warn!(error = %error, "could not compact the database"),
        }
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }
}

impl Db {
    pub fn open(files: &FileManager) -> Result<Self> {
        let paths = files.paths();
        let path = paths.root.join("basalt.db");
        tracing::info!(path = %path.display(), schema_version = SCHEMA_VERSION, "opening database");
        let conn = Connection::open(&path)?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        migrate(&conn)?;
        compact(&conn);
        let db = Db(Arc::new(Mutex::new(conn)));
        db.import_legacy_json(files)?;
        let discarded = db.discard_insecure_credentials()?;
        if discarded > 0 {
            tracing::info!(discarded, "discarded insecure plaintext credentials");
        }
        for path in [
            paths.settings_file(),
            paths.settings_file().with_extension("json.migrated"),
            paths.accounts_file(),
            paths.accounts_file().with_extension("json.migrated"),
        ] {
            files.remove_file_if_exists(path)?;
        }
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
            if let Ok(mut settings) = serde_json::from_slice::<LauncherSettings>(&bytes) {
                settings.curseforge_api_key = None;
                settings.proxy_password.clear();
                self.save_settings(&settings)?;
            }
        }

        let instances_file = paths.instances_file();
        if let Ok(bytes) = files.read(&instances_file) {
            if let Ok(instances) = serde_json::from_slice::<Vec<Instance>>(&bytes) {
                tracing::info!(
                    count = instances.len(),
                    "importing instances from legacy json"
                );
                for instance in &instances {
                    self.insert_instance(instance)?;
                }
            }
            let _ = files.rename(
                &instances_file,
                instances_file.with_extension("json.migrated"),
            );
        }

        Ok(())
    }
}
