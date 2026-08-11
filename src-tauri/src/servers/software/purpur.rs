use futures::future::BoxFuture;
use serde::Deserialize;

use crate::{
    error::Result,
    network::NetworkManager,
    servers::{
        import,
        provision::{self, Provisioned, SERVER_JAR},
    },
};

use super::{Detected, Folder, Install, Runtime, ServerSoftware, Spec};

pub struct Purpur;

#[derive(Deserialize)]
struct Version {
    builds: Builds,
}

#[derive(Deserialize)]
struct Builds {
    latest: String,
    all: Vec<String>,
}

async fn versions(client: &NetworkManager, game_version: &str) -> Result<Version> {
    let url = format!("https://api.purpurmc.org/v2/purpur/{game_version}");
    provision::fetch_json(client, &url).await
}

impl ServerSoftware for Purpur {
    fn spec(&self) -> Spec {
        Spec {
            id: "purpur",
            label: "Purpur",
            hint: "A Paper fork with a long list of gameplay switches.",
            runtime: Runtime::Java,
            builds: true,
            config_file: "server.properties",
            content_dir: Some("plugins"),
        }
    }

    fn versions<'a>(
        &'a self,
        client: &'a NetworkManager,
        game_version: &'a str,
    ) -> BoxFuture<'a, Result<Vec<String>>> {
        Box::pin(async move {
            let mut builds = versions(client, game_version).await?.builds.all;
            builds.reverse();
            Ok(builds)
        })
    }

    fn install<'a>(&'a self, install: Install<'a>) -> BoxFuture<'a, Result<Provisioned>> {
        Box::pin(async move {
            install.task.stage("server-jar");
            let game = install.game_version();
            let build = match install.build() {
                Some(build) => build.to_string(),
                None => versions(install.network(), game).await?.builds.latest,
            };
            provision::download_jar(
                &install,
                format!("https://api.purpurmc.org/v2/purpur/{game}/{build}/download"),
                None,
                None,
            )
            .await?;
            Ok(Provisioned {
                launch_jar: Some(SERVER_JAR.to_string()),
                flavor_version: Some(build),
                ..Default::default()
            })
        })
    }

    fn detect(&self, folder: &Folder) -> Option<Detected> {
        let history = import::version_history(folder.dir)?;
        history.purpur.then(|| Detected {
            certain: true,
            version_id: Some(history.game_version),
            flavor_version: history.build,
            ..Default::default()
        })
    }
}
