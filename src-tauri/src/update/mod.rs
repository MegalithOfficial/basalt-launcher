use serde::{Deserialize, Serialize};

use crate::{error::Result, network::NetworkManager};

pub const REPO: &str = "MegalithOfficial/basalt-launcher";
pub const REPO_URL: &str = "https://github.com/MegalithOfficial/basalt-launcher";

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub notes_url: Option<String>,
    pub published_at: Option<String>,
    pub update_available: bool,
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
}

#[derive(Debug, PartialEq)]
struct Version {
    release: Vec<u64>,
    dev: Option<Vec<u64>>,
}

fn parse_version(raw: &str) -> Option<Version> {
    let raw = raw.trim().trim_start_matches(['v', 'V']);
    let (release, dev) = match raw.split_once("-dev.") {
        Some((release, dev)) => (release, Some(dev)),
        None => (raw, None),
    };
    let release = release
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<Vec<u64>, _>>()
        .ok()?;
    if release.is_empty() {
        return None;
    }
    let dev = dev
        .map(|value| {
            value
                .split('.')
                .map(str::parse)
                .collect::<std::result::Result<Vec<u64>, _>>()
        })
        .transpose()
        .ok()?;
    if dev.as_ref().is_some_and(Vec::is_empty) {
        return None;
    }
    Some(Version { release, dev })
}

fn compare_parts(latest: &[u64], current: &[u64]) -> std::cmp::Ordering {
    for i in 0..latest.len().max(current.len()) {
        let ordering = latest
            .get(i)
            .copied()
            .unwrap_or(0)
            .cmp(&current.get(i).copied().unwrap_or(0));
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    std::cmp::Ordering::Equal
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let (Some(latest), Some(current)) = (parse_version(latest), parse_version(current)) else {
        return false;
    };
    match compare_parts(&latest.release, &current.release) {
        std::cmp::Ordering::Greater => return true,
        std::cmp::Ordering::Less => return false,
        std::cmp::Ordering::Equal => {}
    }
    match (latest.dev, current.dev) {
        (None, Some(_)) => true,
        (Some(_), None) | (None, None) => false,
        (Some(latest), Some(current)) => {
            compare_parts(&latest, &current) == std::cmp::Ordering::Greater
        }
    }
}

fn latest_development_release(releases: Vec<Release>) -> Option<Release> {
    releases
        .into_iter()
        .filter(|release| release.prerelease && release.tag_name.contains("-dev."))
        .max_by(|a, b| {
            if is_newer(&a.tag_name, &b.tag_name) {
                std::cmp::Ordering::Greater
            } else if is_newer(&b.tag_name, &a.tag_name) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
}

fn up_to_date(current: String) -> UpdateInfo {
    UpdateInfo {
        current,
        latest: None,
        notes_url: None,
        published_at: None,
        update_available: false,
    }
}

#[tracing::instrument(skip_all, err)]
pub async fn check(client: &NetworkManager) -> Result<UpdateInfo> {
    let current = crate::build_info::display_version();
    let development = crate::build_info::CHANNEL == "dev";
    let url = if development {
        format!("https://api.github.com/repos/{REPO}/releases?per_page=100")
    } else {
        format!("https://api.github.com/repos/{REPO}/releases/latest")
    };

    let request = client
        .get(&url)
        .header("Accept", "application/vnd.github+json");
    let response = client.send(request).await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        tracing::info!(repo = REPO, "no published releases yet");
        return Ok(up_to_date(current));
    }

    let response = response.error_for_status()?;
    let release = if development {
        latest_development_release(response.json::<Vec<Release>>().await?)
    } else {
        Some(response.json::<Release>().await?)
    };
    let Some(release) = release else {
        tracing::info!(repo = REPO, channel = "dev", "no development releases yet");
        return Ok(up_to_date(current));
    };
    let update_available = is_newer(&release.tag_name, &current);
    tracing::info!(
        current = %current,
        latest = %release.tag_name,
        update_available,
        "checked for updates"
    );

    Ok(UpdateInfo {
        current,
        latest: Some(release.tag_name),
        notes_url: Some(release.html_url),
        published_at: release.published_at,
        update_available,
    })
}

#[cfg(test)]
mod tests {
    use super::{is_newer, latest_development_release, parse_version, Release, Version};

    fn release(tag_name: &str, prerelease: bool) -> Release {
        Release {
            tag_name: tag_name.to_string(),
            html_url: format!("https://example.com/{tag_name}"),
            prerelease,
            published_at: None,
        }
    }

    #[test]
    fn strips_tag_prefixes_and_suffixes() {
        assert_eq!(
            parse_version("v1.2.3-dev.42.2"),
            Some(Version {
                release: vec![1, 2, 3],
                dev: Some(vec![42, 2])
            })
        );
        assert_eq!(
            parse_version("0.1.0"),
            Some(Version {
                release: vec![0, 1, 0],
                dev: None
            })
        );
        assert_eq!(parse_version("2.0.0-beta.1"), None);
        assert_eq!(parse_version("nightly"), None);
    }

    #[test]
    fn compares_versions_by_component() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("nightly", "0.1.0"));
    }

    #[test]
    fn compares_development_builds() {
        assert!(is_newer("v1.0.0-dev.5.1", "1.0.0-dev.4.1"));
        assert!(is_newer("v1.0.0-dev.5.2", "1.0.0-dev.5.1"));
        assert!(!is_newer("v1.0.0-dev.5.1", "1.0.0-dev.5.1"));
        assert!(!is_newer("v1.0.0-dev.4.1", "1.0.0-dev.5.1"));
        assert!(is_newer("v1.0.0", "1.0.0-dev.5.1"));
        assert!(!is_newer("v1.0.0-dev.5.1", "1.0.0"));
    }

    #[test]
    fn development_feed_ignores_stable_and_unrelated_prereleases() {
        let latest = latest_development_release(vec![
            release("v2.0.0", false),
            release("v1.0.0-beta.1", true),
            release("v1.0.0-dev.4.1", true),
            release("v1.0.0-dev.5.1", true),
        ])
        .unwrap();

        assert_eq!(latest.tag_name, "v1.0.0-dev.5.1");
    }

    #[test]
    fn treats_missing_components_as_zero() {
        assert!(!is_newer("1.0", "1.0.0"));
        assert!(is_newer("1.0.1", "1.0"));
    }
}
