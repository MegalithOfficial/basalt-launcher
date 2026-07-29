use rusqlite::params;

use crate::{
    auth::account::{Account, AccountStore},
    error::Result,
};

use super::Db;

impl Db {
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
        Ok(AccountStore {
            accounts,
            active_id,
        })
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
