use std::path::PathBuf;

use crate::{
    content::{self, ContentItem},
    error::{Error, Result},
    files::FileManager,
};

use super::Server;

pub fn dir_of(server: &Server) -> Result<PathBuf> {
    let sub = server.flavor.content_dir().ok_or_else(|| {
        Error::other(format!(
            "A {} server takes no mods or plugins.",
            server.flavor.label()
        ))
    })?;
    Ok(PathBuf::from(&server.dir).join(sub))
}

pub fn list(files: &FileManager, server: &Server) -> Result<Vec<ContentItem>> {
    content::list_in(files, &dir_of(server)?)
}

pub fn toggle(files: &FileManager, server: &Server, file_name: &str) -> Result<bool> {
    content::toggle_in(files, &dir_of(server)?, file_name)
}

pub fn delete(files: &FileManager, server: &Server, file_name: &str) -> Result<()> {
    content::delete_in(files, &dir_of(server)?, file_name)
}

pub fn add(files: &FileManager, server: &Server, sources: &[String]) -> Result<usize> {
    content::add_into(files, &dir_of(server)?, sources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{paths::Paths, servers::software};

    fn server(flavor: &str, dir: &std::path::Path) -> Server {
        Server {
            id: "s1".to_string(),
            name: "Test".to_string(),
            flavor: software::find(flavor).unwrap(),
            version_id: "1.21.8".to_string(),
            created_at: chrono::Utc::now(),
            managed: true,
            dir: dir.display().to_string(),
            available: true,
            flavor_version: None,
            launch_jar: None,
            launch_argfiles: Vec::new(),
            min_memory_mb: None,
            max_memory_mb: None,
            java_path: None,
            jvm_args: None,
            jvm_args_mode: None,
            stop_timeout_secs: None,
            eula_accepted_at: None,
            installed_at: None,
            last_started_at: None,
            uptime_secs: 0,
            port: None,
            motd: None,
            max_players: None,
            notes: None,
            launch_script: None,
            skip_launch_script: false,
            pack_provider: None,
            pack_project_id: None,
            pack_version_id: None,
            import_source: None,
            import_source_id: None,
        }
    }

    #[test]
    fn each_software_keeps_its_content_where_it_expects_it() {
        let root = PathBuf::from("/srv/smp");
        assert!(dir_of(&server("paper", &root))
            .unwrap()
            .ends_with("plugins"));
        assert!(dir_of(&server("purpur", &root))
            .unwrap()
            .ends_with("plugins"));
        assert!(dir_of(&server("fabric", &root)).unwrap().ends_with("mods"));
        assert!(dir_of(&server("neoforge", &root))
            .unwrap()
            .ends_with("mods"));
        assert!(dir_of(&server("vanilla", &root)).is_err());
        assert!(dir_of(&server("pumpkin", &root)).is_err());
    }

    #[test]
    fn a_disabled_plugin_is_listed_as_disabled_under_its_real_name() {
        let root = std::env::temp_dir().join(format!("basalt-sc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let files = FileManager::new(Paths::plain(root.clone())).unwrap();
        let dir = root.join("servers").join("s1");
        let entry = server("paper", &dir);
        let plugins = dir_of(&entry).unwrap();
        files.ensure_dir(&plugins).unwrap();
        files
            .write_atomic(plugins.join("EssentialsX.jar"), b"jar")
            .unwrap();

        assert!(!toggle(&files, &entry, "EssentialsX.jar").unwrap());
        let items = list(&files, &entry).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].file_name, "EssentialsX.jar");
        assert!(!items[0].enabled);

        delete(&files, &entry, "EssentialsX.jar").unwrap();
        assert!(list(&files, &entry).unwrap().is_empty());
        std::fs::remove_dir_all(root).ok();
    }
}
