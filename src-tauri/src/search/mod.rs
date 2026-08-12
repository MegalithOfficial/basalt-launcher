pub mod cache;
pub mod curseforge;
pub mod identify;
pub mod model;
pub mod modrinth;
pub mod resolve;
pub mod updates;

pub use model::*;

use crate::{
    error::{Error, Result},
    state::AppState,
};

pub async fn search(
    state: &AppState,
    provider: Provider,
    kind: ContentKind,
    query: &SearchQuery,
) -> Result<SearchPage> {
    match provider {
        Provider::Modrinth => modrinth::search(state, kind, query).await,
        Provider::Curseforge => curseforge::search(state, kind, query).await,
    }
}

pub async fn project_details(
    state: &AppState,
    provider: Provider,
    project_id: &str,
) -> Result<ProjectDetails> {
    match provider {
        Provider::Modrinth => modrinth::project_details(state, project_id).await,
        Provider::Curseforge => curseforge::project_details(state, project_id).await,
    }
}

pub async fn project_versions(
    state: &AppState,
    provider: Provider,
    project_id: &str,
    kind: ContentKind,
    game_version: &str,
    loader: Option<&str>,
) -> Result<Vec<ProjectVersion>> {
    let mut versions = match provider {
        Provider::Modrinth => {
            modrinth::project_versions(state, project_id, kind, game_version, loader).await?
        }
        Provider::Curseforge => {
            curseforge::project_versions(state, project_id, kind, game_version, loader).await?
        }
    };
    versions.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(versions)
}

pub async fn resolve_projects(
    state: &AppState,
    provider: Provider,
    ids: &[String],
) -> Result<Vec<ProjectSummary>> {
    match provider {
        Provider::Modrinth => modrinth::resolve_projects(state, ids).await,
        Provider::Curseforge => curseforge::resolve_projects(state, ids).await,
    }
}

pub async fn version_changelog(
    state: &AppState,
    provider: Provider,
    project_id: &str,
    version_id: &str,
) -> Result<Changelog> {
    match provider {
        Provider::Modrinth => modrinth::changelog(state, version_id).await,
        Provider::Curseforge => curseforge::changelog(state, project_id, version_id).await,
    }
}

pub async fn taxonomy(
    state: &AppState,
    provider: Provider,
    kind: ContentKind,
    include_snapshots: bool,
) -> Result<FilterTaxonomy> {
    match provider {
        Provider::Modrinth => modrinth::taxonomy(state, kind, include_snapshots).await,
        Provider::Curseforge => curseforge::taxonomy(state, kind).await,
    }
}

fn channel_rank(channel: &str) -> u8 {
    match channel {
        "release" => 0,
        "beta" => 1,
        _ => 2,
    }
}

pub fn pick_best(versions: Vec<ProjectVersion>) -> Option<ProjectVersion> {
    let mut compatible: Vec<ProjectVersion> =
        versions.into_iter().filter(|v| v.compatible).collect();
    compatible.sort_by(|a, b| {
        channel_rank(&a.channel)
            .cmp(&channel_rank(&b.channel))
            .then(b.date.cmp(&a.date))
    });
    compatible.into_iter().next()
}

pub async fn fetch_version(
    state: &AppState,
    provider: Provider,
    project_id: &str,
    kind: ContentKind,
    game_version: &str,
    loader: Option<&str>,
    version_id: Option<&str>,
) -> Result<ProjectVersion> {
    match (provider, version_id) {
        (Provider::Modrinth, Some(id)) => {
            let raw = modrinth::version(state, id).await?;
            Ok(modrinth::to_version(raw, game_version, loader, kind))
        }
        (Provider::Curseforge, Some(id)) => {
            let file = curseforge::version(state, project_id, id).await?;
            Ok(curseforge::to_version(
                file,
                project_id,
                game_version,
                loader,
                kind,
            ))
        }
        (provider, None) => {
            let versions =
                project_versions(state, provider, project_id, kind, game_version, loader).await?;
            pick_best(versions).ok_or_else(|| {
                Error::other(format!(
                    "No version of this project supports {game_version}{}.",
                    loader.map(|l| format!(" with {l}")).unwrap_or_default()
                ))
            })
        }
    }
}

pub fn download_url(version: &ProjectVersion) -> Result<(String, VersionFile)> {
    let file = version
        .primary_file()
        .ok_or_else(|| Error::other("This version has no downloadable file."))?;
    let url = file.url.clone().ok_or_else(|| {
        Error::other(format!(
            "{} cannot be downloaded automatically. The author disabled third-party downloads, so it has to be fetched from the project page.",
            file.file_name
        ))
    })?;
    Ok((url, file.clone()))
}

#[cfg(test)]
mod tests {
    use super::{pick_best, ProjectVersion};

    fn version(id: &str, channel: &str, date: &str, compatible: bool) -> ProjectVersion {
        ProjectVersion {
            server_pack_file_id: None,
            id: id.into(),
            project_id: "p".into(),
            name: id.into(),
            version_number: id.into(),
            channel: channel.into(),
            date: date.into(),
            downloads: 0,
            file_name: "a.jar".into(),
            size: None,
            game_versions: vec!["1.21".into()],
            loaders: vec!["fabric".into()],
            compatible,
            changelog: None,
            dependencies: Vec::new(),
            files: Vec::new(),
        }
    }

    #[test]
    fn prefers_newest_release_over_newer_beta() {
        let picked = pick_best(vec![
            version("beta", "beta", "2026-07-20T00:00:00Z", true),
            version("old", "release", "2026-01-01T00:00:00Z", true),
            version("new", "release", "2026-07-01T00:00:00Z", true),
        ]);
        assert_eq!(picked.unwrap().id, "new");
    }

    #[test]
    fn skips_incompatible_versions() {
        let picked = pick_best(vec![
            version("bad", "release", "2026-07-20T00:00:00Z", false),
            version("good", "beta", "2026-01-01T00:00:00Z", true),
        ]);
        assert_eq!(picked.unwrap().id, "good");
    }

    #[test]
    fn returns_nothing_when_all_incompatible() {
        assert!(pick_best(vec![version(
            "bad",
            "release",
            "2026-07-20T00:00:00Z",
            false
        )])
        .is_none());
    }
}
