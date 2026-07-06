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
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Modrinth => "modrinth",
            Provider::Curseforge => "curseforge",
        }
    }

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
pub struct ProjectLink {
    pub label: String,
    pub url: String,
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
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub client_side: Option<String>,
    pub server_side: Option<String>,
    pub categories: Vec<String>,
    pub license: Option<String>,
    pub links: Vec<ProjectLink>,
    pub published: Option<String>,
    pub updated: Option<String>,
    pub website_url: Option<String>,
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
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    changelog: Option<String>,
    #[serde(default)]
    dependencies: Vec<ModrinthDependency>,
    files: Vec<ModrinthFile>,
}

#[derive(Deserialize, Clone)]
struct ModrinthDependency {
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    dependency_type: String,
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
    #[serde(rename = "gameVersions", default)]
    game_versions: Vec<String>,
    #[serde(default)]
    dependencies: Vec<CurseforgeFileDependency>,
}

#[derive(Deserialize, Clone)]
struct CurseforgeFileDependency {
    #[serde(rename = "modId")]
    mod_id: u64,
    #[serde(rename = "relationType", default)]
    relation_type: u32,
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
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub compatible: bool,
    pub changelog: Option<String>,
    pub dependencies: Vec<VersionDependency>,
}

fn modrinth_deps(deps: &[ModrinthDependency]) -> Vec<VersionDependency> {
    deps.iter()
        .filter_map(|d| {
            d.project_id.clone().map(|project_id| VersionDependency {
                project_id,
                dependency_type: d.dependency_type.clone(),
            })
        })
        .collect()
}

