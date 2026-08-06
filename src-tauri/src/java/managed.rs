use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use flate2::read::GzDecoder;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::{
    download::{self, DownloadSpec},
    error::{Error, Result},
    files::FileManager,
    java::{probe, JavaInfo},
    network::NetworkManager,
    tasks::TaskHandle,
};

const API_ROOT: &str = "https://api.adoptium.net/v3";
static INSTALL_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Platform {
    os: &'static str,
    arch: &'static str,
}

#[derive(Debug, Deserialize)]
struct Release {
    binaries: Vec<Binary>,
}

#[derive(Debug, Deserialize)]
struct Binary {
    package: Package,
}

#[derive(Debug, Deserialize)]
struct Package {
    checksum: String,
    link: String,
    name: String,
    size: u64,
}

fn platform_for(os: &str, arch: &str) -> Result<Platform> {
    let os = match os {
        "linux" => "linux",
        "windows" => "windows",
        "macos" => "mac",
        other => {
            return Err(Error::other(format!(
                "managed Java is not supported on {other}"
            )))
        }
    };
    let arch = match arch {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        "arm" => "arm",
        "riscv64" => "riscv64",
        "s390x" => "s390x",
        "powerpc64" if cfg!(target_endian = "little") => "ppc64le",
        other => {
            return Err(Error::other(format!(
                "managed Java is not supported on the {other} architecture"
            )))
        }
    };
    Ok(Platform { os, arch })
}

fn current_platform() -> Result<Platform> {
    platform_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn runtime_dir(files: &FileManager, major: u32, platform: Platform) -> PathBuf {
    files
        .paths()
        .runtimes()
        .join(format!("temurin-{major}-{}-{}", platform.os, platform.arch))
}

fn java_binary_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

fn java_binary_in(files: &FileManager, root: &Path) -> Option<PathBuf> {
    ["bin", "Contents/Home/bin", "jre/bin"]
        .into_iter()
        .map(|relative| root.join(relative).join(java_binary_name()))
        .find(|path| files.is_file(path).unwrap_or(false))
}

async fn installed_runtime(
    files: &FileManager,
    major: u32,
    platform: Platform,
) -> Option<JavaInfo> {
    let binary = java_binary_in(files, &runtime_dir(files, major, platform))?;
    let info = probe(&binary.display().to_string()).await?;
    (info.major == major).then_some(info)
}

fn metadata_url(major: u32, platform: Platform, image_type: &str) -> String {
    format!(
        "{API_ROOT}/assets/feature_releases/{major}/ga?architecture={}&heap_size=normal&image_type={image_type}&jvm_impl=hotspot&os={}&page=0&page_size=1&project=jdk&sort_method=DEFAULT&sort_order=DESC&vendor=eclipse",
        platform.arch, platform.os,
    )
}

async fn fetch_package(
    network: &NetworkManager,
    major: u32,
    platform: Platform,
    image_type: &str,
) -> Result<Option<Package>> {
    let releases: Vec<Release> = network
        .send(network.get(metadata_url(major, platform, image_type)))
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(releases
        .into_iter()
        .flat_map(|release| release.binaries)
        .map(|binary| binary.package)
        .next())
}

async fn resolve_package(
    network: &NetworkManager,
    major: u32,
    platform: Platform,
) -> Result<Package> {
    if let Some(package) = fetch_package(network, major, platform, "jre").await? {
        return Ok(package);
    }
    fetch_package(network, major, platform, "jdk")
        .await?
        .ok_or_else(|| {
            Error::other(format!(
                "Eclipse Temurin does not provide Java {major} for {}/{}.",
                platform.os, platform.arch
            ))
        })
}

fn archive_extension(name: &str) -> Result<&'static str> {
    if name.ends_with(".zip") {
        Ok("zip")
    } else if name.ends_with(".tar.gz") {
        Ok("tar.gz")
    } else {
        Err(Error::other(format!(
            "unsupported managed Java archive format: {name}"
        )))
    }
}

