use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::{
    config::Instance,
    error::Result,
    files::FileManager,
    meta::version::{AssetIndex, VersionJson},
    paths::Paths,
};

#[derive(Debug, Default)]
pub struct LiveSet {
    pub versions: HashSet<String>,
    pub natives: HashSet<String>,
    pub asset_objects: HashSet<PathBuf>,
    pub asset_indexes: HashSet<PathBuf>,
    pub spare_profiles: Vec<String>,
}

#[derive(Debug)]
pub enum Graph {
    Resolved(LiveSet),
    Unavailable(String),
}

fn read_json<T: serde::de::DeserializeOwned>(files: &FileManager, path: &PathBuf) -> Option<T> {
    let bytes = files.read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn installed_versions(files: &FileManager, paths: &Paths) -> Vec<String> {
    let Ok(entries) = files.read_dir(paths.versions()) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .filter(|path| {
            files
                .symlink_metadata(path)
                .map(|meta| meta.is_dir())
                .unwrap_or(false)
        })
        .filter_map(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect()
}

fn profile_matches(instance: &Instance, id: &str) -> bool {
    let (Some(loader), Some(version)) = (
        instance.loader.as_deref(),
        instance.loader_version.as_deref(),
    ) else {
        return false;
    };
    match loader {
        "neoforge" => id == format!("neoforge-{version}"),
        "forge" => id == format!("{}-forge-{version}", instance.version_id),
        _ => id.contains(version),
    }
}

/// Works out everything on disk that an instance can still reach.
///
/// Every rule here may only widen the live set. Keeping a file nothing needs costs disk;
/// dropping one an instance needs stops it launching, so anything ambiguous is kept. If a
/// version an instance actually uses cannot be read, the whole answer is withheld rather
/// than reported half-resolved.
pub fn resolve(files: &FileManager, paths: &Paths, instances: &[Instance]) -> Result<Graph> {
    let on_disk = installed_versions(files, paths);
    let mut parsed: HashMap<String, VersionJson> = HashMap::new();
    for id in &on_disk {
        if let Some(json) = read_json::<VersionJson>(files, &paths.version_json(id)) {
            parsed.insert(id.clone(), json);
        }
    }

    let game_versions: HashSet<&str> = instances
        .iter()
        .map(|instance| instance.version_id.as_str())
        .collect();

    let mut live: HashSet<String> = HashSet::new();
    let mut roots: Vec<String> = Vec::new();
    for instance in instances {
        roots.push(instance.version_id.clone());
        if let Some(launch) = instance.launch_version_id.as_deref() {
            roots.push(launch.to_string());
        }
    }

    let mut by_rule_four: HashSet<String> = HashSet::new();
    for (id, json) in &parsed {
        let Some(parent) = json.inherits_from.as_deref() else {
            continue;
        };
        if game_versions.contains(parent) {
            by_rule_four.insert(id.clone());
            roots.push(id.clone());
        }
    }

    while let Some(id) = roots.pop() {
        if !live.insert(id.clone()) {
            continue;
        }
        if let Some(json) = parsed.get(&id) {
            if let Some(parent) = json.inherits_from.clone() {
                roots.push(parent);
            }
            let jar = json.client_jar_id().to_string();
            if jar != id {
                roots.push(jar);
            }
        }
    }

    let mut set = LiveSet {
        versions: live.clone(),
        ..Default::default()
    };

    for id in &live {
        let Some(json) = parsed.get(id) else {
            if on_disk.contains(id) {
                return Ok(Graph::Unavailable(format!(
                    "the version file for {id} could not be read"
                )));
            }
            continue;
        };

        set.natives.insert(json.id.clone());

        // A profile that inherits its assets carries no index of its own, and asking for one
        // by name would land on the "legacy" fallback that was never downloaded.
        if json.asset_index.is_none() && json.assets.is_none() {
            continue;
        }
        let name = json.assets_name();
        let index_path = paths.assets_indexes().join(format!("{name}.json"));
        if !files.exists(&index_path).unwrap_or(false) {
            return Ok(Graph::Unavailable(format!(
                "the asset index {name} is missing, so Basalt cannot tell which assets are still needed"
            )));
        }
        set.asset_indexes.insert(index_path.clone());
        let Some(index) = read_json::<AssetIndex>(files, &index_path) else {
            return Ok(Graph::Unavailable(format!(
                "the asset index {name} could not be read"
            )));
        };
        for spec in index.specs(paths) {
            set.asset_objects.insert(spec.dest);
        }
    }

    set.spare_profiles = by_rule_four
        .into_iter()
        .filter(|id| {
            !instances
                .iter()
                .any(|instance| profile_matches(instance, id))
        })
        .filter(|id| {
            !instances
                .iter()
                .any(|instance| instance.launch_version_id.as_deref() == Some(id.as_str()))
        })
        .collect();
    set.spare_profiles.sort();

    Ok(Graph::Resolved(set))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_resolved(graph: Graph) -> LiveSet {
        match graph {
            Graph::Resolved(live) => live,
            Graph::Unavailable(reason) => panic!("expected a resolved graph, got: {reason}"),
        }
    }

    fn test_paths() -> (Paths, FileManager) {
        let root =
            std::env::temp_dir().join(format!("basalt-storage-test-{}", uuid::Uuid::new_v4()));
        let paths = Paths { root };
        let files = FileManager::new(paths.clone()).unwrap();
        files.ensure_base_dirs().unwrap();
        (paths, files)
    }

    fn test_instance(
        name: &str,
        version: &str,
        loader: Option<&str>,
        loader_version: Option<&str>,
    ) -> Instance {
        Instance {
            id: name.to_string(),
            name: name.to_string(),
            version_id: version.to_string(),
            created_at: chrono::Utc::now(),
            min_memory_mb: None,
            max_memory_mb: None,
            java_path: None,
            last_played_at: None,
            playtime_secs: 0,
            dir: String::new(),
            logo: None,
            loader: loader.map(str::to_string),
            loader_version: loader_version.map(str::to_string),
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
            notes: None,
        }
    }

    fn write_version(paths: &Paths, id: &str, inherits: Option<&str>) {
        std::fs::create_dir_all(paths.version_dir(id)).unwrap();
        let body = match inherits {
            Some(parent) => format!(
                r#"{{"id":"{id}","mainClass":"net.Main","inheritsFrom":"{parent}","libraries":[]}}"#
            ),
            None => format!(r#"{{"id":"{id}","mainClass":"net.Main","libraries":[]}}"#),
        };
        std::fs::write(paths.version_json(id), body).unwrap();
    }

    #[test]
    fn a_loader_profile_survives_an_instance_whose_launch_version_was_cleared() {
        let (paths, files) = test_paths();
        write_version(&paths, "1.21.11", None);
        write_version(&paths, "fabric-loader-0.18.4-1.21.11", Some("1.21.11"));

        let instances = vec![test_instance(
            "a",
            "1.21.11",
            Some("fabric"),
            Some("0.18.4"),
        )];
        let live = expect_resolved(resolve(&files, &paths, &instances).unwrap());

        assert!(live.versions.contains("fabric-loader-0.18.4-1.21.11"));
        assert!(live.spare_profiles.is_empty());
    }

    #[test]
    fn a_fabric_profile_with_an_unexpected_name_is_still_kept() {
        let (paths, files) = test_paths();
        write_version(&paths, "1.21.11", None);
        write_version(&paths, "some-future-fabric-naming", Some("1.21.11"));

        let instances = vec![test_instance(
            "a",
            "1.21.11",
            Some("fabric"),
            Some("0.18.4"),
        )];
        let live = expect_resolved(resolve(&files, &paths, &instances).unwrap());

        assert!(live.versions.contains("some-future-fabric-naming"));
    }

    #[test]
    fn only_versions_no_instance_can_reach_are_left_out() {
        let (paths, files) = test_paths();
        let all = [
            ("1.21.1", None),
            ("1.21.11", None),
            ("26.1.2", None),
            ("fabric-loader-0.18.4-1.21.11", Some("1.21.11")),
            ("fabric-loader-0.19.3-26.1.2", Some("26.1.2")),
            ("neoforge-21.1.172", Some("1.21.1")),
            ("neoforge-21.1.221", Some("1.21.1")),
        ];
        for (id, parent) in all {
            write_version(&paths, id, parent);
        }

        let instances = vec![
            test_instance("a", "1.21.1", Some("neoforge"), Some("21.1.172")),
            test_instance("b", "1.21.11", Some("fabric"), Some("0.18.4")),
        ];
        let live = expect_resolved(resolve(&files, &paths, &instances).unwrap());

        let mut orphans: Vec<&str> = all
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| !live.versions.contains(*id))
            .collect();
        orphans.sort();

        assert_eq!(orphans, vec!["26.1.2", "fabric-loader-0.19.3-26.1.2"]);
        assert_eq!(live.spare_profiles, vec!["neoforge-21.1.221".to_string()]);
    }

    #[test]
    fn an_unreadable_version_an_instance_uses_withholds_the_whole_answer() {
        let (paths, files) = test_paths();
        write_version(&paths, "1.21.11", None);
        std::fs::write(paths.version_json("1.21.11"), b"{ truncated").unwrap();

        let instances = vec![test_instance("a", "1.21.11", None, None)];
        match resolve(&files, &paths, &instances).unwrap() {
            Graph::Unavailable(reason) => assert!(reason.contains("1.21.11")),
            Graph::Resolved(_) => panic!("a version that cannot be read must not be resolved"),
        }
    }
}
