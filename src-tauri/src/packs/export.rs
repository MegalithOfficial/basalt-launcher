use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::Serialize;
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::{
    config::Instance,
    db::ContentFile,
    error::{Error, Result},
    files::FileManager,
    state::AppState,
};

use super::{is_content_path, is_skipped, loader_dependency_key, PackFormat, CONTENT_DIRS};

#[derive(Debug, Clone, Serialize)]
pub struct PackExport {
    pub path: String,
    pub format: PackFormat,
    pub linked: usize,
    pub bundled: usize,
    pub bytes: u64,
}

#[derive(Serialize)]
struct MrIndex {
    #[serde(rename = "formatVersion")]
    format_version: u8,
    game: &'static str,
    #[serde(rename = "versionId")]
    version_id: String,
    name: String,
    dependencies: BTreeMap<String, String>,
    files: Vec<MrFile>,
}

#[derive(Serialize)]
struct MrFile {
    path: String,
    hashes: MrHashes,
    downloads: Vec<String>,
    #[serde(rename = "fileSize")]
    file_size: u64,
}

#[derive(Serialize)]
struct MrHashes {
    sha1: String,
    sha512: String,
}

#[derive(Serialize)]
struct CfManifest {
    minecraft: CfMinecraft,
    #[serde(rename = "manifestType")]
    manifest_type: &'static str,
    #[serde(rename = "manifestVersion")]
    manifest_version: u8,
    name: String,
    version: String,
    author: String,
    files: Vec<CfFile>,
    overrides: &'static str,
}

#[derive(Serialize)]
struct CfMinecraft {
    version: String,
    #[serde(rename = "modLoaders")]
    mod_loaders: Vec<CfLoader>,
}

#[derive(Serialize)]
struct CfLoader {
    id: String,
    primary: bool,
}

#[derive(Serialize)]
struct CfFile {
    #[serde(rename = "projectID")]
    project_id: i64,
    #[serde(rename = "fileID")]
    file_id: i64,
    required: bool,
}

pub async fn export_instance(
    state: &AppState,
    instance: &Instance,
    format: PackFormat,
    destination: PathBuf,
) -> Result<PackExport> {
    let files = state.files.clone();
    let root = PathBuf::from(&instance.dir);

    let mut sources = Vec::new();
    for kind in CONTENT_DIRS {
        sources.extend(state.db.content_files(&instance.id, kind)?);
    }

    let instance = instance.clone();
    let stamp = chrono::Utc::now().format("%Y.%m.%d").to_string();
    tokio::task::spawn_blocking(move || {
        write_pack(
            &files,
            &instance,
            &root,
            &sources,
            format,
            &destination,
            &stamp,
        )
    })
    .await
    .map_err(|error| Error::other(format!("export task failed: {error}")))?
}