fn extract_zip(files: &FileManager, archive: &Path, staging: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(files.open(archive)?)
        .map_err(|error| Error::other(format!("opening managed Java ZIP: {error}")))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| Error::other(format!("reading managed Java ZIP: {error}")))?;
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| Error::other("managed Java archive contains an unsafe path"))?;
        let destination = staging.join(relative);
        if entry.is_dir() {
            files.ensure_dir(&destination)?;
            continue;
        }
        if entry.is_symlink() {
            return Err(Error::other(
                "managed Java ZIP contains an unsupported symbolic link",
            ));
        }
        let mut output = files.create(&destination)?;
        std::io::copy(&mut entry, &mut output)?;
    }
    Ok(())
}

fn extract_tar_gz(files: &FileManager, archive: &Path, staging: &Path) -> Result<()> {
    let decoder = GzDecoder::new(files.open(archive)?);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(staging)?;
    Ok(())
}

fn extracted_root(files: &FileManager, staging: &Path) -> Result<PathBuf> {
    if java_binary_in(files, staging).is_some() {
        return Ok(staging.to_path_buf());
    }
    let mut candidates = files
        .read_dir(staging)?
        .into_iter()
        .filter(|path| java_binary_in(files, path).is_some());
    let root = candidates
        .next()
        .ok_or_else(|| Error::other("managed Java archive contains no runnable Java binary"))?;
    if candidates.next().is_some() {
        return Err(Error::other(
            "managed Java archive contains multiple runtime roots",
        ));
    }
    Ok(root)
}

fn install_archive(
    files: &FileManager,
    archive: &Path,
    package_name: &str,
    destination: &Path,
) -> Result<()> {
    let staging = files
        .paths()
        .runtimes()
        .join(format!(".temurin-install-{}", uuid::Uuid::new_v4()));
    let backup = files
        .paths()
        .runtimes()
        .join(format!(".temurin-backup-{}", uuid::Uuid::new_v4()));
    files.ensure_dir(&staging)?;
    let outcome = (|| {
        match archive_extension(package_name)? {
            "zip" => extract_zip(files, archive, &staging)?,
            "tar.gz" => extract_tar_gz(files, archive, &staging)?,
            _ => unreachable!(),
        }
        let root = extracted_root(files, &staging)?;
        let had_destination = files.exists(destination)?;
        if had_destination {
            files.rename(destination, &backup)?;
        }
        if let Err(error) = files.rename(&root, destination) {
            if had_destination {
                files.rename(&backup, destination).map_err(|rollback| {
                    Error::other(format!(
                        "installing managed Java failed: {error}; restoring the previous runtime also failed: {rollback}"
                    ))
                })?;
            }
            return Err(error);
        }
        if had_destination {
            if let Err(error) = files.remove_managed_dir_all_if_exists(&backup) {
                tracing::warn!(path = %backup.display(), %error, "could not remove previous managed Java runtime");
            }
        }
        if root != staging {
            files.remove_managed_dir_all_if_exists(&staging)?;
        }
        Ok(())
    })();
    if outcome.is_err() {
        let _ = files.remove_managed_dir_all_if_exists(&staging);
    }
    outcome
}

