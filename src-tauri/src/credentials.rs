use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::error::{Error, Result};

const SERVICE: &str = "com.megalithofficial.basalt-launcher";
pub const MASKED_SECRET: &str = "••••••••••••";

pub const CURSEFORGE_API_KEY: &str = "settings/curseforge-api-key";
pub const PROXY_PASSWORD: &str = "settings/proxy-password";

pub fn microsoft_access_token(account_id: &str) -> String {
    format!("account/{account_id}/minecraft-access-token")
}

pub fn microsoft_refresh_token(account_id: &str) -> String {
    format!("account/{account_id}/microsoft-refresh-token")
}

trait SecretBackend: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, secret: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

struct SystemKeyring;

impl SystemKeyring {
    fn entry(key: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, key).map_err(|error| credential_error("open", key, error))
    }
}

impl SecretBackend for SystemKeyring {
    fn get(&self, key: &str) -> Result<Option<String>> {
        match Self::entry(key)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(credential_error("read", key, error)),
        }
    }

    fn set(&self, key: &str, secret: &str) -> Result<()> {
        Self::entry(key)?
            .set_password(secret)
            .map_err(|error| credential_error("store", key, error))
    }

    fn delete(&self, key: &str) -> Result<()> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(credential_error("delete", key, error)),
        }
    }
}

fn credential_error(action: &str, key: &str, error: keyring::Error) -> Error {
    tracing::warn!(action, credential = key, %error, "OS credential store operation failed");
    Error::other(format!(
        "Could not {action} a secret in the operating system credential store: {error}"
    ))
}

#[derive(Clone)]
pub struct CredentialStore {
    backend: Arc<dyn SecretBackend>,
    cache: Arc<RwLock<HashMap<String, Option<String>>>>,
}

impl CredentialStore {
    pub fn system() -> Self {
        Self {
            backend: Arc::new(SystemKeyring),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        if let Some(secret) = self.cache.read().unwrap().get(key) {
            return Ok(secret.clone());
        }
        let secret = self.backend.get(key)?;
        self.cache
            .write()
            .unwrap()
            .insert(key.to_string(), secret.clone());
        Ok(secret)
    }

    pub fn set(&self, key: &str, secret: &str) -> Result<()> {
        if secret.is_empty() {
            self.delete(key)
        } else {
            self.backend.set(key, secret)?;
            self.cache
                .write()
                .unwrap()
                .insert(key.to_string(), Some(secret.to_string()));
            Ok(())
        }
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        self.backend.delete(key)?;
        self.cache.write().unwrap().insert(key.to_string(), None);
        Ok(())
    }

    pub fn available() -> bool {
        keyring::Entry::store_status().is_ok()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex, RwLock},
    };

    use super::{CredentialStore, SecretBackend};
    use crate::error::Result;

    #[derive(Default)]
    struct MemoryBackend(Mutex<HashMap<String, String>>);

    impl SecretBackend for MemoryBackend {
        fn get(&self, key: &str) -> Result<Option<String>> {
            Ok(self.0.lock().unwrap().get(key).cloned())
        }

        fn set(&self, key: &str, secret: &str) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .insert(key.to_string(), secret.to_string());
            Ok(())
        }

        fn delete(&self, key: &str) -> Result<()> {
            self.0.lock().unwrap().remove(key);
            Ok(())
        }
    }

    pub(crate) fn memory_store() -> CredentialStore {
        CredentialStore {
            backend: Arc::new(MemoryBackend::default()),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[test]
    fn empty_secret_removes_the_entry() {
        let store = memory_store();
        store.set("test", "secret").unwrap();
        assert_eq!(store.get("test").unwrap().as_deref(), Some("secret"));
        store.set("test", "").unwrap();
        assert_eq!(store.get("test").unwrap(), None);
    }
}
