use futures::future::BoxFuture;

use crate::{
    error::{Error, Result},
    install,
    servers::{
        import, provision,
        provision::{Provisioned, SERVER_JAR},
    },
};

use super::{Detected, Folder, Install, Runtime, ServerSoftware, Spec};

pub struct Vanilla;

impl ServerSoftware for Vanilla {
    fn spec(&self) -> Spec {
        Spec {
            id: "vanilla",
            label: "Vanilla",
            hint: "The server Mojang ships. No mods, no plugins.",
            runtime: Runtime::Java,
            builds: false,
            config_file: "server.properties",
            content_dir: None,
        }
    }

    fn install<'a>(&'a self, install: Install<'a>) -> BoxFuture<'a, Result<Provisioned>> {
        Box::pin(async move {
            install.task.stage("server-jar");
            let version =
                install::load_merged_version(install.state, install.game_version()).await?;
            let spec = version
                .server_spec(install.dir().join(SERVER_JAR))
                .ok_or_else(|| {
                    Error::other(format!(
                        "Mojang does not publish a server for {}.",
                        install.game_version()
                    ))
                })?;
            provision::fetch(install.task, install.state, vec![spec]).await?;
            Ok(Provisioned {
                launch_jar: Some(SERVER_JAR.to_string()),
                ..Default::default()
            })
        })
    }

    fn detect(&self, folder: &Folder) -> Option<Detected> {
        Some(Detected {
            launch_jar: Some(import::fallback_jar(folder.entries)?),
            ..Default::default()
        })
    }
}
