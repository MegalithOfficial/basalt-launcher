use rusqlite::params;

use crate::error::Result;

use super::{ActiveRun, Db};

impl Db {
    pub fn save_active_run(&self, run: &ActiveRun) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO active_runs
                (running_id, instance_id, pid, process_started_at, started_at, checkpointed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                run.running_id,
                run.instance_id,
                i64::from(run.pid),
                run.process_started_at as i64,
                run.started_at,
                run.checkpointed_at,
            ],
        )?;
        Ok(())
    }

    pub fn checkpoint_active_run(&self, running_id: &str, checkpointed_at: i64) -> Result<bool> {
        let conn = self.0.lock().unwrap();
        let changed = conn.execute(
            "UPDATE active_runs
             SET checkpointed_at = MAX(checkpointed_at, ?2)
             WHERE running_id = ?1",
            params![running_id, checkpointed_at],
        )?;
        Ok(changed > 0)
    }

    pub fn active_runs(&self) -> Result<Vec<ActiveRun>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT running_id, instance_id, pid, process_started_at, started_at, checkpointed_at
             FROM active_runs ORDER BY started_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ActiveRun {
                running_id: row.get(0)?,
                instance_id: row.get(1)?,
                pid: row.get(2)?,
                process_started_at: row.get(3)?,
                started_at: row.get(4)?,
                checkpointed_at: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_runs_checkpoint_and_finalize_atomically() {
        let db = Db::open_in_memory().unwrap();
        db.0.lock()
            .unwrap()
            .execute(
                "INSERT INTO instances (id, name, version_id, created_at)
                 VALUES ('instance-1', 'Test', '1.21.8', '2026-08-07')",
                [],
            )
            .unwrap();
        let run = ActiveRun {
            running_id: "run-1".into(),
            instance_id: "instance-1".into(),
            pid: 42,
            process_started_at: 1234,
            started_at: 1200,
            checkpointed_at: 1200,
        };

        db.save_active_run(&run).unwrap();
        assert_eq!(db.active_runs().unwrap(), vec![run]);

        assert!(db.checkpoint_active_run("run-1", 1260).unwrap());
        assert_eq!(db.active_runs().unwrap()[0].checkpointed_at, 1260);
        assert!(db.checkpoint_active_run("run-1", 1230).unwrap());
        assert_eq!(db.active_runs().unwrap()[0].checkpointed_at, 1260);
        assert_eq!(db.play_stats(None, None).unwrap().session_count, 0);
        assert_eq!(db.play_stats(None, None).unwrap().lifetime_secs, 0);

        assert!(db.finalize_active_run("run-1", 1260, false).unwrap());
        assert!(db.active_runs().unwrap().is_empty());
        let stats = db.play_stats(None, None).unwrap();
        assert_eq!(stats.session_count, 1);
        assert_eq!(stats.lifetime_secs, 60);

        assert!(!db.finalize_active_run("run-1", 1320, false).unwrap());
        assert_eq!(db.play_stats(None, None).unwrap().session_count, 1);
        assert!(!db.checkpoint_active_run("run-1", 1320).unwrap());
    }
}
