use serde::{Deserialize, Serialize};

use crate::content;
use crate::download::{self, DownloadSpec};
use crate::error::{Error, Result};
use crate::state::AppState;

const MODRINTH: &str = "https://api.modrinth.com/v2";
const CURSEFORGE: &str = "https://api.curseforge.com/v1";
const CF_GAME_MINECRAFT: u32 = 432;

#[derive(Debug, Clone, Copy)]
pub enum Provider {
    Modrinth,
    Curseforge,
}

impl Provider {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "modrinth" => Ok(Provider::Modrinth),
            "curseforge" => Ok(Provider::Curseforge),
            other => Err(Error::other(format!("unknown provider {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub author: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectDetails {
    pub id: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub body_format: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub author: String,
    pub gallery: Vec<String>,
}

fn modrinth_project_type(kind: &str) -> Result<&'static str> {
    match kind {
        "mods" => Ok("mod"),
        "resourcepacks" => Ok("resourcepack"),
        "shaderpacks" => Ok("shader"),
        other => Err(Error::other(format!("cannot search for {other}"))),
    }
}

fn curseforge_class_id(kind: &str) -> Result<u32> {
    match kind {
        "mods" => Ok(6),
        "resourcepacks" => Ok(12),
        "shaderpacks" => Ok(6552),
        other => Err(Error::other(format!("cannot search for {other}"))),
    }
}

fn curseforge_loader_type(loader: &str) -> Option<u32> {
    match loader {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

fn curseforge_key(state: &AppState) -> Result<String> {
    state
        .db
        .load_settings()?
        .curseforge_api_key
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| {
            Error::other(
                "CurseForge needs an API key. Get a free key at console.curseforge.com and add it in Settings.",
            )
        })
}

#[derive(Deserialize)]
struct ModrinthSearch {
    hits: Vec<ModrinthHit>,
}

#[derive(Deserialize)]
struct ModrinthHit {
    project_id: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    author: String,
}

#[derive(Deserialize)]
struct ModrinthVersion {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version_number: String,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    date_published: String,
    #[serde(default)]
    downloads: u64,
    files: Vec<ModrinthFile>,
}

#[derive(Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: ModrinthHashes,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Deserialize, Default)]
struct ModrinthHashes {
    #[serde(default)]
    sha1: Option<String>,
}

#[derive(Deserialize)]
struct CurseforgeSearch {
    data: Vec<CurseforgeMod>,
}

#[derive(Deserialize)]
struct CurseforgeMod {
    id: u64,
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    logo: Option<CurseforgeLogo>,
    #[serde(rename = "downloadCount", default)]
    download_count: f64,
    #[serde(default)]
    authors: Vec<CurseforgeAuthor>,
}

#[derive(Deserialize)]
struct CurseforgeLogo {
    #[serde(rename = "thumbnailUrl", default)]
    thumbnail_url: Option<String>,
}

#[derive(Deserialize)]
struct CurseforgeAuthor {
    name: String,
}

#[derive(Deserialize)]
struct CurseforgeFiles {
    data: Vec<CurseforgeFile>,
}

#[derive(Deserialize)]
struct CurseforgeFile {
    #[serde(default)]
    id: u64,
    #[serde(rename = "displayName", default)]
    display_name: String,
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "fileDate", default)]
    file_date: String,
    #[serde(rename = "downloadCount", default)]
    download_count: f64,
    #[serde(rename = "releaseType", default)]
    release_type: u32,
    #[serde(rename = "downloadUrl", default)]
    download_url: Option<String>,
    #[serde(default)]
    hashes: Vec<CurseforgeHash>,
    #[serde(rename = "fileLength", default)]
    file_length: Option<u64>,
}

#[derive(Deserialize)]
struct CurseforgeFileResponse {
    data: CurseforgeFile,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub channel: String,
    pub date: String,
    pub downloads: u64,
    pub file_name: String,
    pub size: Option<u64>,
}

#[derive(Deserialize)]
struct CurseforgeHash {
    value: String,
    algo: u32,
}

pub async fn search(
    state: &AppState,
    provider: Provider,
    kind: &str,
    query: &str,
    game_version: &str,
    loader: Option<&str>,
) -> Result<Vec<SearchResult>> {
    match provider {
        Provider::Modrinth => {
            let project_type = modrinth_project_type(kind)?;
            let mut facets = vec![
                vec![format!("project_type:{project_type}")],
                vec![format!("versions:{game_version}")],
            ];
            if kind == "mods" {
                if let Some(loader) = loader {
                    facets.push(vec![format!("categories:{loader}")]);
                }
            }
            let response: ModrinthSearch = state
                .http
                .get(format!("{MODRINTH}/search"))
                .query(&[
                    ("query", query),
                    ("facets", &serde_json::to_string(&facets)?),
                    ("limit", "24"),
                    ("index", "relevance"),
                ])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            Ok(response
                .hits
                .into_iter()
                .map(|h| SearchResult {
                    id: h.project_id,
                    title: h.title,
                    description: h.description,
                    icon_url: h.icon_url,
                    downloads: h.downloads,
                    author: h.author,
                })
                .collect())
        }
        Provider::Curseforge => {
            let key = curseforge_key(state)?;
            let class_id = curseforge_class_id(kind)?;
            let mut request = state
                .http
                .get(format!("{CURSEFORGE}/mods/search"))
                .header("x-api-key", key)
                .query(&[
                    ("gameId", CF_GAME_MINECRAFT.to_string()),
                    ("classId", class_id.to_string()),
                    ("searchFilter", query.to_string()),
                    ("gameVersion", game_version.to_string()),
                    ("sortField", "2".to_string()),
                    ("sortOrder", "desc".to_string()),
                    ("pageSize", "24".to_string()),
                ]);
            if kind == "mods" {
                if let Some(loader_type) = loader.and_then(curseforge_loader_type) {
                    request = request.query(&[("modLoaderType", loader_type.to_string())]);
                }
            }
            let response: CurseforgeSearch = request
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            Ok(response
                .data
                .into_iter()
                .map(|m| SearchResult {
                    id: m.id.to_string(),
                    title: m.name,
                    description: m.summary,
                    icon_url: m.logo.and_then(|l| l.thumbnail_url),
                    downloads: m.download_count as u64,
                    author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
                })
                .collect())
        }
    }
}

#[derive(Deserialize)]
struct ModrinthProject {
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    gallery: Vec<ModrinthGalleryItem>,
}

#[derive(Deserialize)]
struct ModrinthGalleryItem {
    url: String,
}

#[derive(Deserialize)]
struct ModrinthMember {
    user: ModrinthUser,
}

#[derive(Deserialize)]
struct ModrinthUser {
    username: String,
}

#[derive(Deserialize)]
struct CurseforgeModResponse {
    data: CurseforgeModDetail,
}

#[derive(Deserialize)]
struct CurseforgeModDetail {
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    logo: Option<CurseforgeLogo>,
    #[serde(rename = "downloadCount", default)]
    download_count: f64,
    #[serde(default)]
    authors: Vec<CurseforgeAuthor>,
    #[serde(default)]
    screenshots: Vec<CurseforgeScreenshot>,
}

#[derive(Deserialize)]
struct CurseforgeScreenshot {
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
struct CurseforgeDescription {
    data: String,
}

pub async fn project_details(
    state: &AppState,
    provider: Provider,
    project_id: &str,
) -> Result<ProjectDetails> {
    match provider {
        Provider::Modrinth => {
            let project: ModrinthProject = state
                .http
                .get(format!("{MODRINTH}/project/{project_id}"))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let author = match state
                .http
                .get(format!("{MODRINTH}/project/{project_id}/members"))
                .send()
                .await
            {
                Ok(resp) => resp
                    .json::<Vec<ModrinthMember>>()
                    .await
                    .ok()
                    .and_then(|members| members.into_iter().next())
                    .map(|m| m.user.username)
                    .unwrap_or_default(),
                Err(_) => String::new(),
            };
            Ok(ProjectDetails {
                id: project_id.to_string(),
                title: project.title,
                description: project.description,
                body: project.body,
                body_format: "markdown".to_string(),
                icon_url: project.icon_url,
                downloads: project.downloads,
                author,
                gallery: project.gallery.into_iter().map(|g| g.url).collect(),
            })
        }
        Provider::Curseforge => {
            let key = curseforge_key(state)?;
            let detail: CurseforgeModResponse = state
                .http
                .get(format!("{CURSEFORGE}/mods/{project_id}"))
                .header("x-api-key", key.clone())
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let body = match state
                .http
                .get(format!("{CURSEFORGE}/mods/{project_id}/description"))
                .header("x-api-key", key)
                .send()
                .await
            {
                Ok(resp) => resp
                    .json::<CurseforgeDescription>()
                    .await
                    .map(|d| d.data)
                    .unwrap_or_default(),
                Err(_) => String::new(),
            };
            let detail = detail.data;
            Ok(ProjectDetails {
                id: project_id.to_string(),
                title: detail.name,
                description: detail.summary,
                body,
                body_format: "html".to_string(),
                icon_url: detail.logo.and_then(|l| l.thumbnail_url),
                downloads: detail.download_count as u64,
                author: detail.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
                gallery: detail
                    .screenshots
                    .into_iter()
                    .filter_map(|s| s.url)
                    .collect(),
            })
        }
    }
}

async fn modrinth_versions(
    state: &AppState,
    project_id: &str,
    game_version: &str,
    loader: Option<&str>,
    kind: &str,
) -> Result<Vec<ModrinthVersion>> {
    let mut request = state
        .http
        .get(format!("{MODRINTH}/project/{project_id}/version"))
        .query(&[("game_versions", format!("[\"{game_version}\"]"))]);
    if kind == "mods" {
        if let Some(loader) = loader {
            request = request.query(&[("loaders", format!("[\"{loader}\"]"))]);
        }
    }
    Ok(request.send().await?.error_for_status()?.json().await?)
}

async fn curseforge_files(
    state: &AppState,
    project_id: &str,
    game_version: &str,
    loader: Option<&str>,
    kind: &str,
) -> Result<Vec<CurseforgeFile>> {
    let key = curseforge_key(state)?;
    let mut request = state
        .http
        .get(format!("{CURSEFORGE}/mods/{project_id}/files"))
        .header("x-api-key", key)
        .query(&[
            ("gameVersion", game_version.to_string()),
            ("pageSize", "50".to_string()),
        ]);
    if kind == "mods" {
        if let Some(loader_type) = loader.and_then(curseforge_loader_type) {
            request = request.query(&[("modLoaderType", loader_type.to_string())]);
        }
    }
    let response: CurseforgeFiles = request.send().await?.error_for_status()?.json().await?;
    Ok(response.data)
}

fn curseforge_channel(release_type: u32) -> &'static str {
    match release_type {
        1 => "release",
        2 => "beta",
        3 => "alpha",
        _ => "release",
    }
}

pub async fn project_versions(
    state: &AppState,
    provider: Provider,
    project_id: &str,
    kind: &str,
    game_version: &str,
    loader: Option<&str>,
) -> Result<Vec<ProjectVersion>> {
    match provider {
        Provider::Modrinth => {
            let versions = modrinth_versions(state, project_id, game_version, loader, kind).await?;
            Ok(versions
                .into_iter()
                .filter_map(|v| {
                    let file = v.files.iter().find(|f| f.primary).or_else(|| v.files.first())?;
                    Some(ProjectVersion {
                        id: v.id.clone(),
                        name: if v.name.is_empty() { v.version_number.clone() } else { v.name.clone() },
                        version_number: v.version_number.clone(),
                        channel: v.version_type.clone(),
                        date: v.date_published.clone(),
                        downloads: v.downloads,
                        file_name: file.filename.clone(),
                        size: file.size,
                    })
                })
                .collect())
        }
        Provider::Curseforge => {
            let files = curseforge_files(state, project_id, game_version, loader, kind).await?;
            Ok(files
                .into_iter()
                .map(|f| ProjectVersion {
                    id: f.id.to_string(),
                    name: if f.display_name.is_empty() { f.file_name.clone() } else { f.display_name.clone() },
                    version_number: f.file_name.clone(),
                    channel: curseforge_channel(f.release_type).to_string(),
                    date: f.file_date.clone(),
                    downloads: f.download_count as u64,
                    file_name: f.file_name.clone(),
                    size: f.file_length,
                })
                .collect())
        }
    }
}

fn modrinth_pick_file(version: &ModrinthVersion) -> Result<(String, String, Option<String>, Option<u64>)> {
    let file = version
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| Error::other("Version has no files."))?;
    Ok((
        file.url.clone(),
        file.filename.clone(),
        file.hashes.sha1.clone(),
        file.size,
    ))
}

