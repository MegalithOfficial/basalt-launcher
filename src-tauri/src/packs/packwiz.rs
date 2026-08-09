use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use futures::{stream, StreamExt, TryStreamExt};
use reqwest::Url;
use serde::{de::DeserializeOwned, Deserialize};

use crate::{
    db::ContentFile,
    error::{Error, Result},
    modpack::{self, ExpectedHash, MrEnv, MrFile, MrHashes, MrIndex},
    search::{self, curseforge, Provider},
    state::AppState,
};

use super::{PackPreview, PackPreviewFormat};

const MAX_METADATA_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
enum Location {
    Remote {
        url: Url,
        root: Url,
        relative: PathBuf,
    },
    Local {
        path: PathBuf,
        root: PathBuf,
        relative: PathBuf,
    },
}

impl Location {
    async fn source(state: &AppState, value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            return Err(Error::other("Choose a pack.toml file or enter its URL."));
        }
        let is_url = value
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
            || value
                .get(..8)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"));
        if is_url {
            let url = Url::parse(value)
                .map_err(|error| Error::other(format!("invalid packwiz URL: {error}")))?;
            validate_http_url(&url)?;
            let root = url
                .join(".")
                .map_err(|error| Error::other(format!("invalid packwiz URL: {error}")))?;
            return Ok(Self::Remote {
                url,
                root,
                relative: PathBuf::from("pack.toml"),
            });
        }

        let files = state.files.clone();
        let source = PathBuf::from(value);
        let path = tokio::task::spawn_blocking(move || {
            let path = files.canonicalize_external(source)?;
            if !path.is_file() {
                return Err(Error::NotFound(path.display().to_string()));
            }
            Ok(path)
        })
        .await
        .map_err(|error| Error::other(format!("pack path task failed: {error}")))??;
        let root = path
            .parent()
            .ok_or_else(|| Error::other("pack.toml has no parent directory"))?
            .to_path_buf();
        Ok(Self::Local {
            path,
            root,
            relative: PathBuf::from("pack.toml"),
        })
    }

    async fn join(&self, state: &AppState, relative: &str) -> Result<Self> {
        let joined = resolve_pack_path(self.relative(), relative)?;
        match self {
            Self::Remote { root, .. } => {
                let mut url = root.clone();
                {
                    let mut segments = url
                        .path_segments_mut()
                        .map_err(|_| Error::other("packwiz URL cannot be used as a base"))?;
                    segments.pop_if_empty();
                    for component in joined.components() {
                        let std::path::Component::Normal(segment) = component else {
                            return Err(Error::other(format!(
                                "unsafe path in packwiz pack: {relative}"
                            )));
                        };
                        let segment = segment
                            .to_str()
                            .ok_or_else(|| Error::other("packwiz path is not valid Unicode"))?;
                        segments.push(segment);
                    }
                }
                validate_http_url(&url)?;
                if url.origin() != root.origin() || !url.path().starts_with(root.path()) {
                    return Err(Error::other(format!(
                        "packwiz URL escapes the pack directory: {relative}"
                    )));
                }
                Ok(Self::Remote {
                    url,
                    root: root.clone(),
                    relative: joined,
                })
            }
            Self::Local { root, .. } => {
                let files = state.files.clone();
                let candidate = root.join(&joined);
                let path =
                    tokio::task::spawn_blocking(move || files.canonicalize_external(candidate))
                        .await
                        .map_err(|error| {
                            Error::other(format!("pack path task failed: {error}"))
                        })??;
                if !path.starts_with(root) || !path.is_file() {
                    return Err(Error::other(format!(
                        "packwiz path escapes the pack directory: {relative}"
                    )));
                }
                Ok(Self::Local {
                    path,
                    root: root.clone(),
                    relative: joined,
                })
            }
        }
    }

    fn relative(&self) -> &Path {
        match self {
            Self::Remote { relative, .. } | Self::Local { relative, .. } => relative,
        }
    }

    async fn read(&self, state: &AppState) -> Result<Vec<u8>> {
        let bytes = match self {
            Self::Remote { url, .. } => {
                state
                    .network
                    .send(state.network.get(url.clone()))
                    .await?
                    .error_for_status()?
                    .bytes()
                    .await?
            }
            Self::Local { path, .. } => {
                let size = tokio::fs::metadata(path).await?.len();
                if size > MAX_METADATA_BYTES as u64 {
                    return Err(Error::ResponseTooLarge {
                        limit: MAX_METADATA_BYTES,
                        actual: size,
                    });
                }
                state.files.read_external_async(path).await?
            }
        };
        if bytes.len() > MAX_METADATA_BYTES {
            return Err(Error::ResponseTooLarge {
                limit: MAX_METADATA_BYTES,
                actual: bytes.len() as u64,
            });
        }
        Ok(bytes)
    }

    fn download(&self) -> (Vec<String>, Option<PathBuf>) {
        match self {
            Self::Remote { url, .. } => (vec![url.to_string()], None),
            Self::Local { path, .. } => (Vec::new(), Some(path.clone())),
        }
    }

    fn identity(&self) -> String {
        match self {
            Self::Remote { url, .. } => {
                let mut public = url.clone();
                public.set_query(None);
                public.set_fragment(None);
                public.to_string()
            }
            Self::Local { path, .. } => path.display().to_string(),
        }
    }
}

