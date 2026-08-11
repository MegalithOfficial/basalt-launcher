use rusqlite::{params, OptionalExtension};

use crate::{
    config::Instance,
    error::{Error, Result},
};

use super::{ContentFile, ContentUpdate, Db};

impl Db {
    pub fn all_content_files(&self, instance_id: &str) -> Result<Vec<(String, ContentFile)>> {
        let conn = self.0.lock().unwrap();
        let mut statement = conn.prepare(
            "SELECT kind, file_name, sha1, sha512, murmur2, provider, project_id, version_id,
                    title, icon_url, mod_id, mod_version, dependencies, origin,
                    pack_version_id, installed_at
             FROM content_files WHERE instance_id = ?1 ORDER BY kind, file_name",
        )?;
        let rows = statement.query_map([instance_id], |row| {
            Ok((
                row.get(0)?,
                ContentFile {
                    file_name: row.get(1)?,
                    sha1: row.get(2)?,
                    sha512: row.get(3)?,
                    murmur2: row.get(4)?,
                    provider: row.get(5)?,
                    project_id: row.get(6)?,
                    version_id: row.get(7)?,
                    title: row.get(8)?,
                    icon_url: row.get(9)?,
                    mod_id: row.get(10)?,
                    mod_version: row.get(11)?,
                    dependencies: row.get(12)?,
                    origin: row.get(13)?,
                    pack_version_id: row.get(14)?,
                    installed_at: row.get(15)?,
                },
            ))
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn restore_instance_snapshot(
        &self,
        instance_id: &str,
        snapshot: &Instance,
        content: &[(String, ContentFile)],
    ) -> Result<()> {
        let mut conn = self.0.lock().unwrap();
        let transaction = conn.transaction()?;
        let changed = transaction.execute(
            "UPDATE instances SET
                version_id = ?2, min_memory_mb = ?3, max_memory_mb = ?4, java_path = ?5,
                loader = ?6, loader_version = ?7, launch_version_id = ?8,
                pack_provider = ?9, pack_project_id = ?10, pack_version_id = ?11,
                jvm_args = ?12, jvm_args_mode = ?13, env_vars = ?14, env_vars_mode = ?15
             WHERE id = ?1",
            params![
                instance_id,
                snapshot.version_id,
                snapshot.min_memory_mb,
                snapshot.max_memory_mb,
                snapshot.java_path,
                snapshot.loader,
                snapshot.loader_version,
                snapshot.launch_version_id,
                snapshot.pack_provider,
                snapshot.pack_project_id,
                snapshot.pack_version_id,
                snapshot.jvm_args,
                snapshot.jvm_args_mode,
                snapshot.env_vars,
                snapshot.env_vars_mode,
            ],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("instance".to_string()));
        }
        transaction.execute(
            "DELETE FROM content_files WHERE instance_id = ?1",
            [instance_id],
        )?;
        transaction.execute(
            "DELETE FROM content_updates WHERE instance_id = ?1",
            [instance_id],
        )?;
        {
            let mut insert = transaction.prepare(
                "INSERT INTO content_files
                    (instance_id, kind, file_name, sha1, sha512, murmur2, provider, project_id,
                     version_id, title, icon_url, mod_id, mod_version, dependencies, origin,
                     pack_version_id, installed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                         ?15, ?16, ?17)",
            )?;
            for (kind, file) in content {
                insert.execute(params![
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
                ])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

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

    #[allow(clippy::too_many_arguments)]
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

    #[allow(clippy::too_many_arguments)]
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

    pub(super) fn read_content_file(row: &rusqlite::Row) -> rusqlite::Result<ContentFile> {
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

    pub fn delete_pack_content_files(&self, instance_id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM content_files WHERE instance_id = ?1 AND origin = 'pack'",
            params![instance_id],
        )?;
        conn.execute(
            "DELETE FROM content_updates WHERE instance_id = ?1",
            params![instance_id],
        )?;
        Ok(())
    }

    pub fn clone_instance_content(&self, source_id: &str, destination_id: &str) -> Result<()> {
        let mut guard = self.0.lock().unwrap();
        let transaction = guard.transaction()?;
        transaction.execute(
            "INSERT INTO content_files
                (instance_id, kind, file_name, sha1, sha512, murmur2, provider, project_id,
                 version_id, title, icon_url, mod_id, mod_version, dependencies, origin,
                 pack_version_id, installed_at)
             SELECT ?2, kind, file_name, sha1, sha512, murmur2, provider, project_id,
                    version_id, title, icon_url, mod_id, mod_version, dependencies, origin,
                    pack_version_id, installed_at
             FROM content_files WHERE instance_id = ?1",
            params![source_id, destination_id],
        )?;
        transaction.execute(
            "INSERT INTO content_updates
                (instance_id, kind, file_name, latest_version_id, latest_name,
                 latest_file_name, checked_at)
             SELECT ?2, kind, file_name, latest_version_id, latest_name,
                    latest_file_name, checked_at
             FROM content_updates WHERE instance_id = ?1",
            params![source_id, destination_id],
        )?;
        transaction.commit()?;
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

    pub fn clear_all_content_updates(&self) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM content_updates", [])?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(version: &str) -> Instance {
        Instance {
            id: "instance".into(),
            name: "Current name".into(),
            version_id: version.into(),
            created_at: chrono::Utc::now(),
            min_memory_mb: Some(1024),
            max_memory_mb: Some(4096),
            java_path: None,
            last_played_at: Some(10),
            playtime_secs: 20,
            dir: String::new(),
            logo: None,
            loader: Some("fabric".into()),
            loader_version: Some("1".into()),
            launch_version_id: Some(format!("fabric-{version}")),
            pack_provider: Some("modrinth".into()),
            pack_project_id: Some("pack".into()),
            pack_version_id: Some("old-pack".into()),
            jvm_args: None,
            jvm_args_mode: None,
            env_vars: None,
            env_vars_mode: None,
            import_source: None,
            import_source_id: None,
            banner_id: None,
            notes: None,
            wrapper_command: None,
            pre_launch_command: None,
            post_exit_command: None,
        }
    }

    #[test]
    fn clone_instance_content_keeps_identity_and_updates() {
        let db = Db::open_in_memory().unwrap();
        db.record_content_file(
            "source",
            "mods",
            &ContentFile {
                file_name: "example.jar".into(),
                sha1: Some("abc".into()),
                provider: Some("modrinth".into()),
                project_id: Some("project".into()),
                version_id: Some("version".into()),
                title: Some("Example".into()),
                origin: "user".into(),
                installed_at: 42,
                ..Default::default()
            },
        )
        .unwrap();
        db.replace_content_updates(
            "source",
            &[ContentUpdate {
                kind: "mods".into(),
                file_name: "example.jar".into(),
                latest_version_id: "next".into(),
                latest_name: "Next".into(),
                latest_file_name: "example-next.jar".into(),
            }],
            50,
        )
        .unwrap();

        db.clone_instance_content("source", "copy").unwrap();

        let file = db
            .content_file("copy", "mods", "example.jar")
            .unwrap()
            .unwrap();
        assert_eq!(file.project_id.as_deref(), Some("project"));
        assert_eq!(file.installed_at, 42);
        assert_eq!(
            db.content_updates("copy").unwrap()[0].latest_version_id,
            "next"
        );
    }

    #[test]
    fn restoring_snapshot_metadata_is_transactional_and_preserves_identity() {
        let db = Db::open_in_memory().unwrap();
        let mut current = instance("1.21.1");
        current.name = "Do not rename me".into();
        current.loader = Some("neoforge".into());
        current.pack_version_id = Some("new-pack".into());
        db.insert_instance(&current).unwrap();
        let group = db.create_instance_group("Grouped").unwrap();
        db.move_instance_to_group(&current.id, Some(&group.id))
            .unwrap();
        db.record_content_file(
            &current.id,
            "mods",
            &ContentFile {
                file_name: "new.jar".into(),
                origin: "user".into(),
                ..Default::default()
            },
        )
        .unwrap();

        let snapshot = instance("1.20.1");
        let old_content = vec![(
            "mods".into(),
            ContentFile {
                file_name: "old.jar".into(),
                origin: "pack".into(),
                ..Default::default()
            },
        )];
        db.restore_instance_snapshot(&current.id, &snapshot, &old_content)
            .unwrap();

        let restored: (String, String, Option<String>) =
            db.0.lock()
                .unwrap()
                .query_row(
                    "SELECT name, version_id, loader FROM instances WHERE id = 'instance'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
        assert_eq!(restored.0, "Do not rename me");
        assert_eq!(restored.1, "1.20.1");
        assert_eq!(restored.2.as_deref(), Some("fabric"));
        assert_eq!(
            db.content_files(&current.id, "mods").unwrap()[0].file_name,
            "old.jar"
        );
        assert_eq!(
            db.instance_organization().unwrap().placements[0]
                .group_id
                .as_deref(),
            Some(group.id.as_str())
        );
    }
}
