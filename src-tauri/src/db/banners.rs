use rusqlite::{params, OptionalExtension};

use crate::error::Result;

use super::{models::BannerRecord, Db};

impl Db {
    pub fn list_banners(&self) -> Result<Vec<BannerRecord>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, file_name, original_name, kind, width, height, bytes, accent, added_at
             FROM banners ORDER BY added_at DESC",
        )?;
        let rows = stmt.query_map([], Self::read_banner)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn banner(&self, id: &str) -> Result<Option<BannerRecord>> {
        let conn = self.0.lock().unwrap();
        let found = conn
            .query_row(
                "SELECT id, file_name, original_name, kind, width, height, bytes, accent, added_at
                 FROM banners WHERE id = ?1",
                params![id],
                Self::read_banner,
            )
            .optional()?;
        Ok(found)
    }

    pub fn insert_banner(&self, banner: &BannerRecord) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO banners
                (id, file_name, original_name, kind, width, height, bytes, accent, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                original_name = coalesce(excluded.original_name, original_name),
                accent = coalesce(excluded.accent, accent)",
            params![
                banner.id,
                banner.file_name,
                banner.original_name,
                banner.kind,
                banner.width,
                banner.height,
                banner.bytes,
                banner.accent,
                banner.added_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete_banner(&self, id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE instances SET banner_id = NULL WHERE banner_id = ?1",
            params![id],
        )?;
        conn.execute("DELETE FROM banners WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn banner_users(&self, id: &str) -> Result<Vec<String>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT name FROM instances WHERE banner_id = ?1")?;
        let rows = stmt.query_map(params![id], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn set_instance_banner_id(&self, instance_id: &str, banner_id: Option<&str>) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE instances SET banner_id = ?2 WHERE id = ?1",
            params![instance_id, banner_id],
        )?;
        Ok(())
    }

    pub fn instance_banner_id(&self, instance_id: &str) -> Result<Option<String>> {
        let conn = self.0.lock().unwrap();
        let found: Option<Option<String>> = conn
            .query_row(
                "SELECT banner_id FROM instances WHERE id = ?1",
                params![instance_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(found.flatten())
    }

    fn read_banner(row: &rusqlite::Row) -> rusqlite::Result<BannerRecord> {
        Ok(BannerRecord {
            id: row.get(0)?,
            file_name: row.get(1)?,
            original_name: row.get(2)?,
            kind: row.get(3)?,
            width: row.get(4)?,
            height: row.get(5)?,
            bytes: row.get(6)?,
            accent: row.get(7)?,
            added_at: row.get(8)?,
        })
    }
}