async fn parse_toml<T>(
    bytes: Vec<u8>,
    label: &'static str,
    verification: Option<(ExpectedHash, String)>,
) -> Result<T>
where
    T: DeserializeOwned + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        if let Some((hash, source)) = verification {
            hash.verify(&bytes, &source)?;
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|error| Error::other(format!("{label} is not UTF-8: {error}")))?;
        toml::from_str(text).map_err(|error| Error::other(format!("invalid {label}: {error}")))
    })
    .await
    .map_err(|error| Error::other(format!("pack metadata task failed: {error}")))?
}

fn resolve_pack_path(base_file: &Path, relative: &str) -> Result<PathBuf> {
    if relative.contains('\\') || relative.starts_with("//") || Url::parse(relative).is_ok() {
        return Err(Error::other(format!(
            "unsafe path in packwiz pack: {relative}"
        )));
    }
    let normalized = relative.replace("%2e", ".").replace("%2E", ".");
    let root_relative = normalized.strip_prefix('/');
    let path = Path::new(root_relative.unwrap_or(&normalized));
    let mut resolved = if root_relative.is_some() {
        PathBuf::new()
    } else {
        base_file
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf()
    };
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => resolved.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir if resolved.pop() => {}
            _ => {
                return Err(Error::other(format!(
                    "packwiz path escapes the pack directory: {relative}"
                )));
            }
        }
    }
    if resolved.as_os_str().is_empty() {
        return Err(Error::other(format!(
            "unsafe path in packwiz pack: {relative}"
        )));
    }
    Ok(resolved)
}

fn validate_http_url(url: &Url) -> Result<()> {
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(Error::other("packwiz URLs must use HTTP or HTTPS"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(Error::other("packwiz URLs cannot contain credentials"));
    }
    Ok(())
}

#[derive(Deserialize)]
struct PackToml {
    name: String,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(rename = "pack-format", default)]
    pack_format: Option<String>,
    index: PackIndexRef,
    versions: HashMap<String, String>,
}

#[derive(Deserialize)]
struct PackIndexRef {
    file: String,
    #[serde(rename = "hash-format")]
    hash_format: String,
    hash: String,
}

#[derive(Deserialize)]
struct PackIndex {
    #[serde(rename = "hash-format")]
    hash_format: String,
    #[serde(default)]
    files: Vec<PackIndexFile>,
}

#[derive(Deserialize)]
struct PackIndexFile {
    file: String,
    hash: String,
    #[serde(default)]
    alias: Option<String>,
    #[serde(rename = "hash-format")]
    hash_format: Option<String>,
    #[serde(default)]
    metafile: bool,
    #[serde(default)]
    preserve: bool,
}

#[derive(Deserialize)]
struct MetaFile {
    name: String,
    filename: String,
    #[serde(default)]
    side: Option<String>,
    download: MetaDownload,
    #[serde(default)]
    option: Option<MetaOption>,
    #[serde(default)]
    update: Option<MetaUpdate>,
}

#[derive(Deserialize)]
struct MetaDownload {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(rename = "hash-format")]
    hash_format: String,
    hash: String,
}

#[derive(Deserialize)]
struct MetaOption {
    optional: bool,
    #[serde(default)]
    default: bool,
}

