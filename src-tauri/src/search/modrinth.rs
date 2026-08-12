use std::collections::HashMap;

use serde::Deserialize;

use super::{cache, model::*};
use crate::{error::Result, state::AppState};

pub const API: &str = "https://api.modrinth.com/v2";

pub fn project_type(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::Mod => "mod",
        ContentKind::Modpack => "modpack",
        ContentKind::ResourcePack => "resourcepack",
        ContentKind::Shader => "shader",
        ContentKind::DataPack => "datapack",
    }
}

fn sort_index(sort: SortOrder) -> &'static str {
    match sort {
        SortOrder::Relevance => "relevance",
        SortOrder::Downloads => "downloads",
        SortOrder::Follows => "follows",
        SortOrder::Newest => "newest",
        SortOrder::Updated => "updated",
    }
}

fn build_facets(kind: ContentKind, query: &SearchQuery) -> Vec<Vec<String>> {
    let mut facets = vec![vec![format!("project_type:{}", project_type(kind))]];

    if !query.game_versions.is_empty() {
        facets.push(
            query
                .game_versions
                .iter()
                .map(|v| format!("versions:{v}"))
                .collect(),
        );
    }
    if kind.uses_loaders() && !query.loaders.is_empty() {
        facets.push(
            query
                .loaders
                .iter()
                .map(|l| format!("categories:{l}"))
                .collect(),
        );
    }
    for category in &query.categories {
        facets.push(vec![format!("categories:{category}")]);
    }
    match query.environment {
        Some(Environment::Client) => facets.push(vec!["client_side:required".to_string()]),
        Some(Environment::Server)
            if !query.loaders.iter().any(|loader| is_plugin_loader(loader)) =>
        {
            facets.push(vec![
                "server_side:required".to_string(),
                "server_side:optional".to_string(),
            ]);
        }
        Some(Environment::Server) => {}
        None => {}
    }
    if query.open_source_only {
        facets.push(vec!["open_source:true".to_string()]);
    }
    facets
}

#[derive(Deserialize)]
struct SearchResponse {
    hits: Vec<Hit>,
    #[serde(default)]
    total_hits: u32,
    #[serde(default)]
    offset: u32,
}

#[derive(Deserialize)]
struct Hit {
    project_id: String,
    #[serde(default)]
    slug: Option<String>,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    follows: u64,
    #[serde(default)]
    author: String,
    #[serde(default)]
    display_categories: Vec<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    versions: Vec<String>,
    #[serde(default)]
    date_modified: Option<String>,
    #[serde(default)]
    color: Option<u32>,
}

fn split_categories(categories: Vec<String>) -> (Vec<String>, Vec<String>) {
    let (loaders, tags): (Vec<String>, Vec<String>) =
        categories.into_iter().partition(|c| is_loader_token(c));
    (tags, loaders)
}

fn summary(hit: Hit) -> ProjectSummary {
    let display = if hit.display_categories.is_empty() {
        hit.categories.clone()
    } else {
        hit.display_categories.clone()
    };
    let (categories, _) = split_categories(display);
    let (_, loaders) = split_categories(hit.categories);

    ProjectSummary {
        id: hit.project_id,
        slug: hit.slug,
        title: hit.title,
        description: hit.description,
        icon_url: hit.icon_url,
        downloads: hit.downloads,
        follows: hit.follows,
        author: hit.author,
        categories,
        game_versions: hit.versions,
        loaders,
        updated: hit.date_modified,
        color: hit.color,
    }
}

pub async fn search(
    state: &AppState,
    kind: ContentKind,
    query: &SearchQuery,
) -> Result<SearchPage> {
    let facets = build_facets(kind, query);
    let limit = query.limit.clamp(1, 100);
    let params = [
        ("query".to_string(), query.query.clone()),
        ("facets".to_string(), serde_json::to_string(&facets)?),
        ("index".to_string(), sort_index(query.sort).to_string()),
        ("offset".to_string(), query.offset.to_string()),
        ("limit".to_string(), limit.to_string()),
    ];

    let cache_key = format!("mr:search:{params:?}");
    let response: SearchResponse = cache::fetch(
        state,
        &cache_key,
        cache::TTL_SEARCH,
        state.network.get(format!("{API}/search")).query(&params),
    )
    .await?;

    Ok(SearchPage {
        hits: response.hits.into_iter().map(summary).collect(),
        total: response.total_hits,
        offset: response.offset,
        limit,
    })
}

