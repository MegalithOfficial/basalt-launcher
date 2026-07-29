use rusqlite::{params, OptionalExtension};

use crate::error::Result;

use super::{CachedResponse, Db};

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

    pub fn cache_touch(&self, key: &str, fetched_at: i64) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE api_cache SET fetched_at = ?2 WHERE key = ?1",
            params![key, fetched_at],
        )?;
        Ok(())
    }
}
