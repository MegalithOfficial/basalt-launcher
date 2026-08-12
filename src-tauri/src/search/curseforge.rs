use serde::Deserialize;

use super::{cache, model::*};
use crate::{
    error::{Error, Result},
    state::AppState,
};

const API: &str = "https://api.curseforge.com/v1";
const GAME_MINECRAFT: u32 = 432;
const PAGE_SIZE_MAX: u32 = 50;
const SEARCH_INDEX_MAX: u32 = 10_000;

const BUKKIT_PLUGINS: u32 = 5;

pub fn class_for(kind: ContentKind, loaders: &[String]) -> u32 {
    if kind == ContentKind::Mod && loaders.iter().any(|loader| is_plugin_loader(loader)) {
        return BUKKIT_PLUGINS;
    }
    class_id(kind)
}

pub fn class_id(kind: ContentKind) -> u32 {
    match kind {
        ContentKind::Mod => 6,
        ContentKind::ResourcePack => 12,
        ContentKind::Shader => 6552,
        ContentKind::Modpack => 4471,
        ContentKind::DataPack => 6945,
    }
}

fn loader_type(loader: &str) -> Option<u32> {
    match loader.to_lowercase().as_str() {
        "forge" => Some(1),
        "cauldron" => Some(2),
        "liteloader" => Some(3),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

fn loader_name(id: u32) -> Option<&'static str> {
    match id {
        1 => Some("forge"),
        2 => Some("cauldron"),
        3 => Some("liteloader"),
        4 => Some("fabric"),
        5 => Some("quilt"),
        6 => Some("neoforge"),
        _ => None,
    }
}

fn sort_field(sort: SortOrder) -> u32 {
    match sort {
        SortOrder::Relevance => 1,
        SortOrder::Downloads => 6,
        SortOrder::Follows => 2,
        SortOrder::Newest => 11,
        SortOrder::Updated => 3,
    }
}

fn channel(release_type: u32) -> &'static str {
    match release_type {
        2 => "beta",
        3 => "alpha",
        _ => "release",
    }
}

pub fn relation(relation_type: u32) -> Option<&'static str> {
    match relation_type {
        1 => Some("embedded"),
        2 => Some("optional"),
        3 => Some("required"),
        5 => Some("incompatible"),
        _ => None,
    }
}

pub fn key(state: &AppState) -> Result<String> {
    state
        .db
        .load_runtime_settings(&state.credentials)?
        .curseforge_api_key
        .filter(|k| !k.trim().is_empty())
        .or_else(|| crate::build_info::bundled_curseforge_key().map(str::to_string))
        .ok_or_else(|| {
            Error::other(
                "CurseForge needs an API key. Get a free key at console.curseforge.com and add it in Settings.",
            )
        })
}

#[derive(Deserialize)]
struct Paged<T> {
    data: Vec<T>,
    #[serde(default)]
    pagination: Option<Pagination>,
}

#[derive(Deserialize)]
struct Pagination {
    #[serde(rename = "totalCount", default)]
    total_count: u32,
}

#[derive(Deserialize)]
struct Wrapped<T> {
    data: T,
}

#[derive(Deserialize)]
struct Mod {
    id: u64,
    name: String,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    logo: Option<Logo>,
    #[serde(rename = "classId", default)]
    class_id: Option<u32>,
    #[serde(rename = "downloadCount", default)]
    download_count: f64,
    #[serde(rename = "thumbsUpCount", default)]
    thumbs_up_count: u64,
    #[serde(default)]
    authors: Vec<Author>,
    #[serde(default)]
    categories: Vec<Category>,
    #[serde(rename = "latestFilesIndexes", default)]
    latest_files_indexes: Vec<FileIndex>,
    #[serde(rename = "dateModified", default)]
    date_modified: Option<String>,
    #[serde(rename = "dateCreated", default)]
    date_created: Option<String>,
    #[serde(default)]
    screenshots: Vec<Screenshot>,
    #[serde(default)]
    links: Option<Links>,
}