#[derive(Deserialize)]
struct Project {
    #[serde(default)]
    id: String,
    #[serde(default)]
    slug: Option<String>,
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
    followers: u64,
    #[serde(default)]
    gallery: Vec<GalleryItem>,
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
    license: Option<License>,
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
    #[serde(default)]
    color: Option<u32>,
}

#[derive(Deserialize)]
struct License {
    #[serde(default)]
    id: String,
}

#[derive(Deserialize)]
struct GalleryItem {
    url: String,
    #[serde(default)]
    raw_url: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    featured: bool,
    #[serde(default)]
    ordering: i64,
}

#[derive(Deserialize)]
struct Member {
    user: User,
    #[serde(default)]
    role: String,
}

#[derive(Deserialize)]
struct User {
    username: String,
}

pub async fn project_details(state: &AppState, project_id: &str) -> Result<ProjectDetails> {
    let project: Project = cache::fetch(
        state,
        &format!("mr:project:{project_id}"),
        cache::TTL_PROJECT,
        state.network.get(format!("{API}/project/{project_id}")),
    )
    .await?;

    let author = cache::fetch::<Vec<Member>>(
        state,
        &format!("mr:members:{project_id}"),
        cache::TTL_PROJECT,
        state
            .network
            .get(format!("{API}/project/{project_id}/members")),
    )
    .await
    .ok()
    .and_then(|members| {
        members
            .iter()
            .find(|m| m.role.eq_ignore_ascii_case("owner"))
            .or_else(|| members.first())
            .map(|m| m.user.username.clone())
    })
    .unwrap_or_default();

    let mut links = Vec::new();
    for (label, url) in [
        ("Report issues", &project.issues_url),
        ("View source", &project.source_url),
        ("Visit wiki", &project.wiki_url),
        ("Join Discord", &project.discord_url),
    ] {
        if let Some(url) = url.as_ref().filter(|u| !u.is_empty()) {
            links.push(ProjectLink {
                label: label.to_string(),
                url: url.clone(),
            });
        }
    }

    let mut gallery = project.gallery;
    gallery.sort_by_key(|g| g.ordering);
    let slug = project.slug.clone();
    let id = if project.id.is_empty() {
        project_id.to_string()
    } else {
        project.id.clone()
    };
    let page = slug.clone().unwrap_or_else(|| id.clone());

    let (categories, loaders_from_categories) = split_categories(project.categories);
    let loaders = if project.loaders.is_empty() {
        loaders_from_categories
    } else {
        project.loaders
    };

    Ok(ProjectDetails {
        id,
        slug,
        title: project.title,
        description: project.description,
        body: project.body,
        body_format: "markdown".to_string(),
        icon_url: project.icon_url,
        downloads: project.downloads,
        follows: project.followers,
        author,
        gallery: gallery
            .into_iter()
            .map(|g| GalleryImage {
                raw_url: g.raw_url,
                url: g.url,
                title: g.title,
                description: g.description,
                featured: g.featured,
            })
            .collect(),
        game_versions: project.game_versions,
        loaders,
        client_side: project.client_side,
        server_side: project.server_side,
        categories,
        license: project.license.map(|l| l.id).filter(|id| !id.is_empty()),
        links,
        published: project.published,
        updated: project.updated,
        website_url: Some(format!("https://modrinth.com/project/{page}")),
        color: project.color,
    })
}

#[derive(Deserialize, Clone)]
pub struct Version {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version_number: String,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub changelog: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub files: Vec<File>,
}

#[derive(Deserialize, Clone)]
pub struct Dependency {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub dependency_type: String,
}

#[derive(Deserialize, Clone)]
pub struct File {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub hashes: Hashes,
    #[serde(default)]
    pub size: Option<u64>,
}

#[derive(Deserialize, Default, Clone)]
pub struct Hashes {
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub sha512: Option<String>,
}

