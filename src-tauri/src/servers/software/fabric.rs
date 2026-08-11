use futures::future::BoxFuture;
use serde::Deserialize;

use crate::{
    error::{Error, Result},
    loaders::{self, Loader},
    network::NetworkManager,
    servers::provision::{self, Provisioned, SERVER_JAR},
};

use super::{Detected, Folder, Install, Runtime, ServerSoftware, Spec};

pub struct Fabric;

const LAUNCH_JAR: &str = "fabric-server-launch.jar";

#[derive(Deserialize)]
struct Installer {
    version: String,
    stable: bool,
}

impl ServerSoftware for Fabric {
    fn spec(&self) -> Spec {
        Spec {
            id: "fabric",
            label: "Fabric",
            hint: "A light mod loader that updates to new Minecraft versions quickly.",
            runtime: Runtime::Java,
            builds: true,
            config_file: "server.properties",
            content_dir: Some("mods"),
        }
    }

    fn versions<'a>(
        &'a self,
        client: &'a NetworkManager,
        game_version: &'a str,
    ) -> BoxFuture<'a, Result<Vec<String>>> {
        Box::pin(loaders::list_loader_versions(
            client,
            Loader::Fabric,
            game_version,
        ))
    }

    fn install<'a>(&'a self, install: Install<'a>) -> BoxFuture<'a, Result<Provisioned>> {
        Box::pin(async move {
            install.task.stage("server-jar");
            let game = install.game_version();
            let loader = match install.build() {
                Some(version) => version.to_string(),
                None => loaders::list_loader_versions(install.network(), Loader::Fabric, game)
                    .await?
                    .into_iter()
                    .next()
                    .ok_or_else(|| Error::other(format!("Fabric has no loader for {game}.")))?,
            };
            let installers: Vec<Installer> = provision::fetch_json(
                install.network(),
                "https://meta.fabricmc.net/v2/versions/installer",
            )
            .await?;
            let installer = installers
                .iter()
                .find(|entry| entry.stable)
                .or_else(|| installers.first())
                .ok_or_else(|| Error::other("Fabric published no installer."))?;
            provision::download_jar(
                &install,
                format!(
                    "https://meta.fabricmc.net/v2/versions/loader/{game}/{loader}/{}/server/jar",
                    installer.version
                ),
                None,
                None,
            )
            .await?;
            Ok(Provisioned {
                launch_jar: Some(SERVER_JAR.to_string()),
                flavor_version: Some(loader),
                ..Default::default()
            })
        })
    }

    fn detect(&self, folder: &Folder) -> Option<Detected> {
        folder.has(LAUNCH_JAR).then(|| Detected {
            certain: true,
            launch_jar: Some(LAUNCH_JAR.to_string()),
            ..Default::default()
        })
    }
}