#[derive(Deserialize, Default)]
struct MetaUpdate {
    #[serde(default)]
    curseforge: Option<CurseforgeUpdate>,
    #[serde(default)]
    modrinth: Option<ModrinthUpdate>,
}

#[derive(Deserialize)]
struct CurseforgeUpdate {
    #[serde(rename = "project-id")]
    project_id: u64,
    #[serde(rename = "file-id")]
    file_id: u64,
}

#[derive(Deserialize)]
struct ModrinthUpdate {
    #[serde(rename = "mod-id")]
    mod_id: String,
    version: String,
}

struct ResolvedEntry {
    file: Option<MrFile>,
    link: Option<(String, String, ContentFile)>,
    optional: bool,
    disabled_optional: bool,
    server_only: bool,
    bundled: bool,
    preserved: bool,
}

pub(crate) struct ResolvedPackwiz {
    pub(crate) preview: PackPreview,
    pub(crate) index: MrIndex,
    pub(crate) links: Vec<(String, String, ContentFile)>,
    pub(crate) source: String,
    pub(crate) revision: String,
}

fn validate_pack_format(value: Option<&str>) -> Result<()> {
    let value = value.unwrap_or("packwiz:1.0.0");
    let version = value
        .strip_prefix("packwiz:")
        .ok_or_else(|| Error::other(format!("unsupported pack format: {value}")))?;
    let version = semver::Version::parse(version)
        .map_err(|error| Error::other(format!("invalid packwiz format version: {error}")))?;
    if version.major > 1 {
        return Err(Error::other(format!(
            "unsupported packwiz format version: {value}"
        )));
    }
    Ok(())
}

fn dependencies(versions: &HashMap<String, String>) -> Result<HashMap<String, String>> {
    let mut dependencies = HashMap::with_capacity(versions.len());
    for (component, version) in versions {
        let component = match component.as_str() {
            "minecraft" | "forge" | "neoforge" => component.as_str(),
            "fabric" => "fabric-loader",
            "quilt" => "quilt-loader",
            unsupported => {
                return Err(Error::other(format!(
                    "This pack needs an unsupported component: {unsupported}"
                )));
            }
        };
        dependencies.insert(component.to_string(), version.clone());
    }
    let loader_count = ["fabric-loader", "quilt-loader", "forge", "neoforge"]
        .into_iter()
        .filter(|loader| dependencies.contains_key(*loader))
        .count();
    if loader_count > 1 {
        return Err(Error::other("The pack declares more than one mod loader."));
    }
    Ok(dependencies)
}

fn destination_for_metafile(metadata_path: &str, filename: &str) -> Result<String> {
    resolve_pack_path(Path::new(metadata_path), filename).map(|path| pack_path(&path))
}

fn pack_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn content_destination(destination: &str) -> Option<(&'static str, &str)> {
    let (kind, file_name) = destination.split_once('/')?;
    let kind = match kind {
        "mods" => "mods",
        "resourcepacks" => "resourcepacks",
        "shaderpacks" => "shaderpacks",
        _ => return None,
    };
    if file_name.is_empty() || file_name.contains('/') {
        return None;
    }
    Some((kind, file_name))
}

fn apply_project_info(
    links: &mut [(String, String, ContentFile)],
    provider: Provider,
    projects: &[search::ProjectSummary],
) {
    let by_id: HashMap<&str, &search::ProjectSummary> = projects
        .iter()
        .map(|project| (project.id.as_str(), project))
        .collect();
    for (_, _, file) in links {
        if file.provider.as_deref() != Some(provider.as_str()) {
            continue;
        }
        let Some(project) = file
            .project_id
            .as_deref()
            .and_then(|project_id| by_id.get(project_id))
        else {
            continue;
        };
        file.title = Some(project.title.clone());
        file.icon_url = project.icon_url.clone();
    }
}

async fn enrich_links(state: &AppState, links: &mut [(String, String, ContentFile)]) {
    for provider in [Provider::Modrinth, Provider::Curseforge] {
        let mut ids = links
            .iter()
            .filter(|(_, _, file)| file.provider.as_deref() == Some(provider.as_str()))
            .filter_map(|(_, _, file)| file.project_id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            continue;
        }
        match search::resolve_projects(state, provider, &ids).await {
            Ok(projects) => apply_project_info(links, provider, &projects),
            Err(error) => tracing::warn!(
                provider = provider.as_str(),
                %error,
                "could not load packwiz project metadata"
            ),
        }
    }
}