pub async fn install(
    network: &NetworkManager,
    files: &FileManager,
    major: u32,
    task: &TaskHandle,
) -> Result<JavaInfo> {
    let _guard = INSTALL_LOCK.get_or_init(|| Mutex::new(())).lock().await;
    let platform = current_platform()?;
    if let Some(info) = installed_runtime(files, major, platform).await {
        return Ok(info);
    }

    task.stage("java-metadata");
    let package = resolve_package(network, major, platform).await?;
    let extension = archive_extension(&package.name)?;
    let archive = files.paths().cache().join("runtimes").join(format!(
        "temurin-{major}-{}-{}.{extension}",
        platform.os, platform.arch
    ));
    task.stage("java-download");
    task.set_total(1, package.size);
    download::download_many_cancellable(
        network,
        files,
        vec![DownloadSpec {
            url: package.link,
            dest: archive.clone(),
            sha1: None,
            sha256: Some(package.checksum),
            size: Some(package.size),
        }],
        1,
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
    .await?;

    if task.token().is_cancelled() {
        return Err(Error::Cancelled);
    }
    task.stage("java-extract");
    let destination = runtime_dir(files, major, platform);
    let files_for_extract = files.clone();
    let archive_for_extract = archive.clone();
    let package_name = package.name;
    let destination_for_extract = destination.clone();
    tokio::task::spawn_blocking(move || {
        install_archive(
            &files_for_extract,
            &archive_for_extract,
            &package_name,
            &destination_for_extract,
        )
    })
    .await
    .map_err(|error| Error::other(format!("managed Java extraction task failed: {error}")))??;

    let info = installed_runtime(files, major, platform)
        .await
        .ok_or_else(|| Error::other("downloaded Java runtime could not be started"))?;
    if let Err(error) = files.remove_file_if_exists(&archive) {
        tracing::warn!(path = %archive.display(), %error, "could not remove managed Java archive");
    }
    tracing::info!(major, path = %info.path, "managed Java runtime installed");
    Ok(info)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::{write::GzEncoder, Compression};
    use zip::write::SimpleFileOptions;

    use super::{
        archive_extension, extract_tar_gz, extract_zip, extracted_root, java_binary_name,
        metadata_url, platform_for, Platform,
    };
    use crate::{files::FileManager, paths::Paths};

    fn files(root: &std::path::Path) -> FileManager {
        FileManager::new(Paths::plain(root.to_path_buf())).unwrap()
    }

    #[test]
    fn maps_supported_platform_names_for_adoptium() {
        assert_eq!(
            platform_for("linux", "x86_64").unwrap(),
            Platform {
                os: "linux",
                arch: "x64"
            }
        );
        assert_eq!(platform_for("windows", "aarch64").unwrap().os, "windows");
        assert_eq!(platform_for("macos", "aarch64").unwrap().os, "mac");
        assert!(platform_for("freebsd", "x86_64").is_err());
        assert!(platform_for("linux", "mips64").is_err());
    }

    #[test]
    fn metadata_request_selects_verified_jre_archives() {
        let url = metadata_url(
            21,
            Platform {
                os: "linux",
                arch: "x64",
            },
            "jre",
        );
        assert!(url.contains("feature_releases/21/ga"));
        assert!(url.contains("architecture=x64"));
        assert!(url.contains("image_type=jre"));
        assert!(url.contains("vendor=eclipse"));
    }

    #[test]
    fn accepts_only_platform_archive_formats() {
        assert_eq!(archive_extension("temurin.zip").unwrap(), "zip");
        assert_eq!(archive_extension("temurin.tar.gz").unwrap(), "tar.gz");
        assert!(archive_extension("temurin.msi").is_err());
    }

    #[test]
    fn extracts_windows_zip_layout() {
        let root = std::env::temp_dir().join(format!("basalt-java-zip-{}", uuid::Uuid::new_v4()));
        let files = files(&root);
        let archive_path = root.join("runtime.zip");
        let staging = root.join("staging");
        let mut archive = zip::ZipWriter::new(files.create(&archive_path).unwrap());
        archive
            .start_file(
                format!("jdk/bin/{}", java_binary_name()),
                SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(b"java").unwrap();
        archive.finish().unwrap();
        files.ensure_dir(&staging).unwrap();

        extract_zip(&files, &archive_path, &staging).unwrap();
        assert_eq!(
            extracted_root(&files, &staging).unwrap(),
            staging.join("jdk")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn extracts_unix_tar_gz_layout() {
        let root = std::env::temp_dir().join(format!("basalt-java-tar-{}", uuid::Uuid::new_v4()));
        let files = files(&root);
        let archive_path = root.join("runtime.tar.gz");
        let staging = root.join("staging");
        let encoder = GzEncoder::new(files.create(&archive_path).unwrap(), Compression::default());
        let mut archive = tar::Builder::new(encoder);
        let body = b"java";
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        archive
            .append_data(
                &mut header,
                format!("jdk/bin/{}", java_binary_name()),
                &body[..],
            )
            .unwrap();
        archive.into_inner().unwrap().finish().unwrap();
        files.ensure_dir(&staging).unwrap();

        extract_tar_gz(&files, &archive_path, &staging).unwrap();
        assert_eq!(
            extracted_root(&files, &staging).unwrap(),
            staging.join("jdk")
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
