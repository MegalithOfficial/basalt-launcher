use std::path::{Path, PathBuf};

use tauri::AppHandle;

use crate::{
    download::DownloadSpec,
    error::{Error, Result},
    files::FileManager,
    modpack::{
        extract_overrides, loader_from_dependencies, prepare_pack, wanted_by, MrFile, MrIndex,
        PreparePackOutcome, PreparedPack, Side, SERVER_OVERRIDES,
    },
    search::Provider,
    state::AppState,
    tasks::TaskHandle,
};

use super::{provision, software, Server, Software};

pub struct PackServer {
    pub software: Software,
    pub game_version: String,
    pub loader_version: Option<String>,
    pub name: String,
}

pub fn plan_for(index: &MrIndex) -> Result<PackServer> {
    let game_version = index
        .dependencies
        .get("minecraft")
        .cloned()
        .ok_or_else(|| Error::other("This pack does not say which Minecraft version it needs."))?;

    let (software, loader_version) = match loader_from_dependencies(&index.dependencies)? {
        Some((loader, version)) => (software::find(&loader)?, Some(version)),
        None => (software::find("vanilla")?, None),
    };

    Ok(PackServer {
        software,
        game_version,
        loader_version,
        name: index.name.clone(),
    })
}

pub fn server_files(index: &MrIndex) -> Vec<&MrFile> {
    index
        .files
        .iter()
        .filter(|file| wanted_by(file, Side::Server))
        .collect()
}

pub fn dropped_client_files(index: &MrIndex) -> Vec<&str> {
    index
        .files
        .iter()
        .filter(|file| !wanted_by(file, Side::Server))
        .map(|file| file.path.as_str())
        .collect()
}

pub fn unpack_overrides(files: &FileManager, archive: &Path, dir: &Path) -> Result<()> {
    extract_overrides(files, archive, dir, SERVER_OVERRIDES)
}

pub async fn install(
    app: &AppHandle,
    state: &AppState,
    provider: Provider,
    project_id: &str,
    version_id: &str,
    manual_sources: &[crate::modpack::ManualDownloadSource],
    task: &TaskHandle,
) -> Result<Server> {
    task.stage("modpack-download");
    let outcome = prepare_pack(
        app,
        state,
        provider,
        project_id,
        version_id,
        manual_sources,
        true,
    )
    .await?;
    let PreparePackOutcome::Ready(prepared) = outcome else {
        return Err(Error::other(
            "Download all requested CurseForge files before continuing.",
        ));
    };
    let PreparedPack {
        target: version,
        archive_path,
        index,
        curseforge_links,
        ..
    } = *prepared;

    let plan = plan_for(&index)?;
    let id = uuid::Uuid::new_v4().to_string();
    let dir = state
        .paths
        .server_dir_checked(&id)
        .ok_or_else(|| Error::other("invalid server id"))?;
    state.files.ensure_dir(&dir)?;
    provision::write_eula(&state.files, &dir)?;

    let mut server = Server {
        id,
        name: plan.name.clone(),
        flavor: plan.software,
        version_id: plan.game_version.clone(),
        created_at: chrono::Utc::now(),
        managed: true,
        dir: dir.display().to_string(),
        available: true,
        flavor_version: plan.loader_version.clone(),
        launch_jar: None,
        launch_argfiles: Vec::new(),
        min_memory_mb: None,
        max_memory_mb: None,
        java_path: None,
        jvm_args: None,
        jvm_args_mode: None,
        stop_timeout_secs: None,
        eula_accepted_at: Some(chrono::Utc::now().timestamp()),
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
    };
    state.db.insert_server(&server)?;

    let provisioned = provision::install(state, &server, task).await?;
    state.db.set_server_launch(
        &server.id,
        provisioned.launch_jar.as_deref(),
        &provisioned.launch_argfiles,
        provisioned.flavor_version.as_deref(),
        chrono::Utc::now().timestamp(),
    )?;
    server.launch_jar = provisioned.launch_jar;
    server.launch_argfiles = provisioned.launch_argfiles;
    server.flavor_version = provisioned.flavor_version.or(server.flavor_version);

    task.stage("modpack-files");
    let (specs, linkable) = file_specs(&index, &dir)?;
    if !specs.is_empty() {
        provision::fetch(task, state, specs).await?;
    }

    task.stage("modpack-overrides");
    let files = state.files.clone();
    let archive = archive_path.clone();
    let destination = dir.clone();
    tokio::task::spawn_blocking(move || unpack_overrides(&files, &archive, &destination))
        .await
        .map_err(|error| Error::other(format!("unpacking the modpack failed: {error}")))??;

    task.stage("modpack-linking");
    let owner = crate::search::resolve::Target::Server(&server);
    for (kind, _, file) in &curseforge_links {
        if let Ok(kind) = crate::search::ContentKind::parse(kind) {
            owner.record(state, kind, file);
        }
    }
    crate::modpack::link_pack_files(state, owner, &linkable).await;

    state
        .db
        .link_server_pack(&server.id, provider.as_str(), project_id, &version.id)?;

    let dropped = dropped_client_files(&index).len();
    tracing::info!(
        server_id = %server.id,
        software = %server.flavor,
        dropped,
        "installed a modpack as a server"
    );
    Ok(server)
}

