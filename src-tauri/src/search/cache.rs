use reqwest::{header::IF_NONE_MATCH, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;

use crate::{db::CachedResponse, error::Result, state::AppState};

pub const TTL_TAGS: i64 = 60 * 60 * 24;
pub const TTL_SEARCH: i64 = 60 * 5;
pub const TTL_PROJECT: i64 = 60 * 60;
pub const TTL_VERSIONS: i64 = 60 * 15;

pub const MAX_STALE_FALLBACK: i64 = 60 * 60 * 24;

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn servable_stale(cached: &Option<CachedResponse>) -> Option<&CachedResponse> {
    cached
        .as_ref()
        .filter(|entry| entry.age_secs <= MAX_STALE_FALLBACK)
}

fn rejects_key(status: StatusCode) -> bool {
    matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
}

pub async fn fetch<T: DeserializeOwned>(
    state: &AppState,
    key: &str,
    ttl_secs: i64,
    request: RequestBuilder,
) -> Result<T> {
    let cached = state.db.cache_get(key, now()).ok().flatten();
    if let Some(entry) = &cached {
        if entry.fresh {
            if let Ok(value) = serde_json::from_str(&entry.body) {
                return Ok(value);
            }
        }
    }

    let request = match cached.as_ref().and_then(|c| c.etag.as_deref()) {
        Some(etag) => request.header(IF_NONE_MATCH, etag),
        None => request,
    };

    let fetched = match state.network.fetch_body(request).await {
        Ok(fetched) => fetched,
        Err(e) => match servable_stale(&cached) {
            Some(entry) => return Ok(serde_json::from_str(&entry.body)?),
            None => return Err(e),
        },
    };

    if fetched.status == StatusCode::NOT_MODIFIED {
        if let Some(entry) = &cached {
            let _ = state.db.cache_touch(key, now());
            return Ok(serde_json::from_str(&entry.body)?);
        }
    }

    if !fetched.status.is_success() {
        let error = crate::error::Error::http_response(fetched.status, fetched.body.as_bytes());
        if rejects_key(fetched.status) {
            return Err(error);
        }
        return match servable_stale(&cached) {
            Some(entry) => Ok(serde_json::from_str(&entry.body)?),
            None => Err(error),
        };
    }

    let value = serde_json::from_str(&fetched.body)?;
    let _ = state
        .db
        .cache_put(key, &fetched.body, fetched.etag.as_deref(), now(), ttl_secs);
    Ok(value)
}

pub async fn post<T: DeserializeOwned>(state: &AppState, request: RequestBuilder) -> Result<T> {
    let fetched = state.network.fetch_body(request).await?;
    if !fetched.status.is_success() {
        return Err(crate::error::Error::http_response(
            fetched.status,
            fetched.body.as_bytes(),
        ));
    }
    Ok(serde_json::from_str(&fetched.body)?)
}

#[cfg(test)]
mod tests {
    use super::{rejects_key, servable_stale, MAX_STALE_FALLBACK};
    use crate::db::CachedResponse;
    use reqwest::StatusCode;

    fn entry(age_secs: i64) -> Option<CachedResponse> {
        Some(CachedResponse {
            body: "{}".to_string(),
            etag: None,
            fresh: false,
            age_secs,
        })
    }

    #[test]
    fn stale_entries_are_served_only_inside_the_fallback_window() {
        assert!(servable_stale(&entry(MAX_STALE_FALLBACK - 1)).is_some());
        assert!(servable_stale(&entry(MAX_STALE_FALLBACK)).is_some());
        assert!(servable_stale(&entry(MAX_STALE_FALLBACK + 1)).is_none());
        assert!(servable_stale(&None).is_none());
    }

    #[test]
    fn only_auth_failures_bypass_the_stale_fallback() {
        assert!(rejects_key(StatusCode::UNAUTHORIZED));
        assert!(rejects_key(StatusCode::FORBIDDEN));
        assert!(!rejects_key(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(!rejects_key(StatusCode::TOO_MANY_REQUESTS));
        assert!(!rejects_key(StatusCode::SERVICE_UNAVAILABLE));
    }
}
