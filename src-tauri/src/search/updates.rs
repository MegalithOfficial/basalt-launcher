use std::collections::{HashMap, HashSet};

use super::{curseforge, model::*, modrinth};
use crate::{db::ContentUpdate, error::Result, state::AppState};

pub const KINDS: &[&str] = &["mods", "resourcepacks", "shaderpacks"];

const STALE_AFTER_SECS: i64 = 60 * 60 * 6;

pub fn is_stale(checked_at: Option<i64>) -> bool {
    match checked_at {
        Some(at) => chrono::Utc::now().timestamp() - at > STALE_AFTER_SECS,
        None => true,
    }
}

/** One file as seen by one platform, primary or alternate. */
struct Tracked {
    file_name: String,
    sha1: Option<String>,
    project_id: String,
    version_id: Option<String>,
}

fn primary_link(file: &crate::db::ContentFile) -> Option<(Provider, Tracked)> {
    let provider = Provider::parse(file.provider.as_deref()?).ok()?;
    Some((
        provider,
        Tracked {
            file_name: file.file_name.clone(),
            sha1: file.sha1.clone(),
            project_id: file.project_id.clone()?,
            version_id: file.version_id.clone(),
        },
    ))
}

fn alternate_link(file: &crate::db::ContentFile) -> Option<(Provider, Tracked)> {
    let provider = Provider::parse(file.alt_provider.as_deref()?).ok()?;
    Some((
        provider,
        Tracked {
            file_name: file.file_name.clone(),
            sha1: file.sha1.clone(),
            project_id: file.alt_project_id.clone()?,
            version_id: file.alt_version_id.clone(),
        },
    ))
}

fn split_by_provider(links: Vec<(Provider, Tracked)>) -> (Vec<Tracked>, Vec<Tracked>) {
    let mut modrinth = Vec::new();
    let mut curseforge = Vec::new();
    for (provider, tracked) in links {
        match provider {
            Provider::Modrinth => modrinth.push(tracked),
            Provider::Curseforge => curseforge.push(tracked),
        }
    }
    (modrinth, curseforge)
}

async fn modrinth_updates(
    state: &AppState,
    tracked: Vec<Tracked>,
    kind: &str,
    game_version: &str,
    loader: Option<&str>,
) -> Vec<ContentUpdate> {
    let candidates: Vec<(String, String, String)> = tracked
        .into_iter()
        .filter_map(|t| Some((t.file_name, t.sha1?, t.version_id?)))
        .collect();

    if candidates.is_empty() {
        return Vec::new();
    }

    let loaders: Vec<String> = match loader {
        Some(l) if ContentKind::parse(kind).is_ok_and(|k| k.uses_loaders()) => vec![l.to_string()],
        _ => Vec::new(),
    };
    let game_versions = if game_version.is_empty() {
        Vec::new()
    } else {
        vec![game_version.to_string()]
    };

    let hashes: Vec<String> = candidates.iter().map(|(_, sha1, _)| sha1.clone()).collect();
    let latest = modrinth::latest_versions_by_hash(state, &hashes, &loaders, &game_versions)
        .await
        .unwrap_or_default();

    candidates
        .into_iter()
        .filter_map(|(file_name, sha1, installed_version)| {
            let version = latest.get(&sha1)?;
            if version.id == installed_version {
                return None;
            }
            let file = version
                .files
                .iter()
                .find(|f| f.primary)
                .or_else(|| version.files.first())?;
            Some(ContentUpdate {
                kind: kind.to_string(),
                file_name,
                provider: Some(Provider::Modrinth.as_str().to_string()),
                latest_version_id: version.id.clone(),
                latest_name: if version.name.is_empty() {
                    version.version_number.clone()
                } else {
                    version.name.clone()
                },
                latest_file_name: file.filename.clone(),
            })
        })
        .collect()
}

