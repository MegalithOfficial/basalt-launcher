use serde::{Deserialize, Serialize};

use crate::error::Result;

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
    published_at: Option<String>,
}

fn numeric_parts(raw: &str) -> Vec<u64> {
    raw.trim()
        .trim_start_matches(['v', 'V'])
        .split(['.', '-', '+'])
        .map_while(|part| part.parse::<u64>().ok())
        .collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let latest = numeric_parts(latest);
    let current = numeric_parts(current);
    if latest.is_empty() {
        return false;
    }
    for i in 0..latest.len().max(current.len()) {
        let a = latest.get(i).copied().unwrap_or(0);
        let b = current.get(i).copied().unwrap_or(0);
        if a != b {
            return a > b;
        }
    }
    false
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
pub async fn check(client: &reqwest::Client) -> Result<UpdateInfo> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");

    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        tracing::info!(repo = REPO, "no published releases yet");
        return Ok(up_to_date(current));
    }

    let release: Release = response.error_for_status()?.json().await?;
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
    use super::{is_newer, numeric_parts};

    #[test]
    fn strips_tag_prefixes_and_suffixes() {
        assert_eq!(numeric_parts("v1.2.3"), vec![1, 2, 3]);
        assert_eq!(numeric_parts("0.1.0"), vec![0, 1, 0]);
        assert_eq!(numeric_parts("2.0.0-beta.1"), vec![2, 0, 0]);
        assert!(numeric_parts("nightly").is_empty());
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
    fn treats_missing_components_as_zero() {
        assert!(!is_newer("1.0", "1.0.0"));
        assert!(is_newer("1.0.1", "1.0"));
    }
}
