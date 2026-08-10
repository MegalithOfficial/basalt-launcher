use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    download::{self, DownloadSpec},
    error::{Error, Result},
    files::FileManager,
    install, java,
    loaders::{self, Loader},
    network::NetworkManager,
    state::AppState,
    tasks::TaskHandle,
};

use super::{Server, ServerFlavor};

pub const PLATFORM_PLACEHOLDER: &str = "{platform}";
const EULA: &[u8] =
    b"#By changing the setting below to TRUE you are indicating your agreement to the Minecraft EULA (https://aka.ms/MinecraftEULA).\neula=true\n";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Provisioned {
    pub launch_jar: Option<String>,
    pub launch_argfiles: Vec<String>,
    pub flavor_version: Option<String>,
}

#[derive(Deserialize)]
struct PaperBuild {
    id: u32,
    channel: String,
    downloads: HashMap<String, PaperDownload>,
}

#[derive(Deserialize)]
struct PaperDownload {
    size: u64,
    url: String,
    checksums: PaperChecksums,
}

#[derive(Deserialize)]
struct PaperChecksums {
    sha256: String,
}

#[derive(Deserialize)]
struct PurpurVersion {
    builds: PurpurBuilds,
}

#[derive(Deserialize)]
struct PurpurBuilds {
    latest: String,
    all: Vec<String>,
}

#[derive(Deserialize)]
struct FabricInstaller {
    version: String,
    stable: bool,
}

async fn fetch_json<T: serde::de::DeserializeOwned>(
    client: &NetworkManager,
    url: &str,
) -> Result<T> {
    client
        .send(client.get(url))
        .await?
        .error_for_status()?
        .json()
        .await
}

pub fn write_eula(files: &FileManager, dir: &Path) -> Result<()> {
    files.write_atomic(dir.join("eula.txt"), EULA)
}

pub fn eula_accepted(files: &FileManager, dir: &Path) -> bool {
    let Ok(bytes) = files.read(dir.join("eula.txt")) else {
        return false;
    };
    super::properties::Properties::parse(&bytes)
        .get("eula")
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
}

pub fn argfile_for_platform(argfile: &str) -> String {
    let platform = if cfg!(windows) { "win" } else { "unix" };
    argfile.replace(PLATFORM_PLACEHOLDER, platform)
}

#[tracing::instrument(skip(client), fields(flavor = ?flavor), err)]
pub async fn list_flavor_versions(
    client: &NetworkManager,
    flavor: ServerFlavor,
    game_version: &str,
) -> Result<Vec<String>> {
    match flavor {
        ServerFlavor::Vanilla => Ok(Vec::new()),
        ServerFlavor::Paper => {
            let builds = paper_builds(client, game_version).await?;
            Ok(builds
                .into_iter()
                .map(|build| build.id.to_string())
                .collect())
        }
        ServerFlavor::Purpur => {
            let url = format!("https://api.purpurmc.org/v2/purpur/{game_version}");
            let version: PurpurVersion = fetch_json(client, &url).await?;
            let mut builds = version.builds.all;
            builds.reverse();
            Ok(builds)
        }
        ServerFlavor::Fabric => {
            loaders::list_loader_versions(client, Loader::Fabric, game_version).await
        }
        ServerFlavor::Neoforge => {
            loaders::list_loader_versions(client, Loader::Neoforge, game_version).await
        }
        ServerFlavor::Forge => {
            loaders::list_loader_versions(client, Loader::Forge, game_version).await
        }
    }
}

#[tracing::instrument(
    skip_all,
    fields(server_id = %server.id, flavor = ?server.flavor, version = %server.version_id),
    err
)]
pub async fn install(state: &AppState, server: &Server, task: &TaskHandle) -> Result<Provisioned> {
    let dir = PathBuf::from(&server.dir);
    state.files.ensure_dir_async(&dir).await?;
    match server.flavor {
        ServerFlavor::Vanilla => install_vanilla(state, server, task).await,
        ServerFlavor::Paper => install_paper(state, server, task).await,
        ServerFlavor::Purpur => install_purpur(state, server, task).await,
        ServerFlavor::Fabric => install_fabric(state, server, task).await,
        ServerFlavor::Neoforge | ServerFlavor::Forge => {
            install_with_installer(state, server, task).await
        }
    }
}