#[derive(Deserialize)]
struct Logo {
    #[serde(rename = "thumbnailUrl", default)]
    thumbnail_url: Option<String>,
}

#[derive(Deserialize)]
struct Author {
    name: String,
}

#[derive(Deserialize)]
struct Category {
    #[serde(default)]
    id: u32,
    name: String,
    #[serde(rename = "classId", default)]
    class_id: Option<u32>,
    #[serde(rename = "isClass", default)]
    is_class: bool,
}

#[derive(Deserialize)]
struct FileIndex {
    #[serde(rename = "gameVersion", default)]
    game_version: String,
    #[serde(rename = "modLoader", default)]
    mod_loader: Option<u32>,
}

#[derive(Deserialize)]
struct Screenshot {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
struct Links {
    #[serde(rename = "websiteUrl", default)]
    website_url: Option<String>,
    #[serde(rename = "wikiUrl", default)]
    wiki_url: Option<String>,
    #[serde(rename = "issuesUrl", default)]
    issues_url: Option<String>,
    #[serde(rename = "sourceUrl", default)]
    source_url: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct File {
    #[serde(default)]
    pub id: u64,
    #[serde(rename = "modId", default)]
    pub mod_id: u64,
    #[serde(rename = "displayName", default)]
    pub display_name: String,
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "fileDate", default)]
    pub file_date: String,
    #[serde(rename = "downloadCount", default)]
    pub download_count: f64,
    #[serde(rename = "releaseType", default)]
    pub release_type: u32,
    #[serde(rename = "downloadUrl", default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub hashes: Vec<Hash>,
    #[serde(rename = "fileLength", default)]
    pub file_length: Option<u64>,
    #[serde(rename = "alternateFileId", default)]
    pub alternate_file_id: u64,
    #[serde(rename = "parentProjectFileId", default)]
    pub parent_project_file_id: Option<u64>,
    #[serde(rename = "gameVersions", default)]
    pub game_versions: Vec<String>,
    #[serde(rename = "sortableGameVersions", default)]
    pub sortable_game_versions: Vec<SortableGameVersion>,
    #[serde(default)]
    pub dependencies: Vec<FileDependency>,
}

#[derive(Deserialize, Clone)]
pub struct SortableGameVersion {
    #[serde(rename = "gameVersionName", default)]
    pub name: String,
    #[serde(rename = "gameVersionTypeId", default)]
    pub type_id: Option<u32>,
}

#[derive(Deserialize, Clone)]
pub struct Hash {
    pub value: String,
    pub algo: u32,
}

#[derive(Deserialize, Clone)]
pub struct FileDependency {
    #[serde(rename = "modId")]
    pub mod_id: u64,
    #[serde(rename = "relationType", default)]
    pub relation_type: u32,
}

fn request(state: &AppState, url: String, api_key: &str) -> reqwest::RequestBuilder {
    state.network.get(url).header("x-api-key", api_key)
}

fn summary(item: Mod) -> ProjectSummary {
    let mut game_versions: Vec<String> = item
        .latest_files_indexes
        .iter()
        .map(|i| i.game_version.clone())
        .filter(|v| !v.is_empty())
        .collect();
    game_versions.sort();
    game_versions.dedup();

    let mut loaders: Vec<String> = item
        .latest_files_indexes
        .iter()
        .filter_map(|i| i.mod_loader)
        .filter_map(loader_name)
        .map(str::to_owned)
        .collect();
    loaders.sort();
    loaders.dedup();

    ProjectSummary {
        id: item.id.to_string(),
        slug: item.slug,
        title: item.name,
        description: item.summary,
        icon_url: item.logo.and_then(|l| l.thumbnail_url),
        downloads: item.download_count as u64,
        follows: item.thumbs_up_count,
        author: item
            .authors
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_default(),
        categories: item
            .categories
            .into_iter()
            .filter(|c| !c.is_class)
            .map(|c| c.name)
            .collect(),
        game_versions,
        loaders,
        updated: item.date_modified,
        color: None,
    }
}

