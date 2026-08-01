use std::collections::HashSet;

use rusqlite::{params, OptionalExtension};

use crate::error::{Error, Result};

use super::{Db, InstanceGroup, InstanceOrganization, InstancePlacement};

fn clean_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::other("Group name cannot be empty."));
    }
    if name.chars().count() > 64 {
        return Err(Error::other(
            "Group names cannot be longer than 64 characters.",
        ));
    }
    Ok(name)
}

impl Db {
    pub fn instance_organization(&self) -> Result<InstanceOrganization> {
        let conn = self.0.lock().unwrap();
        let groups = {
            let mut statement = conn.prepare(
                "SELECT id, name, sort_order FROM instance_groups ORDER BY sort_order, name",
            )?;
            let rows = statement.query_map([], |row| {
                Ok(InstanceGroup {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    sort_order: row.get(2)?,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let placements = {
            let mut statement = conn.prepare(
                "SELECT id, group_id, group_order FROM instances ORDER BY group_order, created_at",
            )?;
            let rows = statement.query_map([], |row| {
                Ok(InstancePlacement {
                    instance_id: row.get(0)?,
                    group_id: row.get(1)?,
                    sort_order: row.get(2)?,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(InstanceOrganization { groups, placements })
    }

    pub fn create_instance_group(&self, name: &str) -> Result<InstanceGroup> {
        let name = clean_name(name)?;
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.0.lock().unwrap();
        let order: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM instance_groups",
            [],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT INTO instance_groups (id, name, sort_order) VALUES (?1, ?2, ?3)",
            params![id, name, order],
        )?;
        Ok(InstanceGroup {
            id,
            name: name.to_string(),
            sort_order: order,
        })
    }

    pub fn rename_instance_group(&self, id: &str, name: &str) -> Result<InstanceGroup> {
        let name = clean_name(name)?;
        let conn = self.0.lock().unwrap();
        let changed = conn.execute(
            "UPDATE instance_groups SET name = ?2 WHERE id = ?1",
            params![id, name],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("instance group".to_string()));
        }
        let sort_order = conn.query_row(
            "SELECT sort_order FROM instance_groups WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        Ok(InstanceGroup {
            id: id.to_string(),
            name: name.to_string(),
            sort_order,
        })
    }

    pub fn delete_instance_group(&self, id: &str) -> Result<()> {
        let mut conn = self.0.lock().unwrap();
        let transaction = conn.transaction()?;
        transaction.execute(
            "UPDATE instances SET group_id = NULL, group_order = 0 WHERE group_id = ?1",
            [id],
        )?;
        let changed = transaction.execute("DELETE FROM instance_groups WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(Error::NotFound("instance group".to_string()));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn move_instance_to_group(&self, instance_id: &str, group_id: Option<&str>) -> Result<()> {
        let mut conn = self.0.lock().unwrap();
        let transaction = conn.transaction()?;
        if let Some(group_id) = group_id {
            let exists = transaction
                .query_row(
                    "SELECT 1 FROM instance_groups WHERE id = ?1",
                    [group_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !exists {
                return Err(Error::NotFound("instance group".to_string()));
            }
        }
        let order: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(group_order), -1) + 1 FROM instances WHERE group_id IS ?1",
            [group_id],
            |row| row.get(0),
        )?;
        let changed = transaction.execute(
            "UPDATE instances SET group_id = ?2, group_order = ?3 WHERE id = ?1",
            params![instance_id, group_id, order],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("instance".to_string()));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn reorder_instance_groups(&self, ids: &[String]) -> Result<()> {
        if ids.iter().collect::<HashSet<_>>().len() != ids.len() {
            return Err(Error::other("Group order contains duplicates."));
        }
        let mut conn = self.0.lock().unwrap();
        let transaction = conn.transaction()?;
        let count: i64 =
            transaction.query_row("SELECT COUNT(*) FROM instance_groups", [], |row| row.get(0))?;
        if count != ids.len() as i64 {
            return Err(Error::other("Group order is incomplete."));
        }
        for (order, id) in ids.iter().enumerate() {
            let changed = transaction.execute(
                "UPDATE instance_groups SET sort_order = ?2 WHERE id = ?1",
                params![id, order as i64],
            )?;
            if changed == 0 {
                return Err(Error::NotFound("instance group".to_string()));
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn reorder_group_instances(&self, group_id: Option<&str>, ids: &[String]) -> Result<()> {
        if ids.iter().collect::<HashSet<_>>().len() != ids.len() {
            return Err(Error::other("Instance order contains duplicates."));
        }
        let mut conn = self.0.lock().unwrap();
        let transaction = conn.transaction()?;
        let count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM instances WHERE group_id IS ?1",
            [group_id],
            |row| row.get(0),
        )?;
        if count != ids.len() as i64 {
            return Err(Error::other("Instance order is incomplete."));
        }
        for (order, id) in ids.iter().enumerate() {
            let changed = transaction.execute(
                "UPDATE instances SET group_order = ?2 WHERE id = ?1 AND group_id IS ?3",
                params![id, order as i64, group_id],
            )?;
            if changed == 0 {
                return Err(Error::other("An instance is not in that group."));
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn clone_instance_placement(&self, source_id: &str, destination_id: &str) -> Result<()> {
        let conn = self.0.lock().unwrap();
        let group_id: Option<String> = conn.query_row(
            "SELECT group_id FROM instances WHERE id = ?1",
            [source_id],
            |row| row.get(0),
        )?;
        let order: i64 = conn.query_row(
            "SELECT COALESCE(MAX(group_order), -1) + 1 FROM instances WHERE group_id IS ?1",
            [group_id.as_deref()],
            |row| row.get(0),
        )?;
        conn.execute(
            "UPDATE instances SET group_id = ?2, group_order = ?3 WHERE id = ?1",
            params![destination_id, group_id, order],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Instance;

    fn instance(id: &str) -> Instance {
        Instance {
            id: id.to_string(),
            name: id.to_string(),
            version_id: "1.21.1".to_string(),
            created_at: chrono::Utc::now(),
            min_memory_mb: None,
            max_memory_mb: None,
            java_path: None,
            last_played_at: None,
            playtime_secs: 0,
            dir: String::new(),
            logo: None,
            loader: None,
            loader_version: None,
            launch_version_id: None,
            pack_provider: None,
            pack_project_id: None,
            pack_version_id: None,
            jvm_args: None,
            jvm_args_mode: None,
            env_vars: None,
            env_vars_mode: None,
            import_source: None,
            import_source_id: None,
            banner_id: None,
        }
    }

    #[test]
    fn deleting_a_group_keeps_its_instances() {
        let db = Db::open_in_memory().unwrap();
        db.insert_instance(&instance("one")).unwrap();
        let group = db.create_instance_group("Modpacks").unwrap();
        db.move_instance_to_group("one", Some(&group.id)).unwrap();

        db.delete_instance_group(&group.id).unwrap();

        let organization = db.instance_organization().unwrap();
        assert!(organization.groups.is_empty());
        assert_eq!(organization.placements.len(), 1);
        assert_eq!(organization.placements[0].group_id, None);
    }

    #[test]
    fn duplicated_instances_inherit_the_group() {
        let db = Db::open_in_memory().unwrap();
        db.insert_instance(&instance("one")).unwrap();
        db.insert_instance(&instance("two")).unwrap();
        let group = db.create_instance_group("Modpacks").unwrap();
        db.move_instance_to_group("one", Some(&group.id)).unwrap();

        db.clone_instance_placement("one", "two").unwrap();

        let organization = db.instance_organization().unwrap();
        let copy = organization
            .placements
            .iter()
            .find(|placement| placement.instance_id == "two")
            .unwrap();
        assert_eq!(copy.group_id.as_deref(), Some(group.id.as_str()));
    }
}
