use std::path::Path;

use futures::future::BoxFuture;

use crate::{
    download::DownloadSpec,
    error::{Error, Result},
    files::FileManager,
    servers::provision::{self, Provisioned},
};

use super::{Detected, Folder, Install, Runtime, ServerSoftware, Spec};

pub struct Pumpkin;

const RELEASE: &str = "https://github.com/Pumpkin-MC/Pumpkin/releases/download/nightly";
const CONFIG: &str = "pumpkin.toml";
const DEFAULT_PORT: u16 = 25565;

fn asset() -> Result<(&'static str, &'static str)> {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "X64",
        "aarch64" => "ARM64",
        other => return Err(Error::other(format!("Pumpkin has no build for {other}."))),
    };
    Ok(match std::env::consts::OS {
        "linux" => (
            if arch == "X64" {
                "pumpkin-X64-Linux"
            } else {
                "pumpkin-ARM64-Linux"
            },
            "pumpkin",
        ),
        "macos" => (
            if arch == "X64" {
                "pumpkin-X64-macOS"
            } else {
                "pumpkin-ARM64-macOS"
            },
            "pumpkin",
        ),
        "windows" => (
            if arch == "X64" {
                "pumpkin-X64-Windows.exe"
            } else {
                "pumpkin-ARM64-Windows.exe"
            },
            "pumpkin.exe",
        ),
        other => return Err(Error::other(format!("Pumpkin has no build for {other}."))),
    })
}

impl ServerSoftware for Pumpkin {
    fn spec(&self) -> Spec {
        Spec {
            id: "pumpkin",
            label: "Pumpkin",
            hint:
                "A server written in Rust. No Java, configured through pumpkin.toml, still early.",
            runtime: Runtime::Native,
            builds: false,
            config_file: CONFIG,
            content_dir: None,
        }
    }

    fn install<'a>(&'a self, install: Install<'a>) -> BoxFuture<'a, Result<Provisioned>> {
        Box::pin(async move {
            install.task.stage("server-binary");
            let (asset, name) = asset()?;
            let dest = install.dir().join(name);
            provision::fetch(
                install.task,
                install.state,
                vec![DownloadSpec {
                    url: format!("{RELEASE}/{asset}"),
                    dest: dest.clone(),
                    sha1: None,
                    sha256: None,
                    size: None,
                }],
            )
            .await?;
            provision::make_executable(&dest)?;
            Ok(Provisioned {
                launch_jar: Some(name.to_string()),
                flavor_version: Some("nightly".to_string()),
                ..Default::default()
            })
        })
    }

    fn detect(&self, folder: &Folder) -> Option<Detected> {
        let binary = folder.named(|name| name == "pumpkin" || name == "pumpkin.exe")?;
        folder.has(CONFIG).then(|| Detected {
            certain: true,
            flavor_version: Some("nightly".to_string()),
            launch_jar: Some(binary),
            ..Default::default()
        })
    }

    fn launch_args(&self) -> &'static [&'static str] {
        &[]
    }

    fn port(&self, files: &FileManager, dir: &Path) -> Option<u16> {
        let bytes = files.read(dir.join(CONFIG)).ok()?;
        let value: toml::Value = toml::from_str(&String::from_utf8_lossy(&bytes)).ok()?;
        let address = value
            .get("networking")?
            .get("java")?
            .get("address")?
            .as_str()?;
        address
            .rsplit_once(':')?
            .1
            .trim()
            .parse()
            .ok()
            .or(Some(DEFAULT_PORT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_asset_name_and_the_local_name_stay_in_step() {
        let (asset, name) = asset().unwrap();
        assert!(asset.starts_with("pumpkin-"));
        if cfg!(windows) {
            assert_eq!(name, "pumpkin.exe");
            assert!(asset.ends_with(".exe"));
        } else {
            assert_eq!(name, "pumpkin");
            assert!(!asset.ends_with(".exe"));
        }
    }
}