async fn install_vanilla(
    state: &AppState,
    server: &Server,
    task: &TaskHandle,
) -> Result<Provisioned> {
    task.stage("server-jar");
    let version = install::load_merged_version(state, &server.version_id).await?;
    let spec = version
        .server_spec(PathBuf::from(&server.dir).join("server.jar"))
        .ok_or_else(|| {
            Error::other(format!(
                "Mojang does not publish a server for {}.",
                server.version_id
            ))
        })?;
    download::download_one(&state.network, &state.files, &spec).await?;
    Ok(Provisioned {
        launch_jar: Some("server.jar".to_string()),
        ..Default::default()
    })
}

async fn paper_builds(client: &NetworkManager, game_version: &str) -> Result<Vec<PaperBuild>> {
    let url = format!("https://fill.papermc.io/v3/projects/paper/versions/{game_version}/builds");
    fetch_json(client, &url).await
}

async fn install_paper(
    state: &AppState,
    server: &Server,
    task: &TaskHandle,
) -> Result<Provisioned> {
    task.stage("server-jar");
    let builds = paper_builds(&state.network, &server.version_id).await?;
    let build = match server.flavor_version.as_deref() {
        Some(wanted) => builds
            .into_iter()
            .find(|build| build.id.to_string() == wanted)
            .ok_or_else(|| Error::other(format!("Paper build {wanted} is gone.")))?,
        None => {
            let newest = builds
                .iter()
                .position(|build| build.channel == "STABLE")
                .unwrap_or(0);
            builds.into_iter().nth(newest).ok_or_else(|| {
                Error::other(format!("Paper has no build for {}.", server.version_id))
            })?
        }
    };
    let download = build
        .downloads
        .get("server:default")
        .ok_or_else(|| Error::other("This Paper build publishes no server jar."))?;
    download::download_one(
        &state.network,
        &state.files,
        &DownloadSpec {
            url: download.url.clone(),
            dest: PathBuf::from(&server.dir).join("server.jar"),
            sha1: None,
            sha256: Some(download.checksums.sha256.clone()),
            size: Some(download.size),
        },
    )
    .await?;
    Ok(Provisioned {
        launch_jar: Some("server.jar".to_string()),
        flavor_version: Some(build.id.to_string()),
        ..Default::default()
    })
}

async fn install_purpur(
    state: &AppState,
    server: &Server,
    task: &TaskHandle,
) -> Result<Provisioned> {
    task.stage("server-jar");
    let game = &server.version_id;
    let build = match server.flavor_version.clone() {
        Some(build) => build,
        None => {
            let url = format!("https://api.purpurmc.org/v2/purpur/{game}");
            let version: PurpurVersion = fetch_json(&state.network, &url).await?;
            version.builds.latest
        }
    };
    download::download_one(
        &state.network,
        &state.files,
        &DownloadSpec {
            url: format!("https://api.purpurmc.org/v2/purpur/{game}/{build}/download"),
            dest: PathBuf::from(&server.dir).join("server.jar"),
            sha1: None,
            sha256: None,
            size: None,
        },
    )
    .await?;
    Ok(Provisioned {
        launch_jar: Some("server.jar".to_string()),
        flavor_version: Some(build),
        ..Default::default()
    })
}

async fn install_fabric(
    state: &AppState,
    server: &Server,
    task: &TaskHandle,
) -> Result<Provisioned> {
    task.stage("server-jar");
    let game = &server.version_id;
    let loader = match server.flavor_version.clone() {
        Some(version) => version,
        None => loaders::list_loader_versions(&state.network, Loader::Fabric, game)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| Error::other(format!("Fabric has no loader for {game}.")))?,
    };
    let installers: Vec<FabricInstaller> = fetch_json(
        &state.network,
        "https://meta.fabricmc.net/v2/versions/installer",
    )
    .await?;
    let installer = installers
        .iter()
        .find(|entry| entry.stable)
        .or_else(|| installers.first())
        .ok_or_else(|| Error::other("Fabric published no installer."))?;
    download::download_one(
        &state.network,
        &state.files,
        &DownloadSpec {
            url: format!(
                "https://meta.fabricmc.net/v2/versions/loader/{game}/{loader}/{}/server/jar",
                installer.version
            ),
            dest: PathBuf::from(&server.dir).join("server.jar"),
            sha1: None,
            sha256: None,
            size: None,
        },
    )
    .await?;
    Ok(Provisioned {
        launch_jar: Some("server.jar".to_string()),
        flavor_version: Some(loader),
        ..Default::default()
    })
}