pub fn to_version(
    raw: Version,
    game_version: &str,
    loader: Option<&str>,
    kind: ContentKind,
) -> ProjectVersion {
    let files: Vec<VersionFile> = raw
        .files
        .iter()
        .map(|f| VersionFile {
            url: Some(f.url.clone()),
            file_name: f.filename.clone(),
            sha1: f.hashes.sha1.clone(),
            sha512: f.hashes.sha512.clone(),
            size: f.size,
            primary: f.primary,
        })
        .collect();

    let primary = files.iter().find(|f| f.primary).or_else(|| files.first());
    let file_name = primary.map(|f| f.file_name.clone()).unwrap_or_default();
    let size = primary.and_then(|f| f.size);

    let mut version = ProjectVersion {
        id: raw.id.clone(),
        project_id: raw.project_id.clone(),
        name: if raw.name.is_empty() {
            raw.version_number.clone()
        } else {
            raw.name.clone()
        },
        version_number: raw.version_number.clone(),
        channel: raw.version_type.clone(),
        date: raw.date_published.clone(),
        downloads: raw.downloads,
        file_name,
        size,
        game_versions: raw.game_versions.clone(),
        loaders: raw.loaders.clone(),
        compatible: false,
        changelog: raw.changelog.clone().filter(|c| !c.trim().is_empty()),
        dependencies: raw
            .dependencies
            .iter()
            .filter_map(|d| {
                d.project_id.clone().map(|project_id| VersionDependency {
                    project_id,
                    version_id: d.version_id.clone(),
                    dependency_type: d.dependency_type.clone(),
                })
            })
            .collect(),
        files,
        server_pack_file_id: None,
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
    let versions: Vec<Version> = cache::fetch(
        state,
        &format!("mr:versions:{project_id}"),
        cache::TTL_VERSIONS,
        state
            .network
            .get(format!("{API}/project/{project_id}/version")),
    )
    .await?;

    Ok(versions
        .into_iter()
        .map(|v| to_version(v, game_version, loader, kind))
        .collect())
}

pub async fn version(state: &AppState, version_id: &str) -> Result<Version> {
    cache::fetch(
        state,
        &format!("mr:version:{version_id}"),
        cache::TTL_VERSIONS,
        state.network.get(format!("{API}/version/{version_id}")),
    )
    .await
}

#[derive(Deserialize)]
struct ProjectListItem {
    id: String,
    #[serde(default)]
    slug: Option<String>,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    followers: u64,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    updated: Option<String>,
    #[serde(default)]
    color: Option<u32>,
}

pub async fn resolve_projects(state: &AppState, ids: &[String]) -> Result<Vec<ProjectSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut sorted = ids.to_vec();
    sorted.sort();
    sorted.dedup();

    let projects: Vec<ProjectListItem> = cache::fetch(
        state,
        &format!("mr:projects:{}", sorted.join(",")),
        cache::TTL_PROJECT,
        state
            .network
            .get(format!("{API}/projects"))
            .query(&[("ids", serde_json::to_string(&sorted)?)]),
    )
    .await?;

    Ok(projects
        .into_iter()
        .map(|p| {
            let (categories, loaders) = split_categories(p.categories);
            ProjectSummary {
                id: p.id,
                slug: p.slug,
                title: p.title,
                description: p.description,
                icon_url: p.icon_url,
                downloads: p.downloads,
                follows: p.followers,
                author: String::new(),
                categories,
                game_versions: p.game_versions,
                loaders,
                updated: p.updated,
                color: p.color,
            }
        })
        .collect())
}

pub async fn versions_by_hash(
    state: &AppState,
    hashes: &[String],
) -> Result<HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    let mut out = HashMap::new();
    for chunk in hashes.chunks(500) {
        let response: HashMap<String, Version> = cache::post(
            state,
            state
                .network
                .post(format!("{API}/version_files"))
                .json(&serde_json::json!({ "hashes": chunk, "algorithm": "sha1" })),
        )
        .await?;
        out.extend(response);
    }
    Ok(out)
}

pub async fn latest_versions_by_hash(
    state: &AppState,
    hashes: &[String],
    loaders: &[String],
    game_versions: &[String],
) -> Result<HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    let mut out = HashMap::new();
    for chunk in hashes.chunks(500) {
        let response: HashMap<String, Version> = cache::post(
            state,
            state
                .network
                .post(format!("{API}/version_files/update"))
                .json(&serde_json::json!({
                    "hashes": chunk,
                    "algorithm": "sha1",
                    "loaders": loaders,
                    "game_versions": game_versions,
                })),
        )
        .await?;
        out.extend(response);
    }
    Ok(out)
}

pub async fn changelog(state: &AppState, version_id: &str) -> Result<Changelog> {
    let version = version(state, version_id).await?;
    Ok(Changelog {
        body: version.changelog.unwrap_or_default(),
        format: "markdown".to_string(),
    })
}

#[derive(Deserialize)]
struct CategoryTag {
    name: String,
    #[serde(default)]
    project_type: String,
    #[serde(default)]
    header: String,
}