struct LoadedEntry {
    order: usize,
    entry: PackIndexFile,
    hash: ExpectedHash,
    file: LoadedFile,
}

enum LoadedFile {
    Direct(Location),
    Metafile(Location, Box<MetaFile>),
}

async fn load_entry(
    state: &AppState,
    index_location: &Location,
    default_hash_format: &str,
    order: usize,
    entry: PackIndexFile,
) -> Result<LoadedEntry> {
    let entry_hash = ExpectedHash::parse(
        entry.hash_format.as_deref().unwrap_or(default_hash_format),
        &entry.hash,
    )?;
    if entry.metafile {
        let metadata_location = index_location.join(state, &entry.file).await?;
        let metadata_bytes = metadata_location.read(state).await?;
        let metadata: MetaFile = parse_toml(
            metadata_bytes,
            "packwiz metafile",
            Some((entry_hash.clone(), metadata_location.identity())),
        )
        .await?;
        if let Some(side) = metadata.side.as_deref() {
            if !matches!(side, "client" | "server" | "both") {
                return Err(Error::other(format!("unsupported packwiz side: {side}")));
            }
        }
        return Ok(LoadedEntry {
            order,
            entry,
            hash: entry_hash,
            file: LoadedFile::Metafile(metadata_location, Box::new(metadata)),
        });
    }

    let source = index_location.join(state, &entry.file).await?;
    Ok(LoadedEntry {
        order,
        entry,
        hash: entry_hash,
        file: LoadedFile::Direct(source),
    })
}

fn option_state(metadata: &MetaFile) -> (bool, bool) {
    let Some(option) = &metadata.option else {
        return (false, false);
    };
    (option.optional, option.optional && !option.default)
}