fn curseforge_pick_file(file: CurseforgeFile) -> Result<(String, String, Option<String>, Option<u64>)> {
    let sha1 = file
        .hashes
        .iter()
        .find(|h| h.algo == 1)
        .map(|h| h.value.clone());
    let url = file.download_url.ok_or_else(|| {
        Error::other("No downloadable file found. The author may have disabled third-party downloads.")
    })?;
    Ok((url, file.file_name, sha1, file.file_length))
}

pub async fn install(
    state: &AppState,
    provider: Provider,
    project_id: &str,
    instance_id: &str,
    kind: &str,
    game_version: &str,
    loader: Option<&str>,
    version_id: Option<&str>,
) -> Result<String> {
    let dest_dir = content::dir_for(&state.paths, instance_id, kind)?;

    let (url, file_name, sha1, size) = match provider {
        Provider::Modrinth => match version_id {
            Some(vid) => {
                let version: ModrinthVersion = state
                    .http
                    .get(format!("{MODRINTH}/version/{vid}"))
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                modrinth_pick_file(&version)?
            }
            None => {
                let versions =
                    modrinth_versions(state, project_id, game_version, loader, kind).await?;
                let version = versions
                    .into_iter()
                    .next()
                    .ok_or_else(|| Error::other("No compatible version found."))?;
                modrinth_pick_file(&version)?
            }
        },
        Provider::Curseforge => match version_id {
            Some(fid) => {
                let key = curseforge_key(state)?;
                let response: CurseforgeFileResponse = state
                    .http
                    .get(format!("{CURSEFORGE}/mods/{project_id}/files/{fid}"))
                    .header("x-api-key", key)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                curseforge_pick_file(response.data)?
            }
            None => {
                let files = curseforge_files(state, project_id, game_version, loader, kind).await?;
                let file = files
                    .into_iter()
                    .find(|f| f.download_url.is_some())
                    .ok_or_else(|| {
                        Error::other(
                            "No downloadable file found. The author may have disabled third-party downloads.",
                        )
                    })?;
                curseforge_pick_file(file)?
            }
        },
    };

    std::fs::create_dir_all(&dest_dir)?;
    download::download_one(
        &state.http,
        &DownloadSpec {
            url,
            dest: dest_dir.join(&file_name),
            sha1,
            size,
        },
    )
    .await?;
    Ok(file_name)
}
