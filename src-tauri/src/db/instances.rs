use rusqlite::params;

use crate::{config::Instance, error::Result, files::FileManager};

use super::Db;

impl Db {
    pub fn list_instances(&self, files: &FileManager) -> Result<Vec<Instance>> {
        let paths = files.paths();
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, version_id, created_at, min_memory_mb, max_memory_mb,
                    java_path, last_played_at, playtime_secs, loader, loader_version,
                    launch_version_id, pack_provider, pack_project_id, pack_version_id,
                    jvm_args, jvm_args_mode, env_vars, env_vars_mode,
                    import_source, import_source_id
             FROM instances ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let created_at: String = row.get(3)?;
            Ok(Instance {
                dir: paths.instance_dir(&id).display().to_string(),
                logo: crate::meta::media::instance_logo(files, &id),
                id,
                name: row.get(1)?,
                version_id: row.get(2)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now()),
                min_memory_mb: row.get(4)?,
                max_memory_mb: row.get(5)?,
                java_path: row.get(6)?,
                last_played_at: row.get(7)?,
                playtime_secs: row.get(8)?,
                loader: row.get(9)?,
                loader_version: row.get(10)?,
                launch_version_id: row.get(11)?,
                pack_provider: row.get(12)?,
                pack_project_id: row.get(13)?,
                pack_version_id: row.get(14)?,
                jvm_args: row.get(15)?,
                jvm_args_mode: row.get(16)?,
                env_vars: row.get(17)?,
                env_vars_mode: row.get(18)?,
                import_source: row.get(19)?,
                import_source_id: row.get(20)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn insert_instance(&self, instance: &Instance) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO instances
                (id, name, version_id, created_at, min_memory_mb, max_memory_mb,
                 java_path, last_played_at, playtime_secs, loader, loader_version,
                 launch_version_id, pack_provider, pack_project_id, pack_version_id,
                 jvm_args, jvm_args_mode, env_vars, env_vars_mode,
                 import_source, import_source_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                     ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                instance.id,
                instance.name,
                instance.version_id,
                instance.created_at.to_rfc3339(),
                instance.min_memory_mb,
                instance.max_memory_mb,
                instance.java_path,
                instance.last_played_at,
                instance.playtime_secs,
                instance.loader,
                instance.loader_version,
                instance.launch_version_id,
                instance.pack_provider,
                instance.pack_project_id,
                instance.pack_version_id,
                instance.jvm_args,
                instance.jvm_args_mode,
                instance.env_vars,
                instance.env_vars_mode,
                instance.import_source,
                instance.import_source_id,
            ],
        )?;
        Ok(())
    }

    pub fn update_instance_settings(
        &self,
        instance_id: &str,
        name: &str,
        min_memory_mb: Option<u32>,
        max_memory_mb: Option<u32>,
        java_path: Option<String>,
        loader: Option<String>,
        loader_version: Option<String>,
        version_id: &str,
        jvm_args: Option<String>,
        jvm_args_mode: Option<String>,
        env_vars: Option<String>,
        env_vars_mode: Option<String>,
        reset_launch_version: bool,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        if reset_launch_version {
            conn.execute(
                "UPDATE instances
                 SET name = ?2, min_memory_mb = ?3, max_memory_mb = ?4, java_path = ?5,
                     loader = ?6, loader_version = ?7, version_id = ?8,
                     jvm_args = ?9, jvm_args_mode = ?10, env_vars = ?11, env_vars_mode = ?12,
                     launch_version_id = NULL
                 WHERE id = ?1",
                params![
                    instance_id,
                    name,
                    min_memory_mb,
                    max_memory_mb,
                    java_path,
                    loader,
                    loader_version,
                    version_id,
                    jvm_args,
                    jvm_args_mode,
                    env_vars,
                    env_vars_mode
                ],
            )?;
        } else {
            conn.execute(
                "UPDATE instances
                 SET name = ?2, min_memory_mb = ?3, max_memory_mb = ?4, java_path = ?5,
                     loader = ?6, loader_version = ?7, version_id = ?8,
                     jvm_args = ?9, jvm_args_mode = ?10, env_vars = ?11, env_vars_mode = ?12
                 WHERE id = ?1",
                params![
                    instance_id,
                    name,
                    min_memory_mb,
                    max_memory_mb,
                    java_path,
                    loader,
                    loader_version,
                    version_id,
                    jvm_args,
                    jvm_args_mode,
                    env_vars,
                    env_vars_mode
                ],
            )?;
        }
        Ok(())
    }

    pub fn imported_sources(&self, source: &str) -> Result<Vec<String>> {
        let conn = self.0.lock().unwrap();
        let mut statement = conn.prepare(
            "SELECT import_source_id FROM instances
             WHERE import_source = ?1 AND import_source_id IS NOT NULL",
        )?;
        let rows = statement.query_map([source], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn delete_instance(&self, instance_id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM instances WHERE id = ?1", params![instance_id])?;
        Ok(())
    }

    pub fn set_launch_version(&self, instance_id: &str, launch_version_id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE instances SET launch_version_id = ?2 WHERE id = ?1",
            params![instance_id, launch_version_id],
        )?;
        Ok(())
    }

    pub fn record_playtime(
        &self,
        instance_id: &str,
        played_secs: i64,
        ended_at: i64,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE instances
             SET playtime_secs = playtime_secs + ?2, last_played_at = ?3
             WHERE id = ?1",
            params![instance_id, played_secs.max(0), ended_at],
        )?;
        Ok(())
    }
}
