use std::path::PathBuf;

use rusqlite::{params, OptionalExtension};

use crate::{
    error::Result,
    paths::Paths,
    servers::{Server, ServerFlavor},
};

use super::{ActiveServerRun, Db};

impl Db {
    fn read_server(row: &rusqlite::Row, paths: &Paths) -> rusqlite::Result<Server> {
        let id: String = row.get(0)?;
        let flavor: String = row.get(2)?;
        let created_at: String = row.get(4)?;
        let managed: bool = row.get(5)?;
        let external_dir: Option<String> = row.get(6)?;
        let dir = if managed {
            paths.server_dir(&id)
        } else {
            PathBuf::from(external_dir.unwrap_or_default())
        };
        let launch_argfiles: Option<String> = row.get(8)?;
        let port: Option<u32> = row.get(19)?;
        Ok(Server {
            available: dir.is_dir(),
            dir: dir.display().to_string(),
            id,
            name: row.get(1)?,
            flavor: ServerFlavor::parse(&flavor).unwrap_or(ServerFlavor::Vanilla),
            version_id: row.get(3)?,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
            managed,
            launch_jar: row.get(7)?,
            launch_argfiles: launch_argfiles
                .and_then(|value| serde_json::from_str(&value).ok())
                .unwrap_or_default(),
            flavor_version: row.get(9)?,
            min_memory_mb: row.get(10)?,
            max_memory_mb: row.get(11)?,
            java_path: row.get(12)?,
            jvm_args: row.get(13)?,
            jvm_args_mode: row.get(14)?,
            stop_timeout_secs: row.get(15)?,
            eula_accepted_at: row.get(16)?,
            installed_at: row.get(17)?,
            last_started_at: row.get(18)?,
            port: port.and_then(|value| u16::try_from(value).ok()),
            motd: row.get(20)?,
            max_players: row.get(21)?,
            uptime_secs: row.get(22)?,
            notes: row.get(23)?,
        })
    }