pub async fn search(
    state: &AppState,
    kind: ContentKind,
    query: &SearchQuery,
) -> Result<SearchPage> {
    let api_key = key(state)?;
    let limit = query.limit.min(PAGE_SIZE_MAX);
    let index = query.offset.min(SEARCH_INDEX_MAX.saturating_sub(limit));

    let mut params: Vec<(String, String)> = vec![
        ("gameId".into(), GAME_MINECRAFT.to_string()),
        (
            "classId".into(),
            class_for(kind, &query.loaders).to_string(),
        ),
        ("searchFilter".into(), query.query.clone()),
        ("sortField".into(), sort_field(query.sort).to_string()),
        ("sortOrder".into(), "desc".into()),
        ("pageSize".into(), limit.to_string()),
        ("index".into(), index.to_string()),
    ];
    if let Some(game_version) = query.game_versions.first() {
        params.push(("gameVersion".into(), game_version.clone()));
    }
    if kind.uses_loaders() {
        if let Some(id) = query.loaders.first().and_then(|l| loader_type(l)) {
            params.push(("modLoaderType".into(), id.to_string()));
        }
    }
    if let Some(category) = query.categories.first() {
        if let Ok(id) = category.parse::<u32>() {
            params.push(("categoryId".into(), id.to_string()));
        }
    }

    let cache_key = format!("cf:search:{}:{:?}", kind.as_str(), params);
    let response: Paged<Mod> = cache::fetch(
        state,
        &cache_key,
        cache::TTL_SEARCH,
        request(state, format!("{API}/mods/search"), &api_key).query(&params),
    )
    .await?;

    let total = response
        .pagination
        .as_ref()
        .map(|p| p.total_count.min(SEARCH_INDEX_MAX))
        .unwrap_or(response.data.len() as u32);

    Ok(SearchPage {
        hits: response.data.into_iter().map(summary).collect(),
        total,
        offset: index,
        limit,
    })
}

pub async fn project_details(state: &AppState, project_id: &str) -> Result<ProjectDetails> {
    let api_key = key(state)?;
    let detail: Wrapped<Mod> = cache::fetch(
        state,
        &format!("cf:project:{project_id}"),
        cache::TTL_PROJECT,
        request(state, format!("{API}/mods/{project_id}"), &api_key),
    )
    .await?;
    let detail = detail.data;

    let body = cache::fetch::<Wrapped<String>>(
        state,
        &format!("cf:body:{project_id}"),
        cache::TTL_PROJECT,
        request(
            state,
            format!("{API}/mods/{project_id}/description"),
            &api_key,
        ),
    )
    .await
    .map(|d| d.data)
    .unwrap_or_default();

    let website_url = detail.links.as_ref().and_then(|l| l.website_url.clone());
    let mut links = Vec::new();
    if let Some(l) = &detail.links {
        for (label, url) in [
            ("Report issues", &l.issues_url),
            ("View source", &l.source_url),
            ("Visit wiki", &l.wiki_url),
            ("Website", &l.website_url),
        ] {
            if let Some(url) = url.as_ref().filter(|u| !u.is_empty()) {
                links.push(ProjectLink {
                    label: label.to_string(),
                    url: url.clone(),
                });
            }
        }
    }

    let gallery = detail
        .screenshots
        .iter()
        .filter_map(|s| {
            s.url.clone().map(|url| GalleryImage {
                raw_url: None,
                url,
                title: s.title.clone(),
                description: s.description.clone(),
                featured: false,
            })
        })
        .collect();

    let published = detail.date_created.clone();
    let updated = detail.date_modified.clone();
    let summary = summary(detail);

    Ok(ProjectDetails {
        id: summary.id,
        slug: summary.slug,
        title: summary.title,
        description: summary.description,
        body,
        body_format: "html".to_string(),
        icon_url: summary.icon_url,
        downloads: summary.downloads,
        follows: summary.follows,
        author: summary.author,
        gallery,
        game_versions: summary.game_versions,
        loaders: summary.loaders,
        client_side: None,
        server_side: None,
        categories: summary.categories,
        license: None,
        links,
        published,
        updated,
        website_url,
        color: None,
    })
}

