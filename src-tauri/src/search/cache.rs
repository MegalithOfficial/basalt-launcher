use reqwest::header::{ETAG, IF_NONE_MATCH};
use reqwest::{RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;

use crate::error::Result;
use crate::state::AppState;

use super::http;

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

    let response = match http::send(&state.limiter, request).await {
        Ok(response) => response,
        Err(e) => match cached {
            Some(entry) => return Ok(serde_json::from_str(&entry.body)?),
            None => return Err(e),
        },
    };

    if response.status() == StatusCode::NOT_MODIFIED {
        if let Some(entry) = cached {
            let _ = state.db.cache_touch(key, now());
            return Ok(serde_json::from_str(&entry.body)?);
        }
    }

    let response = response.error_for_status()?;
    let etag = response
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = response.text().await?;
    let value = serde_json::from_str(&body)?;
    let _ = state.db.cache_put(key, &body, etag.as_deref(), now(), ttl_secs);
    Ok(value)
}

pub async fn post<T: DeserializeOwned>(state: &AppState, request: RequestBuilder) -> Result<T> {
    let response = http::send(&state.limiter, request).await?.error_for_status()?;
    Ok(response.json().await?)
}