#[derive(Deserialize)]
struct LoaderTag {
    name: String,
    #[serde(default)]
    supported_project_types: Vec<String>,
}

#[derive(Deserialize)]
struct GameVersionTag {
    version: String,
    #[serde(default)]
    version_type: String,
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub async fn taxonomy(
    state: &AppState,
    kind: ContentKind,
    include_snapshots: bool,
) -> Result<FilterTaxonomy> {
    let wanted = project_type(kind);

    let categories: Vec<CategoryTag> = cache::fetch(
        state,
        "mr:tag:category",
        cache::TTL_TAGS,
        state.network.get(format!("{API}/tag/category")),
    )
    .await?;

    let loaders: Vec<LoaderTag> = cache::fetch(
        state,
        "mr:tag:loader",
        cache::TTL_TAGS,
        state.network.get(format!("{API}/tag/loader")),
    )
    .await?;

    let game_versions: Vec<GameVersionTag> = cache::fetch(
        state,
        "mr:tag:game_version",
        cache::TTL_TAGS,
        state.network.get(format!("{API}/tag/game_version")),
    )
    .await?;

    Ok(FilterTaxonomy {
        categories: categories
            .into_iter()
            .filter(|c| c.project_type == wanted)
            .map(|c| FilterOption {
                id: c.name.clone(),
                name: title_case(&c.name.replace('-', " ")),
                group: title_case(&c.header),
            })
            .collect(),
        loaders: loaders
            .into_iter()
            .filter(|l| {
                l.supported_project_types.iter().any(|t| t == wanted)
                    && is_installable_loader(&l.name)
            })
            .map(|l| FilterOption {
                id: l.name.clone(),
                name: title_case(&l.name),
                group: "Loaders".to_string(),
            })
            .collect(),
        game_versions: game_versions
            .into_iter()
            .filter(|v| include_snapshots || v.version_type == "release")
            .map(|v| v.version)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::{build_facets, split_categories};
    use crate::search::model::{ContentKind, Environment, SearchQuery};

    #[test]
    fn facets_combine_or_groups_with_and() {
        let query = SearchQuery {
            game_versions: vec!["1.21".into(), "1.21.1".into()],
            loaders: vec!["fabric".into()],
            categories: vec!["optimization".into()],
            environment: Some(Environment::Client),
            open_source_only: true,
            ..Default::default()
        };
        let facets = build_facets(ContentKind::Mod, &query);
        assert_eq!(facets[0], vec!["project_type:mod"]);
        assert_eq!(facets[1], vec!["versions:1.21", "versions:1.21.1"]);
        assert_eq!(facets[2], vec!["categories:fabric"]);
        assert_eq!(facets[3], vec!["categories:optimization"]);
        assert_eq!(facets[4], vec!["client_side:required"]);
        assert_eq!(facets[5], vec!["open_source:true"]);
    }

    #[test]
    fn a_server_search_keeps_mods_that_are_only_optional_there() {
        let query = SearchQuery {
            loaders: vec!["fabric".into()],
            environment: Some(Environment::Server),
            ..Default::default()
        };
        let facets = build_facets(ContentKind::Mod, &query);
        assert!(facets.contains(&vec![
            "server_side:required".to_string(),
            "server_side:optional".to_string()
        ]));
    }

    #[test]
    fn a_plugin_search_never_filters_on_the_environment() {
        let query = SearchQuery {
            loaders: vec!["paper".into()],
            environment: Some(Environment::Server),
            ..Default::default()
        };
        let facets = build_facets(ContentKind::Mod, &query);
        assert_eq!(facets[0], vec!["project_type:mod"]);
        assert!(facets.iter().all(|group| {
            group
                .iter()
                .all(|facet| !facet.starts_with("server_side") && !facet.starts_with("client_side"))
        }));
    }

    #[test]
    fn loaders_are_split_out_of_categories() {
        let (categories, loaders) = split_categories(vec![
            "fabric".into(),
            "optimization".into(),
            "quilt".into(),
            "utility".into(),
        ]);
        assert_eq!(categories, vec!["optimization", "utility"]);
        assert_eq!(loaders, vec!["fabric", "quilt"]);
    }

    #[test]
    fn resource_packs_do_not_get_loader_facets() {
        let query = SearchQuery {
            loaders: vec!["fabric".into()],
            ..Default::default()
        };
        let facets = build_facets(ContentKind::ResourcePack, &query);
        assert_eq!(facets.len(), 1);
    }
}
