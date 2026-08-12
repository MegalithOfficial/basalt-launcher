use rusqlite::{params, OptionalExtension};

use crate::error::Result;

use super::{ContentFile, Db};

const COLUMNS: &str = "file_name, sha1, sha512, murmur2, provider, project_id, version_id,
                       title, icon_url, mod_id, mod_version, dependencies, origin,
                       pack_version_id, installed_at";

impl Db {
    pub fn has_server_pack_content(&self, server_id: &str, pack_version_id: &str) -> Result<bool> {
        let conn = self.0.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT count(*) FROM server_content_files
             WHERE server_id = ?1 AND origin = 'pack' AND pack_version_id = ?2
               AND provider = 'curseforge'",
            params![server_id, pack_version_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn record_server_content_file(
        &self,
        server_id: &str,
        kind: &str,
        file: &ContentFile,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO server_content_files
                (server_id, kind, file_name, sha1, sha512, murmur2, provider, project_id,
                 version_id, title, icon_url, mod_id, mod_version, dependencies, origin,
                 pack_version_id, installed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                server_id,
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

    pub fn server_content_files(&self, server_id: &str, kind: &str) -> Result<Vec<ContentFile>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM server_content_files WHERE server_id = ?1 AND kind = ?2"
        ))?;
        let rows = stmt.query_map(params![server_id, kind], Self::read_content_file)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn delete_server_content_file(
        &self,
        server_id: &str,
        kind: &str,
        file_name: &str,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM server_content_files
             WHERE server_id = ?1 AND kind = ?2 AND file_name = ?3",
            params![server_id, kind, file_name],
        )?;
        conn.execute(
            "DELETE FROM server_content_updates
             WHERE server_id = ?1 AND kind = ?2 AND file_name = ?3",
            params![server_id, kind, file_name],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn merge_server_identity(
        &self,
        server_id: &str,
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
            "INSERT INTO server_content_files
                (server_id, kind, file_name, sha1, sha512, murmur2, mod_id, mod_version,
                 origin, installed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'manual', 0)
             ON CONFLICT(server_id, kind, file_name) DO UPDATE SET
                sha1 = coalesce(excluded.sha1, sha1),
                sha512 = coalesce(excluded.sha512, sha512),
                murmur2 = coalesce(excluded.murmur2, murmur2),
                mod_id = coalesce(excluded.mod_id, mod_id),
                mod_version = coalesce(excluded.mod_version, mod_version)",
            params![
                server_id,
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

    #[allow(clippy::too_many_arguments)]
    pub fn merge_server_provider_identity(
        &self,
        server_id: &str,
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
            "UPDATE server_content_files SET
                provider = ?4,
                project_id = ?5,
                version_id = coalesce(?6, version_id),
                title = coalesce(?7, title),
                icon_url = coalesce(?8, icon_url)
             WHERE server_id = ?1 AND kind = ?2 AND file_name = ?3",
            params![server_id, kind, file_name, provider, project_id, version_id, title, icon_url],
        )?;
        Ok(())
    }

    pub fn set_server_fallback_title(
        &self,
        server_id: &str,
        kind: &str,
        file_name: &str,
        title: &str,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE server_content_files SET title = ?4
             WHERE server_id = ?1 AND kind = ?2 AND file_name = ?3 AND title IS NULL",
            params![server_id, kind, file_name, title],
        )?;
        Ok(())
    }

    pub fn replace_server_content_updates(
        &self,
        server_id: &str,
        updates: &[super::ContentUpdate],
        checked_at: i64,
    ) -> Result<()> {
        let mut guard = self.0.lock().unwrap();
        let tx = guard.transaction()?;
        tx.execute(
            "DELETE FROM server_content_updates WHERE server_id = ?1",
            params![server_id],
        )?;
        for update in updates {
            tx.execute(
                "INSERT OR REPLACE INTO server_content_updates
                    (server_id, kind, file_name, latest_version_id, latest_name,
                     latest_file_name, checked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    server_id,
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

    pub fn server_content_updates(&self, server_id: &str) -> Result<Vec<super::ContentUpdate>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT kind, file_name, latest_version_id, latest_name, latest_file_name
             FROM server_content_updates WHERE server_id = ?1",
        )?;
        let rows = stmt.query_map(params![server_id], |row| {
            Ok(super::ContentUpdate {
                kind: row.get(0)?,
                file_name: row.get(1)?,
                latest_version_id: row.get(2)?,
                latest_name: row.get(3)?,
                latest_file_name: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn server_updates_checked_at(&self, server_id: &str) -> Result<Option<i64>> {
        let conn = self.0.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT MAX(checked_at) FROM server_content_updates WHERE server_id = ?1",
                params![server_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?;
        Ok(result.flatten())
    }

    pub fn forget_server_content(&self, server_id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM server_content_files WHERE server_id = ?1",
            params![server_id],
        )?;
        conn.execute(
            "DELETE FROM server_content_updates WHERE server_id = ?1",
            params![server_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(name: &str, project: &str) -> ContentFile {
        ContentFile {
            file_name: name.to_string(),
            sha1: None,
            sha512: None,
            murmur2: None,
            provider: Some("modrinth".to_string()),
            project_id: Some(project.to_string()),
            version_id: Some("v1".to_string()),
            title: Some("EssentialsX".to_string()),
            icon_url: None,
            mod_id: None,
            mod_version: None,
            dependencies: None,
            origin: "user".to_string(),
            pack_version_id: None,
            installed_at: 10,
        }
    }

    #[test]
    fn a_server_and_an_instance_never_see_each_other_content() {
        let db = Db::open_in_memory().unwrap();
        db.record_server_content_file("s1", "mods", &record("Essentials.jar", "p1"))
            .unwrap();
        db.record_content_file("s1", "mods", &record("Essentials.jar", "p1"))
            .unwrap();

        db.delete_server_content_file("s1", "mods", "Essentials.jar")
            .unwrap();

        assert!(db.server_content_files("s1", "mods").unwrap().is_empty());
        assert_eq!(db.content_files("s1", "mods").unwrap().len(), 1);
    }

    #[test]
    fn a_server_knows_when_its_pack_content_graph_was_recorded() {
        let db = Db::open_in_memory().unwrap();
        let mut file = record("PackMod.jar", "p1");
        file.origin = "pack".to_string();
        file.pack_version_id = Some("pack-v1".to_string());
        db.record_server_content_file("s1", "mods", &file).unwrap();
        assert!(!db.has_server_pack_content("s1", "pack-v1").unwrap());

        file.provider = Some("curseforge".to_string());
        db.record_server_content_file("s1", "mods", &file).unwrap();

        assert!(db.has_server_pack_content("s1", "pack-v1").unwrap());
        assert!(!db.has_server_pack_content("s1", "pack-v2").unwrap());
    }

    #[test]
    fn matching_a_pack_file_keeps_its_pack_ownership() {
        let db = Db::open_in_memory().unwrap();
        let mut file = record("PackMod.jar", "curseforge-project");
        file.provider = Some("curseforge".to_string());
        file.origin = "pack".to_string();
        file.pack_version_id = Some("pack-v1".to_string());
        db.record_server_content_file("s1", "mods", &file).unwrap();

        db.merge_server_provider_identity(
            "s1",
            "mods",
            "PackMod.jar",
            "modrinth",
            "modrinth-project",
            Some("modrinth-version"),
            None,
            None,
        )
        .unwrap();

        let linked = db.server_content_files("s1", "mods").unwrap().remove(0);
        assert_eq!(linked.provider.as_deref(), Some("modrinth"));
        assert_eq!(linked.project_id.as_deref(), Some("modrinth-project"));
        assert_eq!(linked.origin, "pack");
        assert_eq!(linked.pack_version_id.as_deref(), Some("pack-v1"));
    }
}