fn split_game_versions(file: &File) -> (Vec<String>, Vec<String>) {
    let mut game_versions = Vec::new();
    let mut loaders = Vec::new();

    if !file.sortable_game_versions.is_empty() {
        for entry in &file.sortable_game_versions {
            match entry.type_id {
                Some(68441) => loaders.push(entry.name.to_lowercase()),
                _ if looks_like_game_version(&entry.name) => game_versions.push(entry.name.clone()),
                _ if is_loader_token(&entry.name) => loaders.push(entry.name.to_lowercase()),
                _ => {}
            }
        }
    } else {
        for entry in &file.game_versions {
            if looks_like_game_version(entry) {
                game_versions.push(entry.clone());
            } else if is_loader_token(entry) {
                loaders.push(entry.to_lowercase());
            }
        }
    }

    game_versions.sort();
    game_versions.dedup();
    loaders.sort();
    loaders.dedup();
    (game_versions, loaders)
}

pub fn to_version(
    file: File,
    project_id: &str,
    game_version: &str,
    loader: Option<&str>,
    kind: ContentKind,
) -> ProjectVersion {
    let (game_versions, loaders) = split_game_versions(&file);
    let sha1 = file
        .hashes
        .iter()
        .find(|h| h.algo == 1)
        .map(|h| h.value.clone());

    let mut version = ProjectVersion {
        id: file.id.to_string(),
        project_id: project_id.to_string(),
        name: if file.display_name.is_empty() {
            file.file_name.clone()
        } else {
            file.display_name.clone()
        },
        version_number: file.file_name.clone(),
        channel: channel(file.release_type).to_string(),
        date: file.file_date.clone(),
        downloads: file.download_count as u64,
        file_name: file.file_name.clone(),
        size: file.file_length,
        game_versions,
        loaders,
        compatible: false,
        changelog: None,
        dependencies: file
            .dependencies
            .iter()
            .filter_map(|d| {
                relation(d.relation_type).map(|t| VersionDependency {
                    project_id: d.mod_id.to_string(),
                    version_id: None,
                    dependency_type: t.to_string(),
                })
            })
            .collect(),
        files: vec![VersionFile {
            url: file.download_url.clone(),
            file_name: file.file_name.clone(),
            sha1,
            sha512: None,
            size: file.file_length,
            primary: true,
        }],
        server_pack_file_id: (file.alternate_file_id != 0)
            .then(|| file.alternate_file_id.to_string()),
    };
    version.compatible = version.matches(game_version, loader, kind);
    version
}

pub async fn project_versions(
    state: &AppState,
    project_id: &str,
    kind: ContentKind,
    game_version: &str,
    loader: Option<&str>,
) -> Result<Vec<ProjectVersion>> {
    let api_key = key(state)?;
    let mut all = Vec::new();
    let mut index = 0u32;

    loop {
        let params = [
            ("pageSize".to_string(), PAGE_SIZE_MAX.to_string()),
            ("index".to_string(), index.to_string()),
        ];
        let page: Paged<File> = cache::fetch(
            state,
            &format!("cf:files:{project_id}:{index}"),
            cache::TTL_VERSIONS,
            request(state, format!("{API}/mods/{project_id}/files"), &api_key).query(&params),
        )
        .await?;

        let received = page.data.len() as u32;
        all.extend(page.data);
        index += received;

        let total = page.pagination.map(|p| p.total_count).unwrap_or(index);
        if received == 0 || index >= total || index >= 1000 {
            break;
        }
    }

    Ok(all
        .into_iter()
        .map(|f| to_version(f, project_id, game_version, loader, kind))
        .collect())
}

