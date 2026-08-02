use rusqlite::{params, OptionalExtension};

use crate::{
    auth::account::{Account, AccountView},
    credentials::{microsoft_access_token, microsoft_refresh_token, CredentialStore},
    error::{Error, Result},
};

use super::Db;

impl Db {
    pub fn load_active_account(&self, credentials: &CredentialStore) -> Result<Option<Account>> {
        let account = {
            let conn = self.0.lock().unwrap();
            conn.query_row(
                "SELECT id, name, expires_at
                 FROM accounts WHERE is_active = 1 LIMIT 1",
                [],
                |row| {
                    Ok(Account {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        mc_access_token: String::new(),
                        refresh_token: String::new(),
                        expires_at: row.get(2)?,
                    })
                },
            )
            .optional()?
        };
        account
            .map(|account| hydrate_account(credentials, account))
            .transpose()
    }

    pub fn save_account(
        &self,
        credentials: &CredentialStore,
        account: &Account,
        make_active: bool,
    ) -> Result<()> {
        credentials.set(
            &microsoft_access_token(&account.id),
            &account.mc_access_token,
        )?;
        credentials.set(
            &microsoft_refresh_token(&account.id),
            &account.refresh_token,
        )?;

        let mut guard = self.0.lock().unwrap();
        let tx = guard.transaction()?;
        if make_active {
            tx.execute("UPDATE accounts SET is_active = 0", [])?;
        }
        tx.execute(
            "INSERT INTO accounts(id, name, expires_at, is_active)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                expires_at = excluded.expires_at,
                is_active = CASE WHEN excluded.is_active THEN 1 ELSE accounts.is_active END",
            params![account.id, account.name, account.expires_at, make_active],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_active_account_id(&self, account_id: &str) -> Result<bool> {
        let mut guard = self.0.lock().unwrap();
        let tx = guard.transaction()?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)",
            params![account_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(false);
        }
        tx.execute(
            "UPDATE accounts SET is_active = (id = ?1)",
            params![account_id],
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub fn remove_account_metadata(&self, account_id: &str) -> Result<usize> {
        let mut guard = self.0.lock().unwrap();
        let tx = guard.transaction()?;
        let was_active: Option<bool> = tx
            .query_row(
                "SELECT is_active FROM accounts WHERE id = ?1",
                params![account_id],
                |row| row.get(0),
            )
            .optional()?;
        tx.execute("DELETE FROM accounts WHERE id = ?1", params![account_id])?;
        if was_active == Some(true) {
            tx.execute(
                "UPDATE accounts SET is_active = 1
                 WHERE rowid = (SELECT min(rowid) FROM accounts)",
                [],
            )?;
        }
        let remaining = tx.query_row("SELECT count(*) FROM accounts", [], |row| row.get(0))?;
        tx.commit()?;
        Ok(remaining)
    }

    pub fn list_account_views(&self) -> Result<Vec<AccountView>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, name, is_active FROM accounts ORDER BY rowid")?;
        let rows = stmt.query_map([], |row| {
            Ok(AccountView {
                id: row.get(0)?,
                name: row.get(1)?,
                active: row.get(2)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn delete_account_credentials(
        &self,
        credentials: &CredentialStore,
        account_id: &str,
    ) -> Result<()> {
        credentials.delete(&microsoft_access_token(account_id))?;
        credentials.delete(&microsoft_refresh_token(account_id))
    }
}

fn hydrate_account(credentials: &CredentialStore, account: Account) -> Result<Account> {
    let access_key = microsoft_access_token(&account.id);
    let refresh_key = microsoft_refresh_token(&account.id);
    Ok(Account {
        mc_access_token: credentials.get(&access_key)?.ok_or_else(missing_sign_in)?,
        refresh_token: credentials.get(&refresh_key)?.ok_or_else(missing_sign_in)?,
        ..account
    })
}

fn missing_sign_in() -> Error {
    Error::other("This account's saved sign-in is missing. Remove the account and sign in again.")
}

#[cfg(test)]
mod tests {
    use crate::{auth::account::Account, credentials::tests::memory_store, db::Db};

    fn account() -> Account {
        Account {
            id: "account-id".to_string(),
            name: "Player".to_string(),
            mc_access_token: "access-secret".to_string(),
            refresh_token: "refresh-secret".to_string(),
            expires_at: 42,
        }
    }

    #[test]
    fn account_tokens_are_kept_out_of_sqlite() {
        let db = Db::open_in_memory().unwrap();
        let credentials = memory_store();
        db.save_account(&credentials, &account(), true).unwrap();

        let secret_columns: usize =
            db.0.lock()
                .unwrap()
                .query_row(
                    "SELECT count(*) FROM pragma_table_info('accounts')
                 WHERE name IN ('mc_access_token', 'refresh_token')",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
        assert_eq!(secret_columns, 0);
        let loaded = db.load_active_account(&credentials).unwrap().unwrap();
        assert_eq!(loaded.refresh_token, "refresh-secret");
    }
}
