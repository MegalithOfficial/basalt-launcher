use rusqlite::{params, OptionalExtension};

use crate::error::Result;

use super::{CachedResponse, Db};

pub const CURSEFORGE_CACHE_PREFIX: &str = "cf:";

impl Db {
    pub fn cache_get(&self, key: &str, now: i64) -> Result<Option<CachedResponse>> {
        let conn = self.0.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT body, etag, fetched_at, ttl_secs FROM api_cache WHERE key = ?1",
                params![key],
                |row| {
                    let fetched_at: i64 = row.get(2)?;
                    let ttl_secs: i64 = row.get(3)?;
                    Ok(CachedResponse {
                        body: row.get(0)?,
                        etag: row.get(1)?,
                        fresh: now - fetched_at < ttl_secs,
                        age_secs: (now - fetched_at).max(0),
                    })
                },
            )
            .optional()?;
        Ok(result)
    }

    pub fn cache_put(
        &self,
        key: &str,
        body: &str,
        etag: Option<&str>,
        fetched_at: i64,
        ttl_secs: i64,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO api_cache(key, body, etag, fetched_at, ttl_secs)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![key, body, etag, fetched_at, ttl_secs],
        )?;
        Ok(())
    }

    pub fn api_cache_bytes(&self) -> Result<u64> {
        let conn = self.0.lock().unwrap();
        let bytes: i64 = conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(body)), 0) FROM api_cache",
            [],
            |row| row.get(0),
        )?;
        Ok(bytes.max(0) as u64)
    }

    pub fn clear_api_cache(&self) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM api_cache", [])?;
        conn.execute_batch("VACUUM")?;
        Ok(())
    }

    pub fn purge_api_cache_prefix(&self, prefix: &str) -> Result<usize> {
        let conn = self.0.lock().unwrap();
        let removed = conn.execute(
            "DELETE FROM api_cache WHERE key GLOB ?1",
            params![format!("{prefix}*")],
        )?;
        Ok(removed)
    }

    pub fn cache_touch(&self, key: &str, fetched_at: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE api_cache SET fetched_at = ?2 WHERE key = ?1",
            params![key, fetched_at],
        )?;
        Ok(())
    }
}
