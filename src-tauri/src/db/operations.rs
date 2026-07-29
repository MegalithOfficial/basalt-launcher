use rusqlite::params;

use crate::error::Result;

use super::{Db, PendingOperation};

impl Db {
    pub fn begin_operation(&self, op: &PendingOperation) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO pending_operations
                (id, kind, instance_id, title, payload, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                op.id,
                op.kind,
                op.instance_id,
                op.title,
                op.payload,
                op.started_at
            ],
        )?;
        Ok(())
    }

    pub fn end_operation(&self, id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM pending_operations WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn pending_operations(&self) -> Result<Vec<PendingOperation>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, kind, instance_id, title, payload, started_at
             FROM pending_operations ORDER BY started_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PendingOperation {
                id: row.get(0)?,
                kind: row.get(1)?,
                instance_id: row.get(2)?,
                title: row.get(3)?,
                payload: row.get(4)?,
                started_at: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn clear_pending_operations(&self) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM pending_operations", [])?;
        Ok(())
    }
}