    pub fn list_servers(&self, paths: &Paths) -> Result<Vec<Server>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, flavor, version_id, created_at, managed, external_dir,
                    launch_jar, launch_argfiles, flavor_version, min_memory_mb, max_memory_mb,
                    java_path, jvm_args, jvm_args_mode, stop_timeout_secs, eula_accepted_at,
                    installed_at, last_started_at, cached_port, cached_motd,
                    cached_max_players, uptime_secs, notes
             FROM servers ORDER BY sort_order, created_at",
        )?;
        let rows = stmt.query_map([], |row| Self::read_server(row, paths))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn server(&self, paths: &Paths, server_id: &str) -> Result<Option<Server>> {
        let conn = self.0.lock().unwrap();
        let server = conn
            .query_row(
                "SELECT id, name, flavor, version_id, created_at, managed, external_dir,
                        launch_jar, launch_argfiles, flavor_version, min_memory_mb, max_memory_mb,
                        java_path, jvm_args, jvm_args_mode, stop_timeout_secs, eula_accepted_at,
                        installed_at, last_started_at, cached_port, cached_motd,
                        cached_max_players, uptime_secs, notes
                 FROM servers WHERE id = ?1",
                params![server_id],
                |row| Self::read_server(row, paths),
            )
            .optional()?;
        Ok(server)
    }

    pub fn imported_server_dirs(&self) -> Result<Vec<PathBuf>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT external_dir FROM servers
             WHERE managed = 0 AND external_dir IS NOT NULL AND external_dir <> ''",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(PathBuf::from)
            .collect())
    }

    pub fn insert_server(&self, server: &Server) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO servers
                (id, name, flavor, version_id, flavor_version, created_at, managed,
                 external_dir, launch_jar, launch_argfiles, min_memory_mb, max_memory_mb,
                 java_path, jvm_args, jvm_args_mode, stop_timeout_secs, eula_accepted_at,
                 installed_at, last_started_at, uptime_secs, cached_port, cached_motd,
                 cached_max_players, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                     ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)",
            params![
                server.id,
                server.name,
                server.flavor.as_str(),
                server.version_id,
                server.flavor_version,
                server.created_at.to_rfc3339(),
                server.managed,
                (!server.managed).then(|| server.dir.clone()),
                server.launch_jar,
                serde_json::to_string(&server.launch_argfiles)?,
                server.min_memory_mb,
                server.max_memory_mb,
                server.java_path,
                server.jvm_args,
                server.jvm_args_mode,
                server.stop_timeout_secs,
                server.eula_accepted_at,
                server.installed_at,
                server.last_started_at,
                server.uptime_secs,
                server.port.map(u32::from),
                server.motd,
                server.max_players,
                server.notes,
            ],
        )?;
        Ok(())
    }

    pub fn set_server_launch(
        &self,
        server_id: &str,
        launch_jar: Option<&str>,
        launch_argfiles: &[String],
        flavor_version: Option<&str>,
        installed_at: i64,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE servers SET
                launch_jar = ?2,
                launch_argfiles = ?3,
                flavor_version = coalesce(?4, flavor_version),
                installed_at = ?5
             WHERE id = ?1",
            params![
                server_id,
                launch_jar,
                serde_json::to_string(launch_argfiles)?,
                flavor_version,
                installed_at
            ],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_server_settings(
        &self,
        server_id: &str,
        name: &str,
        version_id: &str,
        flavor_version: Option<String>,
        min_memory_mb: Option<u32>,
        max_memory_mb: Option<u32>,
        java_path: Option<String>,
        jvm_args: Option<String>,
        jvm_args_mode: Option<String>,
        stop_timeout_secs: Option<u32>,
        notes: Option<String>,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE servers SET
                name = ?2,
                version_id = ?3,
                flavor_version = ?4,
                min_memory_mb = ?5,
                max_memory_mb = ?6,
                java_path = ?7,
                jvm_args = ?8,
                jvm_args_mode = ?9,
                stop_timeout_secs = ?10,
                notes = ?11
             WHERE id = ?1",
            params![
                server_id,
                name,
                version_id,
                flavor_version,
                min_memory_mb,
                max_memory_mb,
                java_path,
                jvm_args,
                jvm_args_mode,
                stop_timeout_secs,
                notes
            ],
        )?;
        Ok(())
    }

    pub fn clear_server_launch(&self, server_id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE servers SET launch_jar = NULL, launch_argfiles = '[]', installed_at = NULL
             WHERE id = ?1",
            params![server_id],
        )?;
        Ok(())
    }

    pub fn accept_server_eula(&self, server_id: &str, accepted_at: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE servers SET eula_accepted_at = ?2 WHERE id = ?1",
            params![server_id, accepted_at],
        )?;
        Ok(())
    }

    pub fn cache_server_properties(
        &self,
        server_id: &str,
        port: Option<u16>,
        motd: Option<&str>,
        max_players: Option<u32>,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE servers SET cached_port = ?2, cached_motd = ?3, cached_max_players = ?4
             WHERE id = ?1",
            params![server_id, port.map(u32::from), motd, max_players],
        )?;
        Ok(())
    }

    pub fn start_server_run(&self, server_id: &str, started_at: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE servers SET last_started_at = ?2 WHERE id = ?1",
            params![server_id, started_at],
        )?;
        Ok(())
    }

    pub fn add_server_uptime(&self, server_id: &str, secs: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE servers SET uptime_secs = uptime_secs + ?2 WHERE id = ?1",
            params![server_id, secs.max(0)],
        )?;
        Ok(())
    }

    pub fn delete_server(&self, server_id: &str) -> Result<bool> {
        let conn = self.0.lock().unwrap();
        let removed = conn.execute("DELETE FROM servers WHERE id = ?1", params![server_id])?;
        conn.execute(
            "DELETE FROM active_server_runs WHERE server_id = ?1",
            params![server_id],
        )?;
        Ok(removed > 0)
    }

    pub fn save_active_server_run(&self, run: &ActiveServerRun) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO active_server_runs
                (running_id, server_id, pid, process_started_at, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                run.running_id,
                run.server_id,
                i64::from(run.pid),
                run.process_started_at as i64,
                run.started_at,
            ],
        )?;
        Ok(())
    }

    pub fn active_server_runs(&self) -> Result<Vec<ActiveServerRun>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT running_id, server_id, pid, process_started_at, started_at
             FROM active_server_runs ORDER BY started_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ActiveServerRun {
                running_id: row.get(0)?,
                server_id: row.get(1)?,
                pid: row.get(2)?,
                process_started_at: row.get(3)?,
                started_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn clear_active_server_run(&self, running_id: &str) -> Result<bool> {
        let conn = self.0.lock().unwrap();
        let removed = conn.execute(
            "DELETE FROM active_server_runs WHERE running_id = ?1",
            params![running_id],
        )?;
        Ok(removed > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> Paths {
        Paths::plain(PathBuf::from("/tmp/basalt-servers-db"))
    }

    fn server(id: &str, managed: bool, dir: &str) -> Server {
        Server {
            id: id.into(),
            name: "Survival".into(),
            flavor: ServerFlavor::Paper,
            version_id: "1.21.8".into(),
            created_at: chrono::Utc::now(),
            managed,
            dir: dir.into(),
            available: true,
            flavor_version: Some("42".into()),
            launch_jar: None,
            launch_argfiles: Vec::new(),
            min_memory_mb: Some(1024),
            max_memory_mb: Some(4096),
            java_path: None,
            jvm_args: None,
            jvm_args_mode: None,
            stop_timeout_secs: None,
            eula_accepted_at: None,
            installed_at: None,
            last_started_at: None,
            uptime_secs: 0,
            port: Some(25565),
            motd: Some("A Basalt server".into()),
            max_players: Some(20),
            notes: None,
        }
    }

    #[test]
    fn a_managed_server_computes_its_directory() {
        let db = Db::open_in_memory().unwrap();
        let paths = paths();
        db.insert_server(&server("s1", true, "ignored")).unwrap();

        let stored = db.server(&paths, "s1").unwrap().unwrap();

        assert_eq!(stored.dir, paths.server_dir("s1").display().to_string());
        assert_eq!(stored.flavor, ServerFlavor::Paper);
        assert_eq!(stored.port, Some(25565));
        assert_eq!(stored.flavor_version.as_deref(), Some("42"));
        assert!(db.imported_server_dirs().unwrap().is_empty());
    }

    #[test]
    fn an_imported_server_keeps_the_folder_it_came_from() {
        let db = Db::open_in_memory().unwrap();
        let paths = paths();
        db.insert_server(&server("s2", false, "/mnt/disk/smp"))
            .unwrap();

        let stored = db.server(&paths, "s2").unwrap().unwrap();

        assert_eq!(stored.dir, "/mnt/disk/smp");
        assert!(!stored.managed);
        assert_eq!(
            db.imported_server_dirs().unwrap(),
            vec![PathBuf::from("/mnt/disk/smp")]
        );
    }

    #[test]
    fn the_launch_shape_and_the_eula_survive_a_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let paths = paths();
        db.insert_server(&server("s3", true, "")).unwrap();

        db.set_server_launch(
            "s3",
            None,
            &["user_jvm_args.txt".into(), "libraries/unix_args.txt".into()],
            Some("21.8.54"),
            99,
        )
        .unwrap();
        db.accept_server_eula("s3", 1234).unwrap();
        db.cache_server_properties("s3", Some(25570), Some("Hi"), Some(8))
            .unwrap();
        db.add_server_uptime("s3", 120).unwrap();

        let stored = db.server(&paths, "s3").unwrap().unwrap();
        assert_eq!(stored.launch_argfiles.len(), 2);
        assert!(stored.launch_jar.is_none());
        assert_eq!(stored.installed_at, Some(99));
        assert_eq!(stored.flavor_version.as_deref(), Some("21.8.54"));
        assert_eq!(stored.eula_accepted_at, Some(1234));
        assert_eq!(stored.port, Some(25570));
        assert_eq!(stored.max_players, Some(8));
        assert_eq!(stored.uptime_secs, 120);
    }

    #[test]
    fn deleting_a_server_takes_its_active_run_with_it() {
        let db = Db::open_in_memory().unwrap();
        db.insert_server(&server("s4", true, "")).unwrap();
        db.save_active_server_run(&ActiveServerRun {
            running_id: "run-1".into(),
            server_id: "s4".into(),
            pid: 42,
            process_started_at: 1234,
            started_at: 1200,
        })
        .unwrap();

        assert_eq!(db.active_server_runs().unwrap().len(), 1);
        assert!(db.delete_server("s4").unwrap());
        assert!(db.active_server_runs().unwrap().is_empty());
        assert!(!db.delete_server("s4").unwrap());
    }
}