async fn install_with_installer(
    state: &AppState,
    server: &Server,
    task: &TaskHandle,
) -> Result<Provisioned> {
    let game = &server.version_id;
    let version = server
        .flavor_version
        .clone()
        .ok_or_else(|| Error::other("Pick a loader version first."))?;
    let (url, name, argfiles) = match server.flavor {
        ServerFlavor::Neoforge => (
            format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"),
            format!("neoforge-{version}-installer.jar"),
            format!("libraries/net/neoforged/neoforge/{version}/{PLATFORM_PLACEHOLDER}_args.txt"),
        ),
        _ => (
            format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{game}-{version}/forge-{game}-{version}-installer.jar"),
            format!("forge-{game}-{version}-installer.jar"),
            format!("libraries/net/minecraftforge/forge/{game}-{version}/{PLATFORM_PLACEHOLDER}_args.txt"),
        ),
    };

    task.stage("server-installer");
    let installer_path = state.paths.cache().join("installers").join(&name);
    download::download_one(
        &state.network,
        &state.files,
        &DownloadSpec {
            url,
            dest: installer_path.clone(),
            sha1: None,
            sha256: None,
            size: None,
        },
    )
    .await?;

    let vanilla = install::load_merged_version(state, game).await?;
    let java = java::find_for_major(&state.files, vanilla.required_java_major(), None)
        .await
        .ok_or_else(|| Error::other("No Java found to run the server installer."))?;

    let dir = PathBuf::from(&server.dir);
    tracing::info!(java = %java.path, installer = %installer_path.display(), "running server installer");
    let output = tokio::process::Command::new(&java.path)
        .arg("-jar")
        .arg(&installer_path)
        .arg("--installServer")
        .arg(&dir)
        .current_dir(&dir)
        .output()
        .await?;

    if state
        .files
        .is_file(dir.join(argfile_for_platform(&argfiles)))?
    {
        let mut launch_argfiles = Vec::new();
        if state.files.is_file(dir.join("user_jvm_args.txt"))? {
            launch_argfiles.push("user_jvm_args.txt".to_string());
        }
        launch_argfiles.push(argfiles);
        return Ok(Provisioned {
            launch_argfiles,
            flavor_version: Some(version),
            ..Default::default()
        });
    }

    let legacy = format!("forge-{game}-{version}.jar");
    if state.files.is_file(dir.join(&legacy))? {
        return Ok(Provisioned {
            launch_jar: Some(legacy),
            flavor_version: Some(version),
            ..Default::default()
        });
    }

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let tail = text
        .lines()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    tracing::error!(server_id = %server.id, "server installer left nothing to launch:\n{tail}");
    Err(Error::other(format!(
        "The {} installer did not finish:\n{tail}",
        server.flavor.as_str()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::Paths;

    #[test]
    fn the_platform_decides_which_argument_file_is_used() {
        let resolved = argfile_for_platform(&format!(
            "libraries/net/neoforged/neoforge/21.1.9/{PLATFORM_PLACEHOLDER}_args.txt"
        ));
        if cfg!(windows) {
            assert!(resolved.ends_with("win_args.txt"));
        } else {
            assert!(resolved.ends_with("unix_args.txt"));
        }
        assert!(!resolved.contains(PLATFORM_PLACEHOLDER));
    }

    #[test]
    fn the_eula_is_only_accepted_once_the_file_says_so() {
        let root = std::env::temp_dir().join(format!("basalt-eula-{}", uuid::Uuid::new_v4()));
        let files = FileManager::new(Paths::plain(root.clone())).unwrap();
        let dir = root.join("servers").join("s1");
        files.ensure_dir(&dir).unwrap();

        assert!(!eula_accepted(&files, &dir));
        files
            .write_atomic(dir.join("eula.txt"), b"eula=false\n")
            .unwrap();
        assert!(!eula_accepted(&files, &dir));

        write_eula(&files, &dir).unwrap();
        assert!(eula_accepted(&files, &dir));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn paper_builds_are_read_from_the_shape_the_api_returns() {
        let builds: Vec<PaperBuild> = serde_json::from_str(
            r#"[
                {"id": 60, "channel": "EXPERIMENTAL", "downloads": {}},
                {"id": 59, "channel": "STABLE", "downloads": {
                    "server:default": {
                        "name": "paper-1.21.8-59.jar",
                        "checksums": {"sha256": "abc"},
                        "size": 52810792,
                        "url": "https://fill-data.papermc.io/paper.jar"
                    }
                }}
            ]"#,
        )
        .unwrap();

        let stable = builds
            .iter()
            .find(|build| build.channel == "STABLE")
            .unwrap();
        let download = stable.downloads.get("server:default").unwrap();
        assert_eq!(stable.id, 59);
        assert_eq!(download.checksums.sha256, "abc");
        assert_eq!(download.size, 52810792);
    }
}
