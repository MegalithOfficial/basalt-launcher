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

pub struct Neoforge;

const COORDINATES: &str = "net/neoforged/neoforge";

impl ServerSoftware for Neoforge {
    fn spec(&self) -> Spec {
        Spec {
            id: "neoforge",
            label: "NeoForge",
            hint: "The Forge fork most new modpacks moved to.",
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
            Loader::Neoforge,
            game_version,
        ))
    }

    fn install<'a>(&'a self, install: Install<'a>) -> BoxFuture<'a, Result<Provisioned>> {
        Box::pin(async move {
            let version = install
                .build()
                .ok_or_else(|| Error::other("Pick a NeoForge version first."))?
                .to_string();
            let installed = provision::run_installer(
                &install,
                format!(
                    "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
                ),
                format!("neoforge-{version}-installer.jar"),
                format!("libraries/{COORDINATES}/{version}/{PLATFORM_PLACEHOLDER}_args.txt"),
            )
            .await?;

            let launch_argfiles = installed
                .argfiles
                .clone()
                .ok_or_else(|| installed.failed(self.spec().label))?;
            Ok(Provisioned {
                launch_argfiles,
                flavor_version: Some(version),
                ..Default::default()
            })
        })
    }

    fn detect(&self, folder: &Folder) -> Option<Detected> {
        let found = import::modded_launch(folder.dir, COORDINATES, "--fml.neoForgeVersion")?;
        Some(Detected {
            certain: true,
            version_id: found.game_version,
            flavor_version: Some(found.version),
            launch_argfiles: found.argfiles,
            ..Default::default()
        })
    }
}
