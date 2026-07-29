use reqwest::header::IF_NONE_MATCH;
use reqwest::{RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;

use crate::error::Result;
use crate::state::AppState;

pub const TTL_TAGS: i64 = 60 * 60 * 24;
pub const TTL_SEARCH: i64 = 60 * 5;
pub const TTL_PROJECT: i64 = 60 * 60;
pub const TTL_VERSIONS: i64 = 60 * 15;

fn now() -> i64 {
    chrono::Utc::now().timestamp()
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
        Err(e) => match cached {
            Some(entry) => return Ok(serde_json::from_str(&entry.body)?),
            None => return Err(e),
        },
    };

    if fetched.status == StatusCode::NOT_MODIFIED {
        if let Some(entry) = cached {
            let _ = state.db.cache_touch(key, now());
            return Ok(serde_json::from_str(&entry.body)?);
        }
    }

    if !fetched.status.is_success() {
        return match cached {
            Some(entry) => Ok(serde_json::from_str(&entry.body)?),
            None => Err(crate::error::Error::other(format!(
                "request failed with {}",
                fetched.status
            ))),
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
        return Err(crate::error::Error::other(format!(
            "request failed with {}",
            fetched.status
        )));
    }
    Ok(serde_json::from_str(&fetched.body)?)
}
