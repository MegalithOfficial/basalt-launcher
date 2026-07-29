use rusqlite::{params, OptionalExtension};

use crate::error::Result;

use super::{Db, SkinRecord};

impl Db {
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
        conn.execute(
            "UPDATE skins SET hash = ?2 WHERE id = ?1",
            params![id, hash],
        )?;
        Ok(())
    }

    pub fn enforce_unique_skin_hashes(&self) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS skins_hash_unique ON skins(hash)")?;
        Ok(())
    }

    pub fn rename_skin(&self, id: &str, name: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE skins SET name = ?2 WHERE id = ?1",
            params![id, name],
        )?;
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
}
