use rusqlite::params;

use crate::error::Result;

use super::{ActiveRun, Db};

impl Db {
    pub fn save_active_run(&self, run: &ActiveRun) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO active_runs
                (running_id, instance_id, pid, process_started_at, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                run.running_id,
                run.instance_id,
                i64::from(run.pid),
                run.process_started_at as i64,
                run.started_at,
            ],
        )?;
        Ok(())
    }

    pub fn remove_active_run(&self, running_id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM active_runs WHERE running_id = ?1",
            params![running_id],
        )?;
        Ok(())
    }

    pub fn active_runs(&self) -> Result<Vec<ActiveRun>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT running_id, instance_id, pid, process_started_at, started_at
             FROM active_runs ORDER BY started_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ActiveRun {
                running_id: row.get(0)?,
                instance_id: row.get(1)?,
                pid: row.get(2)?,
                process_started_at: row.get(3)?,
                started_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_runs_round_trip_and_remove() {
        let db = Db::open_in_memory().unwrap();
        let run = ActiveRun {
            running_id: "run-1".into(),
            instance_id: "instance-1".into(),
            pid: 42,
            process_started_at: 1234,
            started_at: 1200,
        };

        db.save_active_run(&run).unwrap();
        assert_eq!(db.active_runs().unwrap(), vec![run]);

        db.remove_active_run("run-1").unwrap();
        assert!(db.active_runs().unwrap().is_empty());
    }
}