pub async fn server_pack(
    state: &AppState,
    project_id: &str,
    file_id: &str,
    parent_id: &str,
) -> Result<VersionFile> {
    let file = version(state, project_id, file_id).await?;
    let parent = parent_id.parse::<u64>().ok();
    if file.parent_project_file_id.is_some() && file.parent_project_file_id != parent {
        return Err(Error::other(
            "That file is not the server pack for this version.",
        ));
    }
    Ok(VersionFile {
        url: file.download_url.clone(),
        file_name: file.file_name.clone(),
        sha1: file
            .hashes
            .iter()
            .find(|hash| hash.algo == 1)
            .map(|hash| hash.value.clone()),
        sha512: None,
        size: file.file_length,
        primary: false,
    })
}

pub async fn version(state: &AppState, project_id: &str, version_id: &str) -> Result<File> {
    let api_key = key(state)?;
    let response: Wrapped<File> = cache::fetch(
        state,
        &format!("cf:file:{project_id}:{version_id}"),
        cache::TTL_VERSIONS,
        request(
            state,
            format!("{API}/mods/{project_id}/files/{version_id}"),
            &api_key,
        ),
    )
    .await?;
    Ok(response.data)
}

pub async fn files(state: &AppState, file_ids: &[i64]) -> Result<Vec<File>> {
    if file_ids.is_empty() {
        return Ok(Vec::new());
    }
    let api_key = key(state)?;
    let mut collected = Vec::with_capacity(file_ids.len());
    for chunk in file_ids.chunks(200) {
        let response: Paged<File> = cache::post(
            state,
            state
                .network
                .post(format!("{API}/mods/files"))
                .header("x-api-key", &api_key)
                .json(&serde_json::json!({ "fileIds": chunk })),
        )
        .await?;
        collected.extend(response.data);
    }
    Ok(collected)
}

async fn projects_by_ids(state: &AppState, ids: &[String]) -> Result<Vec<Mod>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let api_key = key(state)?;
    let mod_ids: Vec<u64> = ids.iter().filter_map(|id| id.parse().ok()).collect();
    let mut collected = Vec::with_capacity(mod_ids.len());
    for chunk in mod_ids.chunks(50) {
        let response: Paged<Mod> = cache::post(
            state,
            state
                .network
                .post(format!("{API}/mods"))
                .header("x-api-key", &api_key)
                .json(&serde_json::json!({ "modIds": chunk })),
        )
        .await?;
        collected.extend(response.data);
    }
    Ok(collected)
}

pub async fn project_classes(
    state: &AppState,
    ids: &[String],
) -> Result<std::collections::HashMap<String, u32>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    Ok(projects_by_ids(state, ids)
        .await?
        .into_iter()
        .filter_map(|item| item.class_id.map(|class| (item.id.to_string(), class)))
        .collect())
}

pub async fn project_download_pages(
    state: &AppState,
    ids: &[String],
) -> Result<std::collections::HashMap<String, String>> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    Ok(projects_by_ids(state, ids)
        .await?
        .into_iter()
        .filter_map(|item| {
            item.links
                .and_then(|links| links.website_url)
                .map(|url| (item.id.to_string(), url))
        })
        .collect())
}

pub async fn resolve_projects(state: &AppState, ids: &[String]) -> Result<Vec<ProjectSummary>> {
    Ok(projects_by_ids(state, ids)
        .await?
        .into_iter()
        .map(summary)
        .collect())
}

#[derive(Deserialize)]
struct FingerprintMatches {
    #[serde(rename = "exactMatches", default)]
    exact_matches: Vec<FingerprintMatch>,
}

