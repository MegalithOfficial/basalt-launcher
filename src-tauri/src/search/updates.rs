use std::collections::HashMap;

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

async fn modrinth_updates(
    state: &AppState,
    files: &[crate::db::ContentFile],
    kind: &str,
    game_version: &str,
    loader: Option<&str>,
    include_pack: bool,
) -> Vec<ContentUpdate> {
    let candidates: Vec<(String, String, String)> = files
        .iter()
        .filter(|f| include_pack || f.origin != "pack")
        .filter(|f| f.provider.as_deref() == Some("modrinth"))
        .filter_map(|f| {
            let sha1 = f.sha1.clone()?;
            let version_id = f.version_id.clone()?;
            Some((f.file_name.clone(), sha1, version_id))
        })
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
    files: &[crate::db::ContentFile],
    kind: &str,
    game_version: &str,
    loader: Option<&str>,
    include_pack: bool,
) -> Vec<ContentUpdate> {
    if curseforge::key(state).is_err() {
        return Vec::new();
    }
    let Ok(content_kind) = ContentKind::parse(kind) else {
        return Vec::new();
    };

    let tracked: Vec<(String, String, Option<String>)> = files
        .iter()
        .filter(|f| include_pack || f.origin != "pack")
        .filter(|f| f.provider.as_deref() == Some("curseforge"))
        .filter_map(|f| {
            let project_id = f.project_id.clone()?;
            Some((f.file_name.clone(), project_id, f.version_id.clone()))
        })
        .collect();

    if tracked.is_empty() {
        return Vec::new();
    }

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
                latest_version_id: latest.id.clone(),
                latest_name: latest.name.clone(),
                latest_file_name: latest.file_name.clone(),
            })
        })
        .collect()
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
        all.extend(modrinth_updates(state, &files, kind, game_version, loader, include_pack).await);
        all.extend(
            curseforge_updates(state, &files, kind, game_version, loader, include_pack).await,
        );
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
    let mut all = modrinth_updates(state, &files, kind, &server.version_id, loader, true).await;
    all.extend(curseforge_updates(state, &files, kind, &server.version_id, loader, true).await);
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
