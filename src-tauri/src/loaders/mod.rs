use std::collections::HashMap;

use serde::Deserialize;
use tauri::AppHandle;

use crate::config::Instance;
use crate::download::{self, DownloadSpec};
use crate::error::{Error, Result};
use crate::install;
use crate::java;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Loader {
    Fabric,
    Quilt,
    Neoforge,
    Forge,
}

impl Loader {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "fabric" => Ok(Loader::Fabric),
            "quilt" => Ok(Loader::Quilt),
            "neoforge" => Ok(Loader::Neoforge),
            "forge" => Ok(Loader::Forge),
            other => Err(Error::other(format!("unknown loader {other}"))),
        }
    }
}

#[derive(Deserialize)]
struct FabricLoaderEntry {
    loader: FabricLoaderVersion,
}

#[derive(Deserialize)]
struct FabricLoaderVersion {
    version: String,
}

#[derive(Deserialize)]
struct NeoforgeVersions {
    versions: Vec<String>,
}

#[derive(Deserialize)]
struct ForgePromotions {
    promos: HashMap<String, String>,
}

async fn fetch_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> Result<T> {
    Ok(client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

fn neoforge_prefix(game_version: &str) -> String {
    match game_version.strip_prefix("1.") {
        Some(rest) if rest.contains('.') => format!("{rest}."),
        Some(rest) => format!("{rest}.0."),
        None => format!("{game_version}."),
    }
}

pub async fn list_loader_versions(
    client: &reqwest::Client,
    loader: Loader,
    game_version: &str,
) -> Result<Vec<String>> {
    match loader {
        Loader::Fabric => {
            let url = format!("https://meta.fabricmc.net/v2/versions/loader/{game_version}");
            let entries: Vec<FabricLoaderEntry> = fetch_json(client, &url).await?;
            Ok(entries.into_iter().map(|e| e.loader.version).collect())
        }
        Loader::Quilt => {
            let url = format!("https://meta.quiltmc.org/v3/versions/loader/{game_version}");
            let entries: Vec<FabricLoaderEntry> = fetch_json(client, &url).await?;
            Ok(entries.into_iter().map(|e| e.loader.version).collect())
        }
        Loader::Neoforge => {
            let data: NeoforgeVersions = fetch_json(
                client,
                "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge",
            )
            .await?;
            let prefix = neoforge_prefix(game_version);
            let mut versions: Vec<String> = data
                .versions
                .into_iter()
                .filter(|v| v.starts_with(&prefix))
                .collect();
            versions.reverse();
            Ok(versions)
        }
        Loader::Forge => {
            let data: ForgePromotions = fetch_json(
                client,
                "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
            )
            .await?;
            let mut versions = Vec::new();
            if let Some(v) = data.promos.get(&format!("{game_version}-recommended")) {
                versions.push(v.clone());
            }
            if let Some(v) = data.promos.get(&format!("{game_version}-latest")) {
                if !versions.contains(v) {
                    versions.push(v.clone());
                }
            }
            Ok(versions)
        }
    }
}

async fn install_profile_json(
    state: &AppState,
    url: &str,
) -> Result<String> {
    let profile: serde_json::Value = fetch_json(&state.http, url).await?;
    let id = profile
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::other("loader profile has no id"))?
        .to_string();
    let dir = state.paths.version_dir(&id);
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(
        state.paths.version_json(&id),
        serde_json::to_vec_pretty(&profile)?,
    )
    .await?;
    Ok(id)
}

async fn run_installer(
    app: &AppHandle,
    state: &AppState,
    instance: &Instance,
    installer_url: &str,
    installer_name: &str,
    expected_id: &str,
) -> Result<String> {
    install::install_version(app, state, &instance.id, &instance.version_id).await?;

    install::emit_stage(app, &instance.id, "loader-installer");

    let installer_dir = state.paths.root.join("cache").join("installers");
    let installer_path = installer_dir.join(installer_name);
    download::download_one(
        &state.http,
        &DownloadSpec {
            url: installer_url.to_string(),
            dest: installer_path.clone(),
            sha1: None,
            size: None,
        },
    )
    .await?;

    let profiles_stub = state.paths.root.join("launcher_profiles.json");
    if !profiles_stub.exists() {
        tokio::fs::write(&profiles_stub, b"{\"profiles\":{}}").await?;
    }

    let vanilla = install::load_merged_version(state, &instance.version_id).await?;
    let java = java::find_for_major(vanilla.required_java_major(), None)
        .await
        .ok_or_else(|| Error::other("No Java found to run the loader installer."))?;

    let output = tokio::process::Command::new(&java.path)
        .arg("-jar")
        .arg(&installer_path)
        .arg("--installClient")
        .arg(&state.paths.root)
        .current_dir(&installer_dir)
        .output()
        .await?;

    if !state.paths.version_json(expected_id).is_file() {
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&output.stderr));
        let tail: String = text
            .lines()
            .rev()
            .take(12)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(Error::other(format!(
            "Loader installer failed for {expected_id}:\n{tail}"
        )));
    }

    Ok(expected_id.to_string())
}

pub async fn install_loader(
    app: &AppHandle,
    state: &AppState,
    instance: &Instance,
) -> Result<String> {
    let loader = Loader::parse(
        instance
            .loader
            .as_deref()
            .ok_or_else(|| Error::other("instance has no loader"))?,
    )?;
    let loader_version = instance
        .loader_version
        .clone()
        .ok_or_else(|| Error::other("instance has no loader version"))?;
    let game = &instance.version_id;

    install::emit_stage(app, &instance.id, "loader-profile");

    match loader {
        Loader::Fabric => {
            let url = format!(
                "https://meta.fabricmc.net/v2/versions/loader/{game}/{loader_version}/profile/json"
            );
            install_profile_json(state, &url).await
        }
        Loader::Quilt => {
            let url = format!(
                "https://meta.quiltmc.org/v3/versions/loader/{game}/{loader_version}/profile/json"
            );
            install_profile_json(state, &url).await
        }
        Loader::Neoforge => {
            let url = format!(
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/{loader_version}/neoforge-{loader_version}-installer.jar"
            );
            let name = format!("neoforge-{loader_version}-installer.jar");
            let expected = format!("neoforge-{loader_version}");
            run_installer(app, state, instance, &url, &name, &expected).await
        }
        Loader::Forge => {
            let full = format!("{game}-{loader_version}");
            let url = format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{full}/forge-{full}-installer.jar"
            );
            let name = format!("forge-{full}-installer.jar");
            let expected = format!("{game}-forge-{loader_version}");
            run_installer(app, state, instance, &url, &name, &expected).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::neoforge_prefix;

    #[test]
    fn neoforge_prefixes() {
        assert_eq!(neoforge_prefix("1.21.1"), "21.1.");
        assert_eq!(neoforge_prefix("1.21"), "21.0.");
        assert_eq!(neoforge_prefix("26.1.2"), "26.1.2.");
    }
}