type Specs = (Vec<DownloadSpec>, Vec<(String, String)>);

fn file_specs(index: &MrIndex, dir: &Path) -> Result<Specs> {
    let mut specs = Vec::new();
    let mut linkable = Vec::new();
    for file in server_files(index) {
        let Some(url) = file.downloads.first() else {
            continue;
        };
        specs.push(DownloadSpec {
            url: url.clone(),
            dest: dir.join(safe_relative(&file.path)?),
            sha1: file.hashes.sha1.clone(),
            sha256: None,
            size: file.file_size,
        });
        if let Some(sha1) = &file.hashes.sha1 {
            linkable.push((file.path.clone(), sha1.clone()));
        }
    }
    Ok((specs, linkable))
}

fn safe_relative(path: &str) -> Result<PathBuf> {
    let mut clean = PathBuf::new();
    for part in path.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(Error::other(format!(
                "This pack has an unsafe path: {path}"
            )));
        }
        clean.push(part);
    }
    if clean.as_os_str().is_empty() {
        return Err(Error::other(format!("This pack has an empty path: {path}")));
    }
    Ok(clean)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modpack::{MrEnv, MrHashes};
    use std::collections::HashMap;

    fn file(path: &str, client: Option<&str>, server: Option<&str>) -> MrFile {
        MrFile {
            path: path.to_string(),
            hashes: MrHashes::default(),
            downloads: Vec::new(),
            file_size: None,
            env: Some(MrEnv {
                client: client.map(str::to_string),
                server: server.map(str::to_string),
            }),
            local_source: None,
            preserve: false,
        }
    }

    fn index(files: Vec<MrFile>, dependencies: &[(&str, &str)]) -> MrIndex {
        MrIndex {
            name: "Test Pack".to_string(),
            dependencies: dependencies
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect::<HashMap<_, _>>(),
            files,
        }
    }

    #[test]
    fn a_path_that_climbs_out_of_the_server_folder_is_refused() {
        assert!(safe_relative("../../etc/passwd").is_err());
        assert!(safe_relative("mods/../../escape.jar").is_err());
        assert_eq!(
            safe_relative("mods/lithium.jar").unwrap(),
            PathBuf::from("mods").join("lithium.jar")
        );
    }

    #[test]
    fn a_client_only_mod_never_reaches_the_server() {
        let pack = index(
            vec![
                file("mods/sodium.jar", Some("required"), Some("unsupported")),
                file("mods/lithium.jar", Some("required"), Some("required")),
                file("mods/carpet.jar", Some("unsupported"), Some("required")),
            ],
            &[],
        );

        let kept = server_files(&pack)
            .into_iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(kept, ["mods/lithium.jar", "mods/carpet.jar"]);
        assert_eq!(dropped_client_files(&pack), ["mods/sodium.jar"]);
    }

    #[test]
    fn a_file_that_says_nothing_is_kept() {
        let mut bare = file("config/thing.toml", None, None);
        bare.env = None;
        let pack = index(
            vec![bare, file("mods/opt.jar", None, Some("optional"))],
            &[],
        );
        assert_eq!(server_files(&pack).len(), 2);
        assert!(dropped_client_files(&pack).is_empty());
    }

    #[test]
    fn the_loader_and_version_come_out_of_the_dependencies() {
        let pack = index(
            Vec::new(),
            &[("minecraft", "1.21.1"), ("neoforge", "21.1.90")],
        );
        let plan = plan_for(&pack).unwrap();
        assert_eq!(plan.software.id(), "neoforge");
        assert_eq!(plan.game_version, "1.21.1");
        assert_eq!(plan.loader_version.as_deref(), Some("21.1.90"));
    }

    #[test]
    fn a_pack_with_no_loader_becomes_a_vanilla_server() {
        let pack = index(Vec::new(), &[("minecraft", "1.21.8")]);
        let plan = plan_for(&pack).unwrap();
        assert_eq!(plan.software.id(), "vanilla");
        assert!(plan.loader_version.is_none());
    }

    #[test]
    fn a_pack_without_a_minecraft_version_is_refused() {
        assert!(plan_for(&index(Vec::new(), &[("fabric-loader", "0.16.9")])).is_err());
    }
}
