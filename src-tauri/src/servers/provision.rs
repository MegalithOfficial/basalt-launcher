use std::path::{Path, PathBuf};

use crate::{
    download::{self, DownloadSpec},
    error::{Error, Result},
    files::FileManager,
    install, java,
    network::NetworkManager,
    state::AppState,
    tasks::TaskHandle,
};

use super::{software::Install, Server};

pub const PLATFORM_PLACEHOLDER: &str = "{platform}";
pub const SERVER_JAR: &str = "server.jar";
const EULA: &[u8] =
    b"#By changing the setting below to TRUE you are indicating your agreement to the Minecraft EULA (https://aka.ms/MinecraftEULA).\neula=true\n";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Provisioned {
    pub launch_jar: Option<String>,
    pub launch_argfiles: Vec<String>,
    pub flavor_version: Option<String>,
}

pub async fn fetch_json<T: serde::de::DeserializeOwned>(
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

pub async fn download_jar(
    install: &Install<'_>,
    url: String,
    sha256: Option<String>,
    size: Option<u64>,
) -> Result<()> {
    fetch(
        install.task,
        install.state,
        vec![DownloadSpec {
            url,
            dest: install.dir().join(SERVER_JAR),
            sha1: None,
            sha256,
            size,
        }],
    )
    .await
}

pub async fn fetch(task: &TaskHandle, state: &AppState, specs: Vec<DownloadSpec>) -> Result<()> {
    task.set_total(
        specs.len() as u64,
        specs.iter().filter_map(|spec| spec.size).sum(),
    );
    let concurrency = state.db.load_settings()?.concurrent_downloads;
    download::download_many_cancellable(
        &state.network,
        &state.files,
        specs,
        concurrency,
        |progress| {
            task.progress(
                progress.completed as u64,
                progress.total as u64,
                progress.downloaded_bytes,
                progress.total_bytes,
            );
        },
        Some(task.token()),
        None,
        Some(&|attempt, max, reason| task.note_retry(attempt, max, reason)),
    )
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

#[cfg(unix)]
pub fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(permissions.mode() | 0o755);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
pub fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[tracing::instrument(
    skip_all,
    fields(server_id = %server.id, software = %server.flavor, version = %server.version_id),
    err
)]
pub async fn install(state: &AppState, server: &Server, task: &TaskHandle) -> Result<Provisioned> {
    let dir = PathBuf::from(&server.dir);
    state.files.ensure_dir_async(&dir).await?;
    server
        .flavor
        .install(Install {
            state,
            server,
            task,
        })
        .await
}

#[derive(Debug, Clone)]
pub struct Installed {
    pub argfiles: Option<Vec<String>>,
    pub tail: String,
}

impl Installed {
    pub fn failed(&self, label: &str) -> Error {
        Error::other(format!(
            "The {label} installer did not finish:\n{}",
            self.tail
        ))
    }
}

pub async fn run_installer(
    install: &Install<'_>,
    url: String,
    name: String,
    argfiles: String,
) -> Result<Installed> {
    let state = install.state;
    let server = install.server;

    install.task.stage("server-installer");
    let installer_path = state.paths.cache().join("installers").join(&name);
    fetch(
        install.task,
        state,
        vec![DownloadSpec {
            url,
            dest: installer_path.clone(),
            sha1: None,
            sha256: None,
            size: None,
        }],
    )
    .await?;

    let vanilla = install::load_merged_version(state, install.game_version()).await?;
    let java = java::find_for_major(&state.files, vanilla.required_java_major(), None)
        .await
        .ok_or_else(|| Error::other("No Java found to run the server installer."))?;

    let dir = install.dir();
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
        return Ok(Installed {
            argfiles: Some(launch_argfiles),
            tail: String::new(),
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
    tracing::error!(server_id = %server.id, "server installer wrote no argument files:\n{tail}");
    Ok(Installed {
        argfiles: None,
        tail,
    })
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
}
