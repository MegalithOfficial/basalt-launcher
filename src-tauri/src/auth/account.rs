use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::paths::Paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub mc_access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountStore {
    pub accounts: Vec<Account>,
    pub active_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountView {
    pub id: String,
    pub name: String,
    pub active: bool,
}

pub fn load(paths: &Paths) -> Result<AccountStore> {
    match std::fs::read(paths.accounts_file()) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AccountStore::default()),
        Err(e) => Err(e.into()),
    }
}

pub fn save(paths: &Paths, store: &AccountStore) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(store)?;
    let path = paths.accounts_file();
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

impl AccountStore {
    pub fn upsert_active(&mut self, account: Account) {
        let id = account.id.clone();
        if let Some(existing) = self.accounts.iter_mut().find(|a| a.id == id) {
            *existing = account;
        } else {
            self.accounts.push(account);
        }
        self.active_id = Some(id);
    }

    pub fn views(&self) -> Vec<AccountView> {
        self.accounts
            .iter()
            .map(|a| AccountView {
                id: a.id.clone(),
                name: a.name.clone(),
                active: self.active_id.as_deref() == Some(a.id.as_str()),
            })
            .collect()
    }

    pub fn active(&self) -> Option<&Account> {
        let id = self.active_id.as_deref()?;
        self.accounts.iter().find(|a| a.id == id)
    }
}
