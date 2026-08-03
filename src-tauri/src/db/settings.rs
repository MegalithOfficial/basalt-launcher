use rusqlite::{params, OptionalExtension};

use crate::{
    config::LauncherSettings,
    credentials::{CredentialStore, CURSEFORGE_API_KEY, MASKED_SECRET, PROXY_PASSWORD},
    error::Result,
};

use super::{cache::CURSEFORGE_CACHE_PREFIX, Db};

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

    pub fn load_runtime_settings(&self, credentials: &CredentialStore) -> Result<LauncherSettings> {
        let mut settings = self.load_settings()?;
        settings.curseforge_api_key = read_optional_secret(credentials, CURSEFORGE_API_KEY);
        settings.proxy_password =
            read_optional_secret(credentials, PROXY_PASSWORD).unwrap_or_default();
        Ok(settings)
    }

    pub fn load_settings_view(&self, credentials: &CredentialStore) -> Result<LauncherSettings> {
        let mut settings = self.load_runtime_settings(credentials)?;
        if settings.curseforge_api_key.is_some() {
            settings.curseforge_api_key = Some(MASKED_SECRET.to_string());
        }
        if !settings.proxy_password.is_empty() {
            settings.proxy_password = MASKED_SECRET.to_string();
        }
        Ok(settings)
    }

    pub fn save_settings_secure(
        &self,
        credentials: &CredentialStore,
        submitted: &LauncherSettings,
    ) -> Result<LauncherSettings> {
        let previous_api_key = read_optional_secret(credentials, CURSEFORGE_API_KEY);
        let curseforge_api_key = update_secret(
            credentials,
            CURSEFORGE_API_KEY,
            submitted.curseforge_api_key.as_deref(),
        )?;
        if curseforge_api_key != previous_api_key {
            self.purge_api_cache_prefix(CURSEFORGE_CACHE_PREFIX)?;
        }
        let proxy_password =
            update_secret(credentials, PROXY_PASSWORD, Some(&submitted.proxy_password))?;

        let mut persisted = submitted.clone();
        persisted.curseforge_api_key = None;
        persisted.proxy_password.clear();
        self.save_settings(&persisted)?;

        let mut runtime = persisted;
        runtime.curseforge_api_key = curseforge_api_key;
        runtime.proxy_password = proxy_password.unwrap_or_default();
        Ok(runtime)
    }

    pub fn discard_insecure_credentials(&self) -> Result<usize> {
        let mut settings = self.load_settings()?;
        let had_settings_secrets = settings
            .curseforge_api_key
            .as_deref()
            .is_some_and(|secret| !secret.is_empty())
            || !settings.proxy_password.is_empty();
        if had_settings_secrets {
            settings.curseforge_api_key = None;
            settings.proxy_password.clear();
            self.save_settings(&settings)?;
        }

        let discarded = usize::from(had_settings_secrets);
        if discarded > 0 {
            let conn = self.0.lock().unwrap();
            conn.execute_batch(
                "PRAGMA secure_delete = ON;
                 PRAGMA wal_checkpoint(TRUNCATE);
                 VACUUM;
                 PRAGMA wal_checkpoint(TRUNCATE);",
            )?;
        }
        Ok(discarded)
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

fn read_optional_secret(credentials: &CredentialStore, key: &str) -> Option<String> {
    match credentials.get(key) {
        Ok(secret) => secret,
        Err(error) => {
            tracing::warn!(credential = key, %error, "credential is unavailable");
            None
        }
    }
}

fn update_secret(
    credentials: &CredentialStore,
    key: &str,
    submitted: Option<&str>,
) -> Result<Option<String>> {
    let submitted = submitted.filter(|secret| !secret.is_empty());
    if submitted == Some(MASKED_SECRET) {
        return credentials.get(key);
    }

    match submitted {
        Some(secret) => {
            credentials.set(key, secret)?;
            Ok(Some(secret.to_string()))
        }
        None => {
            match credentials.get(key) {
                Ok(Some(_)) => credentials.delete(key)?,
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(credential = key, %error, "could not check for a credential to delete");
                }
            }
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Db;
    use crate::{
        config::LauncherSettings,
        credentials::{tests::memory_store, CURSEFORGE_API_KEY},
    };

    #[test]
    fn settings_secrets_are_masked_and_not_written_to_sqlite() {
        let db = Db::open_in_memory().unwrap();
        let credentials = memory_store();
        let settings = LauncherSettings {
            curseforge_api_key: Some("curse-secret".to_string()),
            proxy_password: "proxy-secret".to_string(),
            ..LauncherSettings::default()
        };

        db.save_settings_secure(&credentials, &settings).unwrap();

        let stored = db.load_settings().unwrap();
        assert_eq!(stored.curseforge_api_key, None);
        assert!(stored.proxy_password.is_empty());
        let runtime = db.load_runtime_settings(&credentials).unwrap();
        assert_eq!(runtime.curseforge_api_key.as_deref(), Some("curse-secret"));
        assert_eq!(runtime.proxy_password, "proxy-secret");
        let view = db.load_settings_view(&credentials).unwrap();
        assert_ne!(view.curseforge_api_key.as_deref(), Some("curse-secret"));
        assert_ne!(view.proxy_password, "proxy-secret");
    }

    #[test]
    fn masked_secrets_survive_unrelated_settings_updates() {
        let db = Db::open_in_memory().unwrap();
        let credentials = memory_store();
        let initial = LauncherSettings {
            curseforge_api_key: Some("curse-secret".to_string()),
            proxy_password: "proxy-secret".to_string(),
            ..LauncherSettings::default()
        };
        db.save_settings_secure(&credentials, &initial).unwrap();

        let mut submitted = db.load_settings_view(&credentials).unwrap();
        submitted.max_memory_mb = 4096;
        let runtime = db.save_settings_secure(&credentials, &submitted).unwrap();

        assert_eq!(runtime.max_memory_mb, 4096);
        assert_eq!(runtime.curseforge_api_key.as_deref(), Some("curse-secret"));
        assert_eq!(runtime.proxy_password, "proxy-secret");
    }

    #[test]
    fn replacing_the_curseforge_key_drops_only_its_cached_responses() {
        let db = Db::open_in_memory().unwrap();
        let credentials = memory_store();
        let settings = LauncherSettings {
            curseforge_api_key: Some("first-key".to_string()),
            ..LauncherSettings::default()
        };
        db.save_settings_secure(&credentials, &settings).unwrap();
        db.cache_put("cf:project:42", "{}", None, 0, 3600).unwrap();
        db.cache_put("mr:project:42", "{}", None, 0, 3600).unwrap();

        let rotated = LauncherSettings {
            curseforge_api_key: Some("second-key".to_string()),
            ..LauncherSettings::default()
        };
        db.save_settings_secure(&credentials, &rotated).unwrap();

        assert!(db.cache_get("cf:project:42", 0).unwrap().is_none());
        assert!(db.cache_get("mr:project:42", 0).unwrap().is_some());
    }

    #[test]
    fn removing_the_curseforge_key_drops_its_cached_responses() {
        let db = Db::open_in_memory().unwrap();
        let credentials = memory_store();
        db.save_settings_secure(
            &credentials,
            &LauncherSettings {
                curseforge_api_key: Some("first-key".to_string()),
                ..LauncherSettings::default()
            },
        )
        .unwrap();
        db.cache_put("cf:search:mod:query", "{}", None, 0, 3600)
            .unwrap();

        db.save_settings_secure(&credentials, &LauncherSettings::default())
            .unwrap();

        assert!(db.cache_get("cf:search:mod:query", 0).unwrap().is_none());
    }

    #[test]
    fn unrelated_settings_updates_keep_cached_responses() {
        let db = Db::open_in_memory().unwrap();
        let credentials = memory_store();
        db.save_settings_secure(
            &credentials,
            &LauncherSettings {
                curseforge_api_key: Some("curse-secret".to_string()),
                ..LauncherSettings::default()
            },
        )
        .unwrap();
        db.cache_put("cf:project:42", "{}", None, 0, 3600).unwrap();

        let mut submitted = db.load_settings_view(&credentials).unwrap();
        submitted.max_memory_mb = 4096;
        db.save_settings_secure(&credentials, &submitted).unwrap();

        assert!(db.cache_get("cf:project:42", 0).unwrap().is_some());
    }

    #[test]
    fn discards_plaintext_credentials_instead_of_importing_them() {
        let db = Db::open_in_memory().unwrap();
        let credentials = memory_store();
        db.save_settings(&LauncherSettings {
            curseforge_api_key: Some("old-key".to_string()),
            proxy_password: "old-password".to_string(),
            ..LauncherSettings::default()
        })
        .unwrap();

        assert_eq!(db.discard_insecure_credentials().unwrap(), 1);
        let stored = db.load_settings().unwrap();
        assert_eq!(stored.curseforge_api_key, None);
        assert!(stored.proxy_password.is_empty());
        assert_eq!(credentials.get(CURSEFORGE_API_KEY).unwrap(), None);
    }
}
