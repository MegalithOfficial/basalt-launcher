use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::{
    auth::account::AccountStore,
    config::{Instance, LauncherSettings},
    error::Result,
    files::FileManager,
};

use super::{migrate, Db, SCHEMA_VERSION};

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
            let _ = files.rename(
                &settings_file,
                settings_file.with_extension("json.migrated"),
            );
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

        let accounts_file = paths.accounts_file();
        if let Ok(bytes) = files.read(&accounts_file) {
            if let Ok(store) = serde_json::from_slice::<AccountStore>(&bytes) {
                self.save_accounts(&store)?;
            }
            let _ = files.rename(
                &accounts_file,
                accounts_file.with_extension("json.migrated"),
            );
        }

        Ok(())
    }
}
