use rusqlite::params;

use super::Db;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct DatapackRecord {
    pub file_name: String,
    pub sha1: Option<String>,
    pub provider: Option<String>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub title: Option<String>,
    pub icon_url: Option<String>,
    pub installed_at: i64,
    pub latest_version_id: Option<String>,
    pub latest_file_name: Option<String>,
}

impl Db {
    pub fn world_datapacks(&self, instance_id: &str, world: &str) -> Result<Vec<DatapackRecord>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT file_name, sha1, provider, project_id, version_id, title, icon_url,
                    installed_at, latest_version_id, latest_file_name
             FROM world_datapacks WHERE instance_id = ?1 AND world = ?2",
        )?;
        let rows = stmt.query_map(params![instance_id, world], |row| {
            Ok(DatapackRecord {
                file_name: row.get(0)?,
                sha1: row.get(1)?,
                provider: row.get(2)?,
                project_id: row.get(3)?,
                version_id: row.get(4)?,
                title: row.get(5)?,
                icon_url: row.get(6)?,
                installed_at: row.get(7)?,
                latest_version_id: row.get(8)?,
                latest_file_name: row.get(9)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn all_world_datapacks(&self, instance_id: &str) -> Result<Vec<(String, DatapackRecord)>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT world, file_name, sha1, provider, project_id, version_id, title, icon_url,
                    installed_at, latest_version_id, latest_file_name
             FROM world_datapacks WHERE instance_id = ?1",
        )?;
        let rows = stmt.query_map(params![instance_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                DatapackRecord {
                    file_name: row.get(1)?,
                    sha1: row.get(2)?,
                    provider: row.get(3)?,
                    project_id: row.get(4)?,
                    version_id: row.get(5)?,
                    title: row.get(6)?,
                    icon_url: row.get(7)?,
                    installed_at: row.get(8)?,
                    latest_version_id: row.get(9)?,
                    latest_file_name: row.get(10)?,
                },
            ))
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn record_world_datapack(
        &self,
        instance_id: &str,
        world: &str,
        record: &DatapackRecord,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO world_datapacks
                (instance_id, world, file_name, sha1, provider, project_id, version_id,
                 title, icon_url, installed_at, latest_version_id, latest_file_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                instance_id,
                world,
                record.file_name,
                record.sha1,
                record.provider,
                record.project_id,
                record.version_id,
                record.title,
                record.icon_url,
                record.installed_at,
                record.latest_version_id,
                record.latest_file_name,
            ],
        )?;
        Ok(())
    }

    pub fn delete_world_datapack(
        &self,
        instance_id: &str,
        world: &str,
        file_name: &str,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "DELETE FROM world_datapacks WHERE instance_id = ?1 AND world = ?2 AND file_name = ?3",
            params![instance_id, world, file_name],
        )?;
        Ok(())
    }

    pub fn set_world_datapack_latest(
        &self,
        instance_id: &str,
        world: &str,
        file_name: &str,
        latest_version_id: Option<&str>,
        latest_file_name: Option<&str>,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE world_datapacks
             SET latest_version_id = ?4, latest_file_name = ?5
             WHERE instance_id = ?1 AND world = ?2 AND file_name = ?3",
            params![
                instance_id,
                world,
                file_name,
                latest_version_id,
                latest_file_name
            ],
        )?;
        Ok(())
    }
}