async fn curseforge_downloads(
    state: &AppState,
    entries: &[LoadedEntry],
) -> Result<HashMap<u64, String>> {
    let mut required = HashMap::new();
    for entry in entries {
        let LoadedFile::Metafile(_, metadata) = &entry.file else {
            continue;
        };
        let (_, disabled) = option_state(metadata);
        if disabled
            || metadata.side.as_deref() == Some("server")
            || metadata.download.url.is_some()
            || metadata.download.mode.as_deref() != Some("metadata:curseforge")
        {
            continue;
        }
        let Some(update) = metadata
            .update
            .as_ref()
            .and_then(|update| update.curseforge.as_ref())
        else {
            continue;
        };
        if required
            .insert(update.file_id, update.project_id)
            .is_some_and(|project_id| project_id != update.project_id)
        {
            return Err(Error::other(format!(
                "CurseForge file {} is declared for multiple projects",
                update.file_id
            )));
        }
    }
    if required.is_empty() {
        return Ok(HashMap::new());
    }
    let ids = required
        .keys()
        .map(|id| {
            i64::try_from(*id)
                .map_err(|_| Error::other(format!("CurseForge file ID is too large: {id}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let files = curseforge::files(state, &ids).await?;
    let mut downloads = HashMap::with_capacity(files.len());
    for file in files {
        let Some(project_id) = required.get(&file.id) else {
            continue;
        };
        if file.mod_id != *project_id {
            return Err(Error::other(format!(
                "CurseForge file {} belongs to project {}, not {}",
                file.id, file.mod_id, project_id
            )));
        }
        let url = file.download_url.ok_or_else(|| {
            Error::other(format!(
                "{} cannot be downloaded through third-party launchers",
                file.file_name
            ))
        })?;
        downloads.insert(file.id, url);
    }
    if let Some((file_id, project_id)) = required
        .iter()
        .find(|(file_id, _)| !downloads.contains_key(file_id))
    {
        return Err(Error::other(format!(
            "CurseForge file {file_id} for project {project_id} was not found"
        )));
    }
    Ok(downloads)
}

fn resolve_entry(
    index_location: &Location,
    curseforge_downloads: &HashMap<u64, String>,
    loaded: LoadedEntry,
) -> Result<ResolvedEntry> {
    let LoadedEntry {
        entry,
        hash: entry_hash,
        file,
        ..
    } = loaded;

    let (metadata_location, metadata) = match file {
        LoadedFile::Direct(source) => {
            let (downloads, local_source) = source.download();
            let destination = match entry.alias.as_deref() {
                Some(alias) => resolve_pack_path(index_location.relative(), alias)?,
                None => source.relative().to_path_buf(),
            };
            return Ok(ResolvedEntry {
                file: Some(MrFile {
                    path: pack_path(&destination),
                    hashes: MrHashes {
                        sha1: entry_hash.sha1(),
                        expected: Some(entry_hash),
                    },
                    downloads,
                    file_size: None,
                    env: None,
                    local_source,
                    preserve: entry.preserve,
                }),
                link: None,
                optional: false,
                disabled_optional: false,
                server_only: false,
                bundled: true,
                preserved: entry.preserve,
            });
        }
        LoadedFile::Metafile(location, metadata) => (location, metadata),
    };

    let (optional, disabled_optional) = option_state(&metadata);
    let server_only = metadata.side.as_deref() == Some("server");
    if disabled_optional || server_only {
        return Ok(ResolvedEntry {
            file: None,
            link: None,
            optional,
            disabled_optional,
            server_only,
            bundled: false,
            preserved: false,
        });
    }

    let hash = ExpectedHash::parse(&metadata.download.hash_format, &metadata.download.hash)?;
    let destination = match entry.alias.as_deref() {
        Some(alias) => pack_path(&resolve_pack_path(index_location.relative(), alias)?),
        None => destination_for_metafile(
            &metadata_location.relative().to_string_lossy(),
            &metadata.filename,
        )?,
    };
    let url = if let Some(url) = metadata.download.url.as_deref() {
        let url = Url::parse(url).map_err(|error| {
            Error::other(format!(
                "invalid download URL for {}: {error}",
                metadata.name
            ))
        })?;
        validate_http_url(&url)?;
        url.to_string()
    } else if metadata.download.mode.as_deref() == Some("metadata:curseforge") {
        let update = metadata
            .update
            .as_ref()
            .and_then(|update| update.curseforge.as_ref())
            .ok_or_else(|| {
                Error::other(format!(
                    "{} uses CurseForge metadata without project and file IDs",
                    metadata.name
                ))
            })?;
        curseforge_downloads
            .get(&update.file_id)
            .cloned()
            .ok_or_else(|| {
                Error::other(format!(
                    "CurseForge file {} for project {} was not found",
                    update.file_id, update.project_id
                ))
            })?
    } else {
        return Err(Error::other(format!(
            "{} has no supported download URL",
            metadata.name
        )));
    };

    let now = chrono::Utc::now().timestamp();
    let link = metadata.update.as_ref().and_then(|update| {
        let (kind, file_name) = content_destination(&destination)?;
        let file = if let Some(project) = &update.modrinth {
            ContentFile {
                file_name: file_name.to_string(),
                sha1: hash.sha1(),
                provider: Some("modrinth".to_string()),
                project_id: Some(project.mod_id.clone()),
                version_id: Some(project.version.clone()),
                title: Some(metadata.name.clone()),
                origin: "pack".to_string(),
                installed_at: now,
                ..Default::default()
            }
        } else if let Some(project) = &update.curseforge {
            ContentFile {
                file_name: file_name.to_string(),
                sha1: hash.sha1(),
                provider: Some("curseforge".to_string()),
                project_id: Some(project.project_id.to_string()),
                version_id: Some(project.file_id.to_string()),
                title: Some(metadata.name.clone()),
                origin: "pack".to_string(),
                installed_at: now,
                ..Default::default()
            }
        } else {
            return None;
        };
        Some((kind.to_string(), destination.clone(), file))
    });

    Ok(ResolvedEntry {
        file: Some(MrFile {
            path: destination,
            hashes: MrHashes {
                sha1: hash.sha1(),
                expected: Some(hash),
            },
            downloads: vec![url],
            file_size: None,
            env: Some(MrEnv {
                client: Some("required".to_string()),
            }),
            local_source: None,
            preserve: entry.preserve,
        }),
        link,
        optional,
        disabled_optional: false,
        server_only: false,
        bundled: false,
        preserved: entry.preserve,
    })
}

pub(crate) async fn resolve(state: &AppState, source: &str) -> Result<ResolvedPackwiz> {
    let pack_location = Location::source(state, source).await?;
    let pack_bytes = pack_location.read(state).await?;
    let pack: PackToml = parse_toml(pack_bytes, "pack.toml", None).await?;
    validate_pack_format(pack.pack_format.as_deref())?;

    let index_location = pack_location.join(state, &pack.index.file).await?;
    let index_bytes = index_location.read(state).await?;
    let index_hash = ExpectedHash::parse(&pack.index.hash_format, &pack.index.hash)?;
    let index: PackIndex = parse_toml(
        index_bytes,
        "packwiz index",
        Some((index_hash, index_location.identity())),
    )
    .await?;

    let declared_files = index.files.len();
    let default_hash_format = index.hash_format.clone();
    let mut loaded = stream::iter(index.files.into_iter().enumerate())
        .map(|(order, entry)| {
            load_entry(state, &index_location, &default_hash_format, order, entry)
        })
        .buffer_unordered(8)
        .try_collect::<Vec<_>>()
        .await?;
    loaded.sort_by_key(|entry| entry.order);
    let curseforge_downloads = curseforge_downloads(state, &loaded).await?;
    let mut resolved = loaded
        .into_iter()
        .map(|entry| resolve_entry(&index_location, &curseforge_downloads, entry))
        .collect::<Result<Vec<_>>>()?;

    let optional = resolved.iter().filter(|entry| entry.optional).count();
    let disabled_optional = resolved
        .iter()
        .filter(|entry| entry.disabled_optional)
        .count();
    let server_only = resolved.iter().filter(|entry| entry.server_only).count();
    let bundled = resolved.iter().filter(|entry| entry.bundled).count();
    let preserved = resolved.iter().filter(|entry| entry.preserved).count();
    let mut links = resolved
        .iter_mut()
        .filter_map(|entry| entry.link.take())
        .collect::<Vec<_>>();
    enrich_links(state, &mut links).await;
    let files = resolved
        .into_iter()
        .filter_map(|entry| entry.file)
        .collect::<Vec<_>>();
    let dependencies = dependencies(&pack.versions)?;
    let game_version = dependencies.get("minecraft").cloned();
    let loader = modpack::loader_from_dependencies(&dependencies)?;
    let mut warnings = Vec::new();
    if optional > 0 {
        warnings.push(format!(
            "{optional} optional files use the pack defaults; {disabled_optional} are disabled"
        ));
    }
    if server_only > 0 {
        warnings.push(format!("{server_only} server-only files will be skipped"));
    }
    if preserved > 0 {
        warnings.push(format!(
            "{preserved} files are marked to preserve local changes"
        ));
    }
    if game_version.is_none() {
        warnings.push("The pack does not declare a Minecraft version.".to_string());
    }

    Ok(ResolvedPackwiz {
        preview: PackPreview {
            format: PackPreviewFormat::Packwiz,
            name: pack.name.clone(),
            version: pack.version,
            author: pack.author.filter(|value| !value.trim().is_empty()),
            game_version: game_version.clone().unwrap_or_default(),
            loader: loader.as_ref().map(|(name, _)| name.clone()),
            loader_version: loader.as_ref().map(|(_, version)| version.clone()),
            declared_files,
            override_files: bundled,
            override_bytes: 0,
            warnings,
            importable: game_version.is_some(),
        },
        index: MrIndex {
            name: pack.name,
            dependencies,
            files,
        },
        links,
        source: pack_location.identity(),
        revision: pack.index.hash,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;

    use sha2::{Digest, Sha256};

    use crate::{
        db::ContentFile,
        modpack::ExpectedHash,
        search::{ProjectSummary, Provider},
    };

    use super::{
        apply_project_info, content_destination, dependencies, destination_for_metafile, resolve,
        resolve_pack_path, validate_pack_format, Location,
    };

    fn test_state(root: &Path) -> crate::state::AppState {
        let paths = crate::paths::Paths::plain(root.join("basalt-data"));
        let files = crate::files::FileManager::new(paths).unwrap();
        files.ensure_base_dirs().unwrap();
        let db = crate::db::Db::open(&files).unwrap();
        crate::state::AppState::new(files, db)
    }

    #[test]
    fn validates_supported_packwiz_versions() {
        assert!(validate_pack_format(None).is_ok());
        assert!(validate_pack_format(Some("packwiz:0.9.0")).is_ok());
        assert!(validate_pack_format(Some("packwiz:1.0.0")).is_ok());
        assert!(validate_pack_format(Some("packwiz:1.1.0-beta.1")).is_ok());
        assert!(validate_pack_format(Some("packwiz:2.0.0")).is_err());
        assert!(validate_pack_format(Some("other:1.1.0")).is_err());
    }

    #[test]
    fn rejects_components_basalt_cannot_install() {
        let unsupported = HashMap::from([
            ("minecraft".to_string(), "1.12.2".to_string()),
            ("liteloader".to_string(), "1.12.2".to_string()),
        ]);
        assert!(dependencies(&unsupported).is_err());

        let multiple = HashMap::from([
            ("minecraft".to_string(), "1.20.1".to_string()),
            ("fabric".to_string(), "0.16.0".to_string()),
            ("forge".to_string(), "47.0.0".to_string()),
        ]);
        assert!(dependencies(&multiple).is_err());
    }

    #[test]
    fn verifies_all_packwiz_hash_formats() {
        let bytes = b"abc";
        for (format, expected) in [
            ("sha256", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
            ("sha512", "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"),
            ("sha1", "a9993e364706816aba3e25717850c26c9cd0d89d"),
            ("md5", "900150983cd24fb0d6963f7d28e17f72"),
            ("murmur2", "1621425345"),
        ] {
            ExpectedHash::parse(format, expected)
                .unwrap()
                .verify(bytes, "fixture")
                .unwrap();
        }
    }

    #[test]
    fn resolves_metafile_destinations_without_escaping() {
        assert_eq!(
            destination_for_metafile("mods/example.pw.toml", "example.jar").unwrap(),
            "mods/example.jar"
        );
        assert_eq!(
            destination_for_metafile("mods/example.pw.toml", "nested/example.jar").unwrap(),
            "mods/nested/example.jar"
        );
        assert_eq!(
            destination_for_metafile("sub/mods/example.pw.toml", "example.jar").unwrap(),
            "sub/mods/example.jar"
        );
        assert_eq!(
            destination_for_metafile("sub/meta/example.pw.toml", "../../mods/example.jar").unwrap(),
            "mods/example.jar"
        );
        assert!(destination_for_metafile("mods/example.pw.toml", "../../escape.jar").is_err());
    }

    #[test]
    fn links_only_top_level_content_files() {
        assert_eq!(
            content_destination("mods/example.jar"),
            Some(("mods", "example.jar"))
        );
        assert_eq!(
            content_destination("resourcepacks/example.zip"),
            Some(("resourcepacks", "example.zip"))
        );
        assert_eq!(content_destination("mods/nested/example.jar"), None);
        assert_eq!(content_destination("config/example.toml"), None);
    }

    #[test]
    fn project_metadata_adds_the_content_icon() {
        let mut links = vec![(
            "mods".to_string(),
            "mods/sodium.jar".to_string(),
            ContentFile {
                file_name: "sodium.jar".to_string(),
                provider: Some("modrinth".to_string()),
                project_id: Some("AANobbMI".to_string()),
                title: Some("Sodium from packwiz".to_string()),
                ..Default::default()
            },
        )];
        let projects = vec![ProjectSummary {
            id: "AANobbMI".to_string(),
            slug: Some("sodium".to_string()),
            title: "Sodium".to_string(),
            description: String::new(),
            icon_url: Some("https://cdn.modrinth.com/sodium.png".to_string()),
            downloads: 0,
            follows: 0,
            author: String::new(),
            categories: Vec::new(),
            game_versions: Vec::new(),
            loaders: Vec::new(),
            updated: None,
            color: None,
        }];

        apply_project_info(&mut links, Provider::Modrinth, &projects);

        assert_eq!(links[0].2.title.as_deref(), Some("Sodium"));
        assert_eq!(
            links[0].2.icon_url.as_deref(),
            Some("https://cdn.modrinth.com/sodium.png")
        );
    }

    #[test]
    fn resolves_nested_index_paths_against_the_pack_root() {
        assert_eq!(
            resolve_pack_path(std::path::Path::new("sub/index.toml"), "config/a.txt").unwrap(),
            std::path::PathBuf::from("sub/config/a.txt")
        );
        assert_eq!(
            resolve_pack_path(std::path::Path::new("sub/index.toml"), "../config/a.txt").unwrap(),
            std::path::PathBuf::from("config/a.txt")
        );
        assert_eq!(
            resolve_pack_path(std::path::Path::new("sub/index.toml"), "/mods/a.jar").unwrap(),
            std::path::PathBuf::from("mods/a.jar")
        );
        assert!(
            resolve_pack_path(std::path::Path::new("sub/index.toml"), "../../outside.txt").is_err()
        );
    }

    #[tokio::test]
    async fn remote_references_cannot_leave_the_pack_url_root() {
        let root = std::env::temp_dir().join(format!("basalt-packwiz-{}", uuid::Uuid::new_v4()));
        let state = test_state(&root);
        let pack = Location::source(&state, "https://example.com/packs/demo/pack.toml")
            .await
            .unwrap();
        let signed = Location::source(
            &state,
            "https://example.com/packs/demo/pack.toml?token=secret#download",
        )
        .await
        .unwrap();
        assert_eq!(
            signed.identity(),
            "https://example.com/packs/demo/pack.toml"
        );
        let index = pack.join(&state, "sub/index.toml").await.unwrap();
        assert!(index.join(&state, "../config/a.txt").await.is_ok());
        assert_eq!(
            index.join(&state, "/mods/a.jar").await.unwrap().identity(),
            "https://example.com/packs/demo/mods/a.jar"
        );
        assert!(index.join(&state, "../../outside.txt").await.is_err());
        assert_eq!(
            index
                .join(&state, "/mods/foo#bar?.jar")
                .await
                .unwrap()
                .identity(),
            "https://example.com/packs/demo/mods/foo%23bar%3F.jar"
        );
        assert_eq!(
            index.join(&state, "%2e%2e/file").await.unwrap().identity(),
            "https://example.com/packs/demo/file"
        );
        assert!(index
            .join(&state, "%2e%2e/%2e%2e/outside.txt")
            .await
            .is_err());
        assert!(index
            .join(&state, "https://other.example/file")
            .await
            .is_err());

        drop(state);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn resolves_a_local_pack_toml_and_neighboring_files() {
        let root = std::env::temp_dir().join(format!("basalt-packwiz-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("mods")).unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::create_dir_all(root.join("metadata")).unwrap();
        std::fs::write(root.join("config/options.txt"), b"setting=true\n").unwrap();
        let metafile = r#"name = "Example"
filename = "example.jar"
side = "client"

[download]
url = "https://example.org/example.jar"
hash-format = "sha1"
hash = "a9993e364706816aba3e25717850c26c9cd0d89d"
"#;
        std::fs::write(root.join("mods/example.pw.toml"), metafile).unwrap();
        let direct_hash = format!("{:x}", Sha256::digest(b"setting=true\n"));
        let metafile_hash = format!("{:x}", Sha256::digest(metafile.as_bytes()));
        let index = format!(
            r#"hash-format = "sha256"

[[files]]
file = "../config/options.txt"
hash = "{direct_hash}"
preserve = true

[[files]]
file = "../config/options.txt"
alias = "../config/options-copy.txt"
hash = "{direct_hash}"

[[files]]
file = "../mods/example.pw.toml"
alias = "../mods/aliased-example.jar"
hash = "{metafile_hash}"
metafile = true
"#
        );
        std::fs::write(root.join("metadata/index.toml"), &index).unwrap();
        let index_hash = format!("{:x}", Sha256::digest(index.as_bytes()));
        let pack = format!(
            r#"name = "Local example"
author = "Basalt"
version = "1.0.0"

[index]
file = "metadata/index.toml"
hash-format = "sha256"
hash = "{index_hash}"

[versions]
minecraft = "1.20.1"
fabric = "0.16.0"
"#
        );
        let pack_path = root.join("pack.toml");
        std::fs::write(&pack_path, pack).unwrap();

        let state = test_state(&root);
        let resolved = resolve(&state, &pack_path.display().to_string())
            .await
            .unwrap();

        assert_eq!(resolved.preview.name, "Local example");
        assert_eq!(resolved.preview.game_version, "1.20.1");
        assert_eq!(resolved.preview.loader.as_deref(), Some("fabric"));
        assert_eq!(resolved.index.files.len(), 3);
        assert_eq!(resolved.index.files[0].path, "config/options.txt");
        assert_eq!(resolved.index.files[1].path, "config/options-copy.txt");
        assert_eq!(resolved.index.files[2].path, "mods/aliased-example.jar");
        assert!(resolved
            .preview
            .warnings
            .iter()
            .any(|warning| warning.contains("preserve")));

        drop(state);
        std::fs::remove_dir_all(root).unwrap();
    }
}