fn curseforge_deps(deps: &[CurseforgeFileDependency]) -> Vec<VersionDependency> {
    deps.iter()
        .filter_map(|d| {
            curseforge_relation(d.relation_type).map(|t| VersionDependency {
                project_id: d.mod_id.to_string(),
                dependency_type: t.to_string(),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct Changelog {
    pub body: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionDependency {
    pub project_id: String,
    pub dependency_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstalledFile {
    pub version_id: Option<String>,
    pub file_name: String,
}

fn curseforge_relation(relation_type: u32) -> Option<&'static str> {
    match relation_type {
        1 => Some("embedded"),
        2 => Some("optional"),
        3 => Some("required"),
        5 => Some("incompatible"),
        _ => None,
    }
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
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    client_side: Option<String>,
    #[serde(default)]
    server_side: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    license: Option<ModrinthLicense>,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    issues_url: Option<String>,
    #[serde(default)]
    wiki_url: Option<String>,
    #[serde(default)]
    discord_url: Option<String>,
    #[serde(default)]
    published: Option<String>,
    #[serde(default)]
    updated: Option<String>,
}

#[derive(Deserialize)]
struct ModrinthLicense {
    #[serde(default)]
    id: String,
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
    #[serde(default)]
    links: Option<CurseforgeLinks>,
    #[serde(default)]
    categories: Vec<CurseforgeCategory>,
    #[serde(rename = "latestFilesIndexes", default)]
    latest_files_indexes: Vec<CurseforgeFileIndex>,
    #[serde(rename = "dateCreated", default)]
    date_created: Option<String>,
    #[serde(rename = "dateModified", default)]
    date_modified: Option<String>,
}

#[derive(Deserialize)]
struct CurseforgeLinks {
    #[serde(rename = "websiteUrl", default)]
    website_url: Option<String>,
    #[serde(rename = "wikiUrl", default)]
    wiki_url: Option<String>,
    #[serde(rename = "issuesUrl", default)]
    issues_url: Option<String>,
    #[serde(rename = "sourceUrl", default)]
    source_url: Option<String>,
}

#[derive(Deserialize)]
struct CurseforgeCategory {
    name: String,
}

#[derive(Deserialize)]
struct CurseforgeFileIndex {
    #[serde(rename = "gameVersion", default)]
    game_version: String,
    #[serde(rename = "modLoader", default)]
    mod_loader: Option<u32>,
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
            let mut links = Vec::new();
            for (label, url) in [
                ("Report issues", &project.issues_url),
                ("View source", &project.source_url),
                ("Visit wiki", &project.wiki_url),
                ("Join Discord", &project.discord_url),
            ] {
                if let Some(url) = url {
                    if !url.is_empty() {
                        links.push(ProjectLink {
                            label: label.to_string(),
                            url: url.clone(),
                        });
                    }
                }
            }
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
                game_versions: project.game_versions,
                loaders: project.loaders,
                client_side: project.client_side,
                server_side: project.server_side,
                categories: project.categories,
                license: project.license.map(|l| l.id).filter(|id| !id.is_empty()),
                links,
                published: project.published,
                updated: project.updated,
                website_url: Some(format!("https://modrinth.com/project/{project_id}")),
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
            let mut game_versions: Vec<String> = detail
                .latest_files_indexes
                .iter()
                .map(|i| i.game_version.clone())
                .collect();
            game_versions.dedup();
            game_versions.truncate(24);
            let mut loaders: Vec<String> = detail
                .latest_files_indexes
                .iter()
                .filter_map(|i| i.mod_loader)
                .filter_map(|l| match l {
                    1 => Some("forge".to_string()),
                    4 => Some("fabric".to_string()),
                    5 => Some("quilt".to_string()),
                    6 => Some("neoforge".to_string()),
                    _ => None,
                })
                .collect();
            loaders.sort();
            loaders.dedup();
            let website_url = detail.links.as_ref().and_then(|l| l.website_url.clone());
            let mut links = Vec::new();
            if let Some(l) = &detail.links {
                for (label, url) in [
                    ("Report issues", &l.issues_url),
                    ("View source", &l.source_url),
                    ("Visit wiki", &l.wiki_url),
                    ("Website", &l.website_url),
                ] {
                    if let Some(url) = url {
                        if !url.is_empty() {
                            links.push(ProjectLink {
                                label: label.to_string(),
                                url: url.clone(),
                            });
                        }
                    }
                }
            }
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
                game_versions,
                loaders,
                client_side: None,
                server_side: None,
                categories: detail.categories.into_iter().map(|c| c.name).collect(),
                license: None,
                links,
                published: detail.date_created,
                updated: detail.date_modified,
                website_url,
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

fn curseforge_loader_name(loader: &str) -> &'static str {
    match loader {
        "forge" => "Forge",
        "fabric" => "Fabric",
        "quilt" => "Quilt",
        "neoforge" => "NeoForge",
        _ => "",
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
            let versions: Vec<ModrinthVersion> = state
                .http
                .get(format!("{MODRINTH}/project/{project_id}/version"))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            Ok(versions
                .into_iter()
                .filter_map(|v| {
                    let file = v.files.iter().find(|f| f.primary).or_else(|| v.files.first())?;
                    let loader_ok = kind != "mods"
                        || loader.map_or(true, |l| v.loaders.iter().any(|x| x == l));
                    let compatible =
                        v.game_versions.iter().any(|g| g == game_version) && loader_ok;
                    Some(ProjectVersion {
                        id: v.id.clone(),
                        name: if v.name.is_empty() { v.version_number.clone() } else { v.name.clone() },
                        version_number: v.version_number.clone(),
                        channel: v.version_type.clone(),
                        date: v.date_published.clone(),
                        downloads: v.downloads,
                        file_name: file.filename.clone(),
                        size: file.size,
                        game_versions: v.game_versions.clone(),
                        loaders: v.loaders.clone(),
                        compatible,
                        changelog: v.changelog.clone().filter(|c| !c.trim().is_empty()),
                        dependencies: modrinth_deps(&v.dependencies),
                    })
                })
                .collect())
        }
        Provider::Curseforge => {
            let key = curseforge_key(state)?;
            let response: CurseforgeFiles = state
                .http
                .get(format!("{CURSEFORGE}/mods/{project_id}/files"))
                .header("x-api-key", key)
                .query(&[("pageSize", "50".to_string())])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let loader_name = loader.map(curseforge_loader_name).unwrap_or("");
            Ok(response
                .data
                .into_iter()
                .map(|f| {
                    let loader_ok = kind != "mods"
                        || loader_name.is_empty()
                        || f.game_versions.iter().any(|g| g == loader_name);
                    let compatible =
                        f.game_versions.iter().any(|g| g == game_version) && loader_ok;
                    ProjectVersion {
                        id: f.id.to_string(),
                        name: if f.display_name.is_empty() { f.file_name.clone() } else { f.display_name.clone() },
                        version_number: f.file_name.clone(),
                        channel: curseforge_channel(f.release_type).to_string(),
                        date: f.file_date.clone(),
                        downloads: f.download_count as u64,
                        file_name: f.file_name.clone(),
                        size: f.file_length,
                        game_versions: f
                            .game_versions
                            .iter()
                            .filter(|g| g.chars().next().is_some_and(|c| c.is_ascii_digit()))
                            .cloned()
                            .collect(),
                        loaders: f
                            .game_versions
                            .iter()
                            .filter(|g| g.chars().next().is_some_and(|c| c.is_ascii_alphabetic()))
                            .map(|g| g.to_lowercase())
                            .collect(),
                        compatible,
                        changelog: None,
                        dependencies: curseforge_deps(&f.dependencies),
                    }
                })
                .collect())
        }
    }
}

#[derive(Deserialize)]
struct ModrinthProjectListItem {
    id: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    downloads: u64,
}

pub async fn resolve_projects(
    state: &AppState,
    provider: Provider,
    ids: Vec<String>,
) -> Result<Vec<SearchResult>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    match provider {
        Provider::Modrinth => {
            let projects: Vec<ModrinthProjectListItem> = state
                .http
                .get(format!("{MODRINTH}/projects"))
                .query(&[("ids", serde_json::to_string(&ids)?)])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            Ok(projects
                .into_iter()
                .map(|p| SearchResult {
                    id: p.id,
                    title: p.title,
                    description: p.description,
                    icon_url: p.icon_url,
                    downloads: p.downloads,
                    author: String::new(),
                })
                .collect())
        }
        Provider::Curseforge => {
            let key = curseforge_key(state)?;
            let mod_ids: Vec<u64> = ids.iter().filter_map(|i| i.parse().ok()).collect();
            let response: CurseforgeSearch = state
                .http
                .post(format!("{CURSEFORGE}/mods"))
                .header("x-api-key", key)
                .json(&serde_json::json!({ "modIds": mod_ids }))
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

pub async fn version_changelog(
    state: &AppState,
    provider: Provider,
    project_id: &str,
    version_id: &str,
) -> Result<Changelog> {
    match provider {
        Provider::Modrinth => {
            let version: ModrinthVersion = state
                .http
                .get(format!("{MODRINTH}/version/{version_id}"))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            Ok(Changelog {
                body: version.changelog.unwrap_or_default(),
                format: "markdown".to_string(),
            })
        }
        Provider::Curseforge => {
            let key = curseforge_key(state)?;
            let response: CurseforgeDescription = state
                .http
                .get(format!(
                    "{CURSEFORGE}/mods/{project_id}/files/{version_id}/changelog"
                ))
                .header("x-api-key", key)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            Ok(Changelog {
                body: response.data,
                format: "html".to_string(),
            })
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

async fn install_single(
    state: &AppState,
    provider: Provider,
    project_id: &str,
    instance_id: &str,
    kind: &str,
    game_version: &str,
    loader: Option<&str>,
    version_id: Option<&str>,
    title: Option<&str>,
    icon_url: Option<&str>,
) -> Result<(String, Vec<VersionDependency>)> {
    let dest_dir = content::dir_for(&state.paths, instance_id, kind)?;

    let (url, file_name, sha1, size, source_version, dependencies) = match provider {
        Provider::Modrinth => {
            let version: ModrinthVersion = match version_id {
                Some(vid) => {
                    state
                        .http
                        .get(format!("{MODRINTH}/version/{vid}"))
                        .send()
                        .await?
                        .error_for_status()?
                        .json()
                        .await?
                }
                None => modrinth_versions(state, project_id, game_version, loader, kind)
                    .await?
                    .into_iter()
                    .next()
                    .ok_or_else(|| Error::other("No compatible version found."))?,
            };
            let (url, file_name, sha1, size) = modrinth_pick_file(&version)?;
            let deps = modrinth_deps(&version.dependencies);
            (url, file_name, sha1, size, version.id, deps)
        }
        Provider::Curseforge => {
            let file = match version_id {
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
                    response.data
                }
                None => curseforge_files(state, project_id, game_version, loader, kind)
                    .await?
                    .into_iter()
                    .find(|f| f.download_url.is_some())
                    .ok_or_else(|| {
                        Error::other(
                            "No downloadable file found. The author may have disabled third-party downloads.",
                        )
                    })?,
            };
            let source_version = file.id.to_string();
            let deps = curseforge_deps(&file.dependencies);
            let (url, file_name, sha1, size) = curseforge_pick_file(file)?;
            (url, file_name, sha1, size, source_version, deps)
        }
    };

    if let Some((_, old_file)) = state.db.installed_project_file(instance_id, kind, project_id)? {
        if old_file != file_name {
            let _ = content::delete(&state.paths, instance_id, kind, &old_file);
            let _ = state.db.delete_content_source(instance_id, kind, &old_file);
        }
    }

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

    let _ = state.db.record_content_source(
        instance_id,
        kind,
        &file_name,
        provider.as_str(),
        project_id,
        Some(&source_version).filter(|v| !v.is_empty()).map(|v| v.as_str()),
        title,
        icon_url,
    );

    Ok((file_name, dependencies))
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
    title: Option<&str>,
    icon_url: Option<&str>,
) -> Result<Vec<String>> {
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    visited.insert(project_id.to_string());

    let (file_name, dependencies) = install_single(
        state, provider, project_id, instance_id, kind, game_version, loader, version_id,
        title, icon_url,
    )
    .await?;
    let mut installed_files = vec![file_name];

    if kind == "mods" {
        let mut queue: Vec<(String, u8)> = dependencies
            .into_iter()
            .filter(|d| d.dependency_type == "required")
            .map(|d| (d.project_id, 1u8))
            .collect();

        while let Some((dep_id, depth)) = queue.pop() {
            if depth > 3 || !visited.insert(dep_id.clone()) {
                continue;
            }
            if state
                .db
                .installed_project_file(instance_id, kind, &dep_id)?
                .is_some()
            {
                continue;
            }
            let info = resolve_projects(state, provider, vec![dep_id.clone()])
                .await
                .ok()
                .and_then(|mut list| list.pop());
            match Box::pin(install_single(
                state,
                provider,
                &dep_id,
                instance_id,
                kind,
                game_version,
                loader,
                None,
                info.as_ref().map(|i| i.title.as_str()),
                info.as_ref().and_then(|i| i.icon_url.as_deref()),
            ))
            .await
            {
                Ok((dep_file, dep_deps)) => {
                    installed_files.push(dep_file);
                    queue.extend(
                        dep_deps
                            .into_iter()
                            .filter(|d| d.dependency_type == "required")
                            .map(|d| (d.project_id, depth + 1)),
                    );
                }
                Err(_) => continue,
            }
        }
    }

    Ok(installed_files)
}

#[cfg(test)]
mod tests {
    use super::curseforge_relation;

    #[test]
    fn relation_types_map() {
        assert_eq!(curseforge_relation(3), Some("required"));
        assert_eq!(curseforge_relation(2), Some("optional"));
        assert_eq!(curseforge_relation(1), Some("embedded"));
        assert_eq!(curseforge_relation(5), Some("incompatible"));
        assert_eq!(curseforge_relation(4), None);
    }
}