#[allow(clippy::too_many_arguments)]
fn write_pack(
    files: &FileManager,
    instance: &Instance,
    root: &Path,
    sources: &[ContentFile],
    format: PackFormat,
    destination: &Path,
    stamp: &str,
) -> Result<PackExport> {
    let provider = match format {
        PackFormat::Mrpack => "modrinth",
        PackFormat::Curseforge => "curseforge",
    };

    let mut index_files: Vec<MrFile> = Vec::new();
    let mut manifest_files: Vec<CfFile> = Vec::new();
    let mut bundled: Vec<(String, PathBuf)> = Vec::new();

    for path in walk(files, root) {
        let Some(relative) = relative_string(root, &path) else {
            continue;
        };
        if relative == ".basalt" || relative.starts_with(".basalt/") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if file_name.is_empty() || file_name == ".DS_Store" {
            continue;
        }

        let linked = if is_content_path(Path::new(&relative)) && !file_name.ends_with(".disabled") {
            sources.iter().find(|source| {
                source.file_name == file_name && source.provider.as_deref() == Some(provider)
            })
        } else {
            None
        };

        let declared = match (format, linked) {
            (PackFormat::Mrpack, Some(source)) => mr_file(source, &relative, &path, files)
                .map(|file| index_files.push(file))
                .is_some(),
            (PackFormat::Curseforge, Some(source)) => cf_file(source)
                .map(|file| manifest_files.push(file))
                .is_some(),
            _ => false,
        };

        if !declared {
            bundled.push((relative, path));
        }
    }

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let target = std::fs::File::create(destination)?;
    let mut zip = ZipWriter::new(target);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let linked = index_files.len() + manifest_files.len();
    match format {
        PackFormat::Mrpack => {
            let mut dependencies = BTreeMap::new();
            dependencies.insert("minecraft".to_string(), instance.version_id.clone());
            if let (Some(key), Some(version)) = (
                instance.loader.as_deref().and_then(loader_dependency_key),
                instance.loader_version.clone(),
            ) {
                dependencies.insert(key.to_string(), version);
            }

            index_files.sort_by(|a, b| a.path.cmp(&b.path));
            let index = MrIndex {
                format_version: 1,
                game: "minecraft",
                version_id: stamp.to_string(),
                name: instance.name.clone(),
                dependencies,
                files: index_files,
            };
            write_entry(
                &mut zip,
                options,
                "modrinth.index.json",
                serde_json::to_vec_pretty(&index)?.as_slice(),
            )?;
        }
        PackFormat::Curseforge => {
            let loaders = instance
                .loader
                .as_deref()
                .zip(instance.loader_version.as_deref())
                .map(|(loader, version)| CfLoader {
                    id: format!("{loader}-{version}"),
                    primary: true,
                })
                .into_iter()
                .collect();
            manifest_files.sort_by_key(|file| (file.project_id, file.file_id));
            let manifest = CfManifest {
                minecraft: CfMinecraft {
                    version: instance.version_id.clone(),
                    mod_loaders: loaders,
                },
                manifest_type: "minecraftModpack",
                manifest_version: 1,
                name: instance.name.clone(),
                version: stamp.to_string(),
                author: String::new(),
                files: manifest_files,
                overrides: "overrides",
            };
            write_entry(
                &mut zip,
                options,
                "manifest.json",
                serde_json::to_vec_pretty(&manifest)?.as_slice(),
            )?;
        }
    }

    bundled.sort_by(|a, b| a.0.cmp(&b.0));
    let mut bytes = 0u64;
    for (relative, path) in &bundled {
        let Ok(mut source) = files.open(path) else {
            continue;
        };
        zip.start_file(format!("overrides/{relative}"), options)
            .map_err(|error| Error::other(format!("writing the pack: {error}")))?;
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let read = source.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            zip.write_all(&buffer[..read])?;
            bytes += read as u64;
        }
    }

    zip.finish()
        .map_err(|error| Error::other(format!("finishing the pack: {error}")))?;

    Ok(PackExport {
        path: destination.display().to_string(),
        format,
        linked,
        bundled: bundled.len(),
        bytes,
    })
}

fn write_entry(
    zip: &mut ZipWriter<std::fs::File>,
    options: SimpleFileOptions,
    name: &str,
    body: &[u8],
) -> Result<()> {
    zip.start_file(name, options)
        .map_err(|error| Error::other(format!("writing the pack: {error}")))?;
    zip.write_all(body)?;
    Ok(())
}

fn mr_file(
    source: &ContentFile,
    relative: &str,
    path: &Path,
    files: &FileManager,
) -> Option<MrFile> {
    let project_id = source.project_id.as_deref()?;
    let version_id = source.version_id.as_deref()?;
    let sha1 = source.sha1.clone()?;
    let sha512 = source.sha512.clone()?;
    let size = files.metadata(path).ok()?.len();

    Some(MrFile {
        path: relative.to_string(),
        hashes: MrHashes { sha1, sha512 },
        downloads: vec![format!(
            "https://cdn.modrinth.com/data/{project_id}/versions/{version_id}/{}",
            encode_segment(&source.file_name)
        )],
        file_size: size,
    })
}

fn cf_file(source: &ContentFile) -> Option<CfFile> {
    Some(CfFile {
        project_id: source.project_id.as_deref()?.parse().ok()?,
        file_id: source.version_id.as_deref()?.parse().ok()?,
        required: true,
    })
}

fn encode_segment(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for byte in name.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn walk(files: &FileManager, root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut found = Vec::new();
    while let Some(directory) = pending.pop() {
        let Ok(entries) = files.read_dir(&directory) else {
            continue;
        };
        for path in entries {
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            if is_skipped(relative) {
                continue;
            }
            let Ok(metadata) = files.symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                found.push(path);
            }
        }
    }
    found
}

fn relative_string(root: &Path, path: &Path) -> Option<String> {
    Some(path.strip_prefix(root).ok()?.to_str()?.replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::encode_segment;

    #[test]
    fn encodes_url_unsafe_characters() {
        assert_eq!(encode_segment("sodium-0.6.jar"), "sodium-0.6.jar");
        assert_eq!(encode_segment("my mod+1.jar"), "my%20mod%2B1.jar");
    }
}
