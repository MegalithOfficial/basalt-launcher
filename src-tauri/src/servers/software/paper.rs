use std::collections::HashMap;

use futures::future::BoxFuture;
use serde::Deserialize;

use crate::{
    error::{Error, Result},
    network::NetworkManager,
    servers::{
        import,
        provision::{self, Provisioned, SERVER_JAR},
    },
};

use super::{Detected, Folder, Install, Runtime, ServerSoftware, Spec};

pub struct Paper;

#[derive(Deserialize)]
pub struct Build {
    pub id: u32,
    pub channel: String,
    pub downloads: HashMap<String, Download>,
}

#[derive(Deserialize)]
pub struct Download {
    pub size: u64,
    pub url: String,
    pub checksums: Checksums,
}

#[derive(Deserialize)]
pub struct Checksums {
    pub sha256: String,
}

async fn builds(client: &NetworkManager, game_version: &str) -> Result<Vec<Build>> {
    let url = format!("https://fill.papermc.io/v3/projects/paper/versions/{game_version}/builds");
    provision::fetch_json(client, &url).await
}

impl ServerSoftware for Paper {
    fn spec(&self) -> Spec {
        Spec {
            id: "paper",
            label: "Paper",
            hint: "A faster Spigot fork that runs Bukkit plugins.",
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
            Ok(builds(client, game_version)
                .await?
                .into_iter()
                .map(|build| build.id.to_string())
                .collect())
        })
    }

    fn install<'a>(&'a self, install: Install<'a>) -> BoxFuture<'a, Result<Provisioned>> {
        Box::pin(async move {
            install.task.stage("server-jar");
            let published = builds(install.network(), install.game_version()).await?;
            let build = match install.build() {
                Some(wanted) => published
                    .into_iter()
                    .find(|build| build.id.to_string() == wanted)
                    .ok_or_else(|| Error::other(format!("Paper build {wanted} is gone.")))?,
                None => {
                    let newest = published
                        .iter()
                        .position(|build| build.channel == "STABLE")
                        .unwrap_or(0);
                    published.into_iter().nth(newest).ok_or_else(|| {
                        Error::other(format!(
                            "Paper has no build for {}.",
                            install.game_version()
                        ))
                    })?
                }
            };
            let download = build
                .downloads
                .get("server:default")
                .ok_or_else(|| Error::other("This Paper build publishes no server jar."))?;
            provision::download_jar(
                &install,
                download.url.clone(),
                Some(download.checksums.sha256.clone()),
                Some(download.size),
            )
            .await?;
            Ok(Provisioned {
                launch_jar: Some(SERVER_JAR.to_string()),
                flavor_version: Some(build.id.to_string()),
                ..Default::default()
            })
        })
    }

    fn detect(&self, folder: &Folder) -> Option<Detected> {
        let history = import::version_history(folder.dir)?;
        if history.purpur {
            return None;
        }
        Some(Detected {
            certain: true,
            version_id: Some(history.game_version),
            flavor_version: history.build,
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_builds_are_read_from_the_shape_the_api_returns() {
        let published: Vec<Build> = serde_json::from_str(
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

        let stable = published
            .iter()
            .find(|build| build.channel == "STABLE")
            .unwrap();
        let download = stable.downloads.get("server:default").unwrap();
        assert_eq!(stable.id, 59);
        assert_eq!(download.checksums.sha256, "abc");
        assert_eq!(download.size, 52810792);
    }
}