async fn curseforge_updates(
    state: &AppState,
    tracked: Vec<Tracked>,
    kind: &str,
    game_version: &str,
    loader: Option<&str>,
) -> Vec<ContentUpdate> {
    if tracked.is_empty() || curseforge::key(state).is_err() {
        return Vec::new();
    }
    let Ok(content_kind) = ContentKind::parse(kind) else {
        return Vec::new();
    };
    let tracked: Vec<(String, String, Option<String>)> = tracked
        .into_iter()
        .map(|t| (t.file_name, t.project_id, t.version_id))
        .collect();

    let mut best: HashMap<String, ProjectVersion> = HashMap::new();
    for (_, project_id, _) in &tracked {
        if best.contains_key(project_id) {
            continue;
        }
        let Ok(versions) =
            curseforge::project_versions(state, project_id, content_kind, game_version, loader)
                .await
        else {
            continue;
        };
        if let Some(latest) = super::pick_best(versions) {
            best.insert(project_id.clone(), latest);
        }
    }

    tracked
        .into_iter()
        .filter_map(|(file_name, project_id, installed_version)| {
            let latest = best.get(&project_id)?;
            if Some(&latest.id) == installed_version.as_ref() {
                return None;
            }
            if latest.file_name == file_name {
                return None;
            }
            Some(ContentUpdate {
                kind: kind.to_string(),
                file_name,
                provider: Some(Provider::Curseforge.as_str().to_string()),
                latest_version_id: latest.id.clone(),
                latest_name: latest.name.clone(),
                latest_file_name: latest.file_name.clone(),
            })
        })
        .collect()
}

/**
 * The platform a file was downloaded from is asked first. Files it has nothing
 * newer for are then asked on their alternate platform, so a mod whose author
 * moved on to the other site still gets updates.
 */
async fn check_files(
    state: &AppState,
    files: &[crate::db::ContentFile],
    kind: &str,
    game_version: &str,
    loader: Option<&str>,
    include_pack: bool,
) -> Vec<ContentUpdate> {
    let files: Vec<&crate::db::ContentFile> = files
        .iter()
        .filter(|f| include_pack || f.origin != "pack")
        .collect();

    let (modrinth, curseforge) =
        split_by_provider(files.iter().filter_map(|f| primary_link(f)).collect());
    let mut all = modrinth_updates(state, modrinth, kind, game_version, loader).await;
    all.extend(curseforge_updates(state, curseforge, kind, game_version, loader).await);

    let found: HashSet<&str> = all.iter().map(|u| u.file_name.as_str()).collect();
    let (modrinth, curseforge) = split_by_provider(
        files
            .iter()
            .filter(|f| !found.contains(f.file_name.as_str()))
            .filter_map(|f| alternate_link(f))
            .collect(),
    );
    all.extend(modrinth_updates(state, modrinth, kind, game_version, loader).await);
    all.extend(curseforge_updates(state, curseforge, kind, game_version, loader).await);
    all
}

pub async fn check(
    state: &AppState,
    instance_id: &str,
    game_version: &str,
    loader: Option<&str>,
) -> Result<Vec<ContentUpdate>> {
    let include_pack = state
        .db
        .load_settings()
        .map(|settings| settings.pack_content_updates)
        .unwrap_or(false);
    let mut all = Vec::new();
    for kind in KINDS {
        let files = state
            .db
            .content_files(instance_id, kind)
            .unwrap_or_default();
        all.extend(check_files(state, &files, kind, game_version, loader, include_pack).await);
    }
    state
        .db
        .replace_content_updates(instance_id, &all, chrono::Utc::now().timestamp())?;
    Ok(all)
}

pub async fn check_server(
    state: &AppState,
    server: &crate::servers::Server,
) -> Result<Vec<ContentUpdate>> {
    let kind = ContentKind::Mod.as_str();
    let files = state
        .db
        .server_content_files(&server.id, kind)
        .unwrap_or_default();
    let loader = Some(server.flavor.id());
    let all = check_files(state, &files, kind, &server.version_id, loader, true).await;
    state
        .db
        .replace_server_content_updates(&server.id, &all, chrono::Utc::now().timestamp())?;
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::is_stale;

    #[test]
    fn never_checked_is_stale() {
        assert!(is_stale(None));
    }

    #[test]
    fn recent_checks_are_fresh() {
        let now = chrono::Utc::now().timestamp();
        assert!(!is_stale(Some(now)));
        assert!(!is_stale(Some(now - 60)));
        assert!(is_stale(Some(now - 60 * 60 * 24)));
    }
}
