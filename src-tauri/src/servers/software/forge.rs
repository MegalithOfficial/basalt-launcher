use futures::future::BoxFuture;

use crate::{
    error::{Error, Result},
    loaders::{self, Loader},
    network::NetworkManager,
    servers::{
        import,
        provision::{self, Provisioned, PLATFORM_PLACEHOLDER},
    },
};

use super::{Detected, Folder, Install, Runtime, ServerSoftware, Spec};

pub struct Forge;

const COORDINATES: &str = "net/minecraftforge/forge";

impl ServerSoftware for Forge {
    fn spec(&self) -> Spec {
        Spec {
            id: "forge",
            label: "Forge",
            hint: "The oldest mod loader, and the one most large modpacks still use.",
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
            Loader::Forge,
            game_version,
        ))
    }

    fn install<'a>(&'a self, install: Install<'a>) -> BoxFuture<'a, Result<Provisioned>> {
        Box::pin(async move {
            let game = install.game_version();
            let version = install
                .build()
                .ok_or_else(|| Error::other("Pick a Forge version first."))?
                .to_string();
            let installed = provision::run_installer(
                &install,
                format!(
                    "https://maven.minecraftforge.net/net/minecraftforge/forge/{game}-{version}/forge-{game}-{version}-installer.jar"
                ),
                format!("forge-{game}-{version}-installer.jar"),
                format!("libraries/{COORDINATES}/{game}-{version}/{PLATFORM_PLACEHOLDER}_args.txt"),
            )
            .await?;

            if let Some(launch_argfiles) = installed.argfiles {
                return Ok(Provisioned {
                    launch_argfiles,
                    flavor_version: Some(version),
                    ..Default::default()
                });
            }

            let legacy = format!("forge-{game}-{version}.jar");
            if install.files().is_file(install.dir().join(&legacy))? {
                return Ok(Provisioned {
                    launch_jar: Some(legacy),
                    flavor_version: Some(version),
                    ..Default::default()
                });
            }

            Err(installed.failed(self.spec().label))
        })
    }

    fn detect(&self, folder: &Folder) -> Option<Detected> {
        let found = import::modded_launch(folder.dir, COORDINATES, "--fml.forgeVersion")?;
        let (game, loader) = match found.game_version {
            Some(game) => (Some(game), found.version),
            None => found
                .version
                .split_once('-')
                .map(|(game, loader)| (Some(game.to_string()), loader.to_string()))
                .unwrap_or((None, found.version)),
        };
        Some(Detected {
            certain: true,
            version_id: game,
            flavor_version: Some(loader),
            launch_argfiles: found.argfiles,
            ..Default::default()
        })
    }
}