#[derive(Deserialize)]
pub struct FingerprintMatch {
    pub id: u64,
    pub file: File,
}

pub async fn match_fingerprints(
    state: &AppState,
    fingerprints: &[u32],
) -> Result<Vec<FingerprintMatch>> {
    if fingerprints.is_empty() {
        return Ok(Vec::new());
    }
    let api_key = key(state)?;
    let response: Wrapped<FingerprintMatches> = cache::post(
        state,
        state
            .network
            .post(format!("{API}/fingerprints"))
            .header("x-api-key", api_key)
            .json(&serde_json::json!({ "fingerprints": fingerprints })),
    )
    .await?;
    Ok(response.data.exact_matches)
}

pub async fn changelog(state: &AppState, project_id: &str, version_id: &str) -> Result<Changelog> {
    let api_key = key(state)?;
    let response: Wrapped<String> = cache::fetch(
        state,
        &format!("cf:changelog:{project_id}:{version_id}"),
        cache::TTL_VERSIONS,
        request(
            state,
            format!("{API}/mods/{project_id}/files/{version_id}/changelog"),
            &api_key,
        ),
    )
    .await?;
    Ok(Changelog {
        body: response.data,
        format: "html".to_string(),
    })
}

pub async fn taxonomy(state: &AppState, kind: ContentKind) -> Result<FilterTaxonomy> {
    let api_key = key(state)?;
    let class = class_id(kind);
    let response: Paged<Category> = cache::fetch(
        state,
        &format!("cf:categories:{class}"),
        cache::TTL_TAGS,
        request(state, format!("{API}/categories"), &api_key).query(&[
            ("gameId", GAME_MINECRAFT.to_string()),
            ("classId", class.to_string()),
        ]),
    )
    .await?;

    let categories = response
        .data
        .into_iter()
        .filter(|c| !c.is_class && c.class_id == Some(class))
        .map(|c| FilterOption {
            id: c.id.to_string(),
            name: c.name,
            group: "Categories".to_string(),
        })
        .collect();

    let loaders = if kind.uses_loaders() {
        ["forge", "fabric", "neoforge", "quilt"]
            .iter()
            .map(|l| FilterOption {
                id: l.to_string(),
                name: l.to_string(),
                group: "Loaders".to_string(),
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(FilterTaxonomy {
        categories,
        loaders,
        game_versions: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::{split_game_versions, File, SortableGameVersion};

    fn file_with(game_versions: &[&str]) -> File {
        File {
            alternate_file_id: 0,
            parent_project_file_id: None,
            id: 1,
            mod_id: 1,
            display_name: String::new(),
            file_name: "a.jar".into(),
            file_date: String::new(),
            download_count: 0.0,
            release_type: 1,
            download_url: None,
            hashes: Vec::new(),
            file_length: None,
            game_versions: game_versions.iter().map(|s| s.to_string()).collect(),
            sortable_game_versions: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn noise_tokens_are_not_treated_as_loaders() {
        let file = file_with(&["1.20.1", "Fabric", "Client", "Java 17", "Server"]);
        let (versions, loaders) = split_game_versions(&file);
        assert_eq!(versions, vec!["1.20.1"]);
        assert_eq!(loaders, vec!["fabric"]);
    }

    #[test]
    fn sortable_versions_take_priority() {
        let mut file = file_with(&["ignored"]);
        file.sortable_game_versions = vec![
            SortableGameVersion {
                name: "1.21.1".into(),
                type_id: Some(75125),
            },
            SortableGameVersion {
                name: "NeoForge".into(),
                type_id: Some(68441),
            },
            SortableGameVersion {
                name: "Client".into(),
                type_id: Some(75208),
            },
        ];
        let (versions, loaders) = split_game_versions(&file);
        assert_eq!(versions, vec!["1.21.1"]);
        assert_eq!(loaders, vec!["neoforge"]);
    }
}
