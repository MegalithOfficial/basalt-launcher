use rusqlite::{params, Connection};

use crate::error::Result;

pub(super) const SCHEMA_VERSION: i64 = 15;

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in names {
        if name? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    declaration: &str,
) -> Result<()> {
    if !column_exists(conn, table, column)? {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {declaration}"),
            [],
        )?;
    }
    Ok(())
}

pub(super) fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS settings(
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS instances(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            version_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            min_memory_mb INTEGER,
            max_memory_mb INTEGER,
            java_path TEXT,
            last_played_at INTEGER,
            playtime_secs INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS accounts(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            expires_at INTEGER NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS content_files(
            instance_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            file_name TEXT NOT NULL,
            sha1 TEXT,
            sha512 TEXT,
            murmur2 INTEGER,
            provider TEXT,
            project_id TEXT,
            version_id TEXT,
            title TEXT,
            icon_url TEXT,
            mod_id TEXT,
            mod_version TEXT,
            dependencies TEXT,
            origin TEXT NOT NULL DEFAULT 'user',
            pack_version_id TEXT,
            installed_at INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (instance_id, kind, file_name)
        );
        CREATE INDEX IF NOT EXISTS content_files_project
            ON content_files(instance_id, kind, project_id);
        CREATE INDEX IF NOT EXISTS content_files_sha1 ON content_files(sha1);
        CREATE TABLE IF NOT EXISTS content_updates(
            instance_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            file_name TEXT NOT NULL,
            latest_version_id TEXT NOT NULL,
            latest_name TEXT NOT NULL,
            latest_file_name TEXT NOT NULL,
            checked_at INTEGER NOT NULL,
            PRIMARY KEY (instance_id, kind, file_name)
        );
        CREATE TABLE IF NOT EXISTS server_content_files(
            server_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            file_name TEXT NOT NULL,
            sha1 TEXT,
            sha512 TEXT,
            murmur2 INTEGER,
            provider TEXT,
            project_id TEXT,
            version_id TEXT,
            title TEXT,
            icon_url TEXT,
            mod_id TEXT,
            mod_version TEXT,
            dependencies TEXT,
            origin TEXT NOT NULL DEFAULT 'user',
            pack_version_id TEXT,
            installed_at INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (server_id, kind, file_name)
        );
        CREATE INDEX IF NOT EXISTS server_content_files_project
            ON server_content_files(server_id, kind, project_id);
        CREATE TABLE IF NOT EXISTS server_content_updates(
            server_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            file_name TEXT NOT NULL,
            latest_version_id TEXT NOT NULL,
            latest_name TEXT NOT NULL,
            latest_file_name TEXT NOT NULL,
            checked_at INTEGER NOT NULL,
            PRIMARY KEY (server_id, kind, file_name)
        );
        CREATE TABLE IF NOT EXISTS world_datapacks(
            instance_id TEXT NOT NULL,
            world TEXT NOT NULL,
            file_name TEXT NOT NULL,
            sha1 TEXT,
            provider TEXT,
            project_id TEXT,
            version_id TEXT,
            title TEXT,
            icon_url TEXT,
            installed_at INTEGER NOT NULL DEFAULT 0,
            latest_version_id TEXT,
            latest_file_name TEXT,
            PRIMARY KEY (instance_id, world, file_name)
        );
        CREATE TABLE IF NOT EXISTS pending_operations(
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            instance_id TEXT,
            title TEXT NOT NULL,
            payload TEXT,
            started_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS active_runs(
            running_id TEXT PRIMARY KEY,
            instance_id TEXT NOT NULL,
            pid INTEGER NOT NULL,
            process_started_at INTEGER NOT NULL,
            started_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS api_cache(
            key TEXT PRIMARY KEY,
            body TEXT NOT NULL,
            etag TEXT,
            fetched_at INTEGER NOT NULL,
            ttl_secs INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS banners(
            id TEXT PRIMARY KEY,
            file_name TEXT NOT NULL,
            original_name TEXT,
            kind TEXT NOT NULL,
            width INTEGER,
            height INTEGER,
            bytes INTEGER NOT NULL,
            accent TEXT,
            added_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS instance_groups(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL COLLATE NOCASE UNIQUE,
            sort_order INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS servers(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            flavor TEXT NOT NULL,
            version_id TEXT NOT NULL,
            flavor_version TEXT,
            created_at TEXT NOT NULL,
            managed INTEGER NOT NULL DEFAULT 1,
            external_dir TEXT,
            launch_jar TEXT,
            launch_argfiles TEXT,
            min_memory_mb INTEGER,
            max_memory_mb INTEGER,
            java_path TEXT,
            jvm_args TEXT,
            jvm_args_mode TEXT,
            stop_timeout_secs INTEGER,
            eula_accepted_at INTEGER,
            installed_at INTEGER,
            last_started_at INTEGER,
            uptime_secs INTEGER NOT NULL DEFAULT 0,
            cached_port INTEGER,
            cached_motd TEXT,
            cached_max_players INTEGER,
            notes TEXT,
            sort_order INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS active_server_runs(
            running_id TEXT PRIMARY KEY,
            server_id TEXT NOT NULL,
            pid INTEGER NOT NULL,
            process_started_at INTEGER NOT NULL,
            started_at INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS active_server_runs_server
            ON active_server_runs(server_id);

        CREATE TABLE IF NOT EXISTS skins(
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            variant TEXT NOT NULL,
            file_name TEXT NOT NULL,
            source TEXT,
            hash TEXT,
            remote_hash TEXT,
            added_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS play_sessions(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            instance_id TEXT NOT NULL,
            instance_name TEXT NOT NULL,
            started_at INTEGER NOT NULL,
            ended_at INTEGER NOT NULL,
            played_secs INTEGER NOT NULL,
            crashed INTEGER NOT NULL DEFAULT 0,
            version_id TEXT,
            loader TEXT
        );
        CREATE INDEX IF NOT EXISTS play_sessions_started
            ON play_sessions(started_at);
        CREATE INDEX IF NOT EXISTS play_sessions_instance
            ON play_sessions(instance_id, started_at);",
    )?;

    if column_exists(conn, "accounts", "mc_access_token")?
        || column_exists(conn, "accounts", "refresh_token")?
    {
        conn.execute_batch(
            "PRAGMA secure_delete = ON;
             DROP TABLE accounts;
             CREATE TABLE accounts(
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                expires_at INTEGER NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 0
             );
             PRAGMA wal_checkpoint(TRUNCATE);
             VACUUM;
             PRAGMA wal_checkpoint(TRUNCATE);",
        )?;
    }

    for (column, declaration) in [
        ("loader", "TEXT"),
        ("loader_version", "TEXT"),
        ("launch_version_id", "TEXT"),
        ("pack_provider", "TEXT"),
        ("pack_project_id", "TEXT"),
        ("pack_version_id", "TEXT"),
        ("jvm_args", "TEXT"),
        ("jvm_args_mode", "TEXT"),
        ("env_vars", "TEXT"),
        ("env_vars_mode", "TEXT"),
        ("import_source", "TEXT"),
        ("import_source_id", "TEXT"),
        ("banner_id", "TEXT"),
        ("group_id", "TEXT"),
        ("group_order", "INTEGER NOT NULL DEFAULT 0"),
        ("notes", "TEXT"),
        ("wrapper_command", "TEXT"),
        ("pre_launch_command", "TEXT"),
        ("post_exit_command", "TEXT"),
    ] {
        add_column_if_missing(conn, "instances", column, declaration)?;
    }
    add_column_if_missing(conn, "skins", "hash", "TEXT")?;
    add_column_if_missing(conn, "skins", "remote_hash", "TEXT")?;
    add_column_if_missing(
        conn,
        "active_runs",
        "checkpointed_at",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    conn.execute(
        "UPDATE active_runs SET checkpointed_at = started_at WHERE checkpointed_at = 0",
        [],
    )?;

    if table_exists(conn, "content_sources")? {
        conn.execute_batch(
            "INSERT OR IGNORE INTO content_files
                (instance_id, kind, file_name, provider, project_id, version_id,
                 title, icon_url, origin, installed_at)
             SELECT instance_id, kind, file_name, provider, project_id, version_id,
                    title, icon_url, 'user', 0
             FROM content_sources;
             DROP TABLE content_sources;",
        )?;
    }

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{column_exists, migrate};

    #[test]
    fn migrate_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        migrate(&conn).unwrap();
        assert!(column_exists(&conn, "instances", "pack_provider").unwrap());
        assert!(column_exists(&conn, "instances", "loader").unwrap());
        assert!(column_exists(&conn, "content_files", "origin").unwrap());
        assert!(super::table_exists(&conn, "api_cache").unwrap());
        assert!(super::table_exists(&conn, "active_runs").unwrap());
        assert!(column_exists(&conn, "active_runs", "checkpointed_at").unwrap());
        assert!(super::table_exists(&conn, "instance_groups").unwrap());
        assert!(super::table_exists(&conn, "servers").unwrap());
        assert!(super::table_exists(&conn, "active_server_runs").unwrap());
        assert!(super::table_exists(&conn, "play_sessions").unwrap());
        assert!(column_exists(&conn, "instances", "group_id").unwrap());
        assert!(!column_exists(&conn, "accounts", "mc_access_token").unwrap());
    }

    #[test]
    fn migrate_initializes_existing_active_run_checkpoints() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE active_runs(
                running_id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                pid INTEGER NOT NULL,
                process_started_at INTEGER NOT NULL,
                started_at INTEGER NOT NULL
            );
            INSERT INTO active_runs VALUES ('run-1', 'instance-1', 42, 100, 1200);",
        )
        .unwrap();

        migrate(&conn).unwrap();

        let checkpointed_at: i64 = conn
            .query_row(
                "SELECT checkpointed_at FROM active_runs WHERE running_id = 'run-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(checkpointed_at, 1200);
    }

    #[test]
    fn migrate_moves_content_sources_forward() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE content_sources(
                instance_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                file_name TEXT NOT NULL,
                provider TEXT NOT NULL,
                project_id TEXT NOT NULL,
                version_id TEXT,
                title TEXT,
                icon_url TEXT,
                PRIMARY KEY (instance_id, kind, file_name)
            );
            INSERT INTO content_sources VALUES
                ('i1', 'mods', 'sodium.jar', 'modrinth', 'AANobbMI', 'abc', 'Sodium', NULL);",
        )
        .unwrap();
        migrate(&conn).unwrap();

        assert!(!super::table_exists(&conn, "content_sources").unwrap());
        let (project, origin): (String, String) = conn
            .query_row(
                "SELECT project_id, origin FROM content_files WHERE file_name = 'sodium.jar'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(project, "AANobbMI");
        assert_eq!(origin, "user");
    }

    #[test]
    fn migrate_heals_partial_state() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE instances(
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                version_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                min_memory_mb INTEGER,
                max_memory_mb INTEGER,
                java_path TEXT,
                last_played_at INTEGER,
                playtime_secs INTEGER NOT NULL DEFAULT 0
            );
            ALTER TABLE instances ADD COLUMN loader TEXT;
            ALTER TABLE instances ADD COLUMN loader_version TEXT;
            ALTER TABLE instances ADD COLUMN launch_version_id TEXT;
            ALTER TABLE instances ADD COLUMN pack_provider TEXT;
            ALTER TABLE instances ADD COLUMN pack_project_id TEXT;
            ALTER TABLE instances ADD COLUMN pack_version_id TEXT;
            PRAGMA user_version = 3;",
        )
        .unwrap();
        migrate(&conn).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, super::SCHEMA_VERSION);
        assert!(column_exists(&conn, "instances", "pack_version_id").unwrap());
    }
}
