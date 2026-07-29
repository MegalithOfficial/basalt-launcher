use rusqlite::{params, OptionalExtension};

use crate::error::Result;

use super::{ContentFile, ContentUpdate, Db};

impl Db {
    pub fn record_content_file(
        &self,
        instance_id: &str,
        kind: &str,
        file: &ContentFile,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO content_files
                (instance_id, kind, file_name, sha1, sha512, murmur2, provider, project_id,
                 version_id, title, icon_url, mod_id, mod_version, dependencies, origin,
                 pack_version_id, installed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                instance_id,
                kind,
                file.file_name,
                file.sha1,
                file.sha512,
                file.murmur2,
                file.provider,
                file.project_id,
                file.version_id,
                file.title,
                file.icon_url,
                file.mod_id,
                file.mod_version,
                file.dependencies,
                file.origin,
                file.pack_version_id,
                file.installed_at,
            ],
        )?;
        Ok(())
    }

    pub fn merge_identity(
        &self,
        instance_id: &str,
        kind: &str,
        file_name: &str,
        sha1: Option<&str>,
        sha512: Option<&str>,
        murmur2: Option<i64>,
        mod_id: Option<&str>,
        mod_version: Option<&str>,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO content_files
                (instance_id, kind, file_name, sha1, sha512, murmur2, mod_id, mod_version,
                 origin, installed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'manual', 0)
             ON CONFLICT(instance_id, kind, file_name) DO UPDATE SET
                sha1 = coalesce(excluded.sha1, sha1),
                sha512 = coalesce(excluded.sha512, sha512),
                murmur2 = coalesce(excluded.murmur2, murmur2),
                mod_id = coalesce(excluded.mod_id, mod_id),
                mod_version = coalesce(excluded.mod_version, mod_version)",
            params![
                instance_id,
                kind,
                file_name,
                sha1,
                sha512,
                murmur2,
                mod_id,
                mod_version
            ],
        )?;
        Ok(())
    }

    pub fn merge_provider_identity(
        &self,
        instance_id: &str,
        kind: &str,
        file_name: &str,
        provider: &str,
        project_id: &str,
        version_id: Option<&str>,
        title: Option<&str>,
        icon_url: Option<&str>,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE content_files SET
                provider = ?4,
                project_id = ?5,
                version_id = coalesce(?6, version_id),
                title = coalesce(?7, title),
                icon_url = coalesce(?8, icon_url)
             WHERE instance_id = ?1 AND kind = ?2 AND file_name = ?3",
            params![
                instance_id,
                kind,
                file_name,
                provider,
                project_id,
                version_id,
                title,
                icon_url
            ],
        )?;
        Ok(())
    }

    pub fn set_fallback_title(
        &self,
        instance_id: &str,
        kind: &str,
        file_name: &str,
        title: &str,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE content_files SET title = ?4
             WHERE instance_id = ?1 AND kind = ?2 AND file_name = ?3 AND title IS NULL",
            params![instance_id, kind, file_name, title],
        )?;
        Ok(())
    }

    fn read_content_file(row: &rusqlite::Row) -> rusqlite::Result<ContentFile> {
        Ok(ContentFile {
            file_name: row.get(0)?,
            sha1: row.get(1)?,
            sha512: row.get(2)?,
            murmur2: row.get(3)?,
            provider: row.get(4)?,
            project_id: row.get(5)?,
            version_id: row.get(6)?,
            title: row.get(7)?,
            icon_url: row.get(8)?,
            mod_id: row.get(9)?,
            mod_version: row.get(10)?,
            dependencies: row.get(11)?,
            origin: row.get(12)?,
            pack_version_id: row.get(13)?,
            installed_at: row.get(14)?,
        })
    }

    pub fn content_files(&self, instance_id: &str, kind: &str) -> Result<Vec<ContentFile>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT file_name, sha1, sha512, murmur2, provider, project_id, version_id,
                    title, icon_url, mod_id, mod_version, dependencies, origin,
                    pack_version_id, installed_at
             FROM content_files WHERE instance_id = ?1 AND kind = ?2",
        )?;
        let rows = stmt.query_map(params![instance_id, kind], Self::read_content_file)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn content_file(
        &self,
        instance_id: &str,
        kind: &str,
        file_name: &str,
    ) -> Result<Option<ContentFile>> {
        let conn = self.0.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT file_name, sha1, sha512, murmur2, provider, project_id, version_id,
                        title, icon_url, mod_id, mod_version, dependencies, origin,
                        pack_version_id, installed_at
                 FROM content_files WHERE instance_id = ?1 AND kind = ?2 AND file_name = ?3",
                params![instance_id, kind, file_name],
                Self::read_content_file,
            )
            .optional()?;
        Ok(result)
    }

    pub fn installed_project_file(
        &self,
        instance_id: &str,
        kind: &str,
        project_id: &str,
    ) -> Result<Option<(Option<String>, String)>> {
        let conn = self.0.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT version_id, file_name FROM content_files
                 WHERE instance_id = ?1 AND kind = ?2 AND project_id = ?3
                 ORDER BY installed_at DESC LIMIT 1",
                params![instance_id, kind, project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(result)
    }

    pub fn delete_content_file(
        &self,
        instance_id: &str,
        kind: &str,
        file_name: &str,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM content_files
             WHERE instance_id = ?1 AND kind = ?2 AND file_name = ?3",
            params![instance_id, kind, file_name],
        )?;
        conn.execute(
            "DELETE FROM content_updates
             WHERE instance_id = ?1 AND kind = ?2 AND file_name = ?3",
            params![instance_id, kind, file_name],
        )?;
        Ok(())
    }

    pub fn delete_instance_content_files(&self, instance_id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        for table in ["content_files", "content_updates"] {
            conn.execute(
                &format!("DELETE FROM {table} WHERE instance_id = ?1"),
                params![instance_id],
            )?;
        }
        Ok(())
    }

    pub fn replace_content_updates(
        &self,
        instance_id: &str,
        updates: &[ContentUpdate],
        checked_at: i64,
    ) -> Result<()> {
        let mut guard = self.0.lock().unwrap();
        let tx = guard.transaction()?;
        tx.execute(
            "DELETE FROM content_updates WHERE instance_id = ?1",
            params![instance_id],
        )?;
        for update in updates {
            tx.execute(
                "INSERT OR REPLACE INTO content_updates
                    (instance_id, kind, file_name, latest_version_id, latest_name,
                     latest_file_name, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    instance_id,
                    update.kind,
                    update.file_name,
                    update.latest_version_id,
                    update.latest_name,
                    update.latest_file_name,
                    checked_at
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn content_updates(&self, instance_id: &str) -> Result<Vec<ContentUpdate>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT kind, file_name, latest_version_id, latest_name, latest_file_name
             FROM content_updates WHERE instance_id = ?1",
        )?;
        let rows = stmt.query_map(params![instance_id], |row| {
            Ok(ContentUpdate {
                kind: row.get(0)?,
                file_name: row.get(1)?,
                latest_version_id: row.get(2)?,
                latest_name: row.get(3)?,
                latest_file_name: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn updates_checked_at(&self, instance_id: &str) -> Result<Option<i64>> {
        let conn = self.0.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT max(checked_at) FROM content_updates WHERE instance_id = ?1",
                params![instance_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?;
        Ok(result.flatten())
    }
}
