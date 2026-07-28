use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::{RequestBuilder, Response, StatusCode};
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::error::{Error, Result};

const MAX_ATTEMPTS: u32 = 4;
const BASE_BACKOFF_MS: u64 = 400;
const MAX_BACKOFF: Duration = Duration::from_secs(20);

pub struct RateLimiter {
    concurrency: Semaphore,
    window: Mutex<VecDeque<Instant>>,
    limit: usize,
    period: Duration,
}

pub struct Lease<'a> {
    _permit: SemaphorePermit<'a>,
}

impl RateLimiter {
    pub fn new(limit: usize, period: Duration, concurrency: usize) -> Self {
        Self {
            concurrency: Semaphore::new(concurrency.max(1)),
            window: Mutex::new(VecDeque::with_capacity(limit)),
            limit: limit.max(1),
            period,
        }
    }

    fn reserve(&self) -> std::result::Result<(), Duration> {
        let now = Instant::now();
        let mut window = self.window.lock().unwrap();
        while let Some(oldest) = window.front() {
            if now.duration_since(*oldest) >= self.period {
                window.pop_front();
            } else {
                break;
            }
        }
        if window.len() < self.limit {
            window.push_back(now);
            return Ok(());
        }
        let oldest = *window.front().expect("window is full so it has a front");
        Err(self.period - now.duration_since(oldest) + Duration::from_millis(25))
    }

    pub async fn acquire(&self) -> Lease<'_> {
        let permit = self
            .concurrency
            .acquire()
            .await
            .expect("rate limiter semaphore is never closed");
        loop {
            match self.reserve() {
                Ok(()) => return Lease { _permit: permit },
                Err(wait) => tokio::time::sleep(wait).await,
            }
        }
    }
}

fn retry_after(response: &Response) -> Option<Duration> {
    let header = response.headers().get(reqwest::header::RETRY_AFTER)?;
    let secs: u64 = header.to_str().ok()?.trim().parse().ok()?;
    Some(Duration::from_secs(secs).min(MAX_BACKOFF))
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(BASE_BACKOFF_MS * 2u64.pow(attempt.min(6))).min(MAX_BACKOFF)
}

fn should_retry(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

pub struct Fetched {
    pub status: reqwest::StatusCode,
    pub etag: Option<String>,
    pub body: String,
}

pub async fn fetch_body(limiter: &RateLimiter, request: RequestBuilder) -> Result<Fetched> {
    let mut attempt = 0;
    loop {
        let attempt_request = request
            .try_clone()
            .ok_or_else(|| Error::other("request body cannot be retried"))?;

        let outcome = async {
            let _lease = limiter.acquire().await;
            let response = match attempt_request.send().await {
                Ok(response) => response,
                Err(e) => return Err((None, Some(e))),
            };
            let status = response.status();
            let etag = response
                .headers()
                .get(reqwest::header::ETAG)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            if should_retry(status) {
                return Err((Some(status), None));
            }
            match response.text().await {
                Ok(body) => Ok(Fetched { status, etag, body }),
                Err(e) => Err((None, Some(e))),
            }
        }
        .await;

        match outcome {
            Ok(fetched) => return Ok(fetched),
            Err((status, error)) => {
                attempt += 1;
                if attempt >= MAX_ATTEMPTS {
                    return match error {
                        Some(e) => Err(e.into()),
                        None => Err(Error::other(format!(
                            "request failed after {attempt} attempts{}",
                            status.map(|s| format!(" ({s})")).unwrap_or_default()
                        ))),
                    };
                }
                tokio::time::sleep(backoff(attempt)).await;
            }
        }
    }
}

pub async fn send(limiter: &RateLimiter, request: RequestBuilder) -> Result<Response> {
    let mut attempt = 0;
    loop {
        let attempt_request = request
            .try_clone()
            .ok_or_else(|| Error::other("request body cannot be retried"))?;
        let outcome = {
            let _lease = limiter.acquire().await;
            attempt_request.send().await
        };

        let wait = match &outcome {
            Ok(response) if should_retry(response.status()) => {
                retry_after(response).unwrap_or_else(|| backoff(attempt))
            }
            Ok(_) => return Ok(outcome?),
            Err(e) if e.is_timeout() || e.is_connect() => backoff(attempt),
            Err(_) => return Ok(outcome?),
        };

        attempt += 1;
        if attempt >= MAX_ATTEMPTS {
            let response = outcome?;
            return Ok(response.error_for_status()?);
        }
        tokio::time::sleep(wait).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{backoff, RateLimiter, MAX_BACKOFF};
    use std::time::Duration;

    #[test]
    fn window_allows_up_to_limit_then_asks_for_a_wait() {
        let limiter = RateLimiter::new(3, Duration::from_secs(60), 4);
        assert!(limiter.reserve().is_ok());
        assert!(limiter.reserve().is_ok());
        assert!(limiter.reserve().is_ok());
        let wait = limiter.reserve().unwrap_err();
        assert!(wait <= Duration::from_secs(61));
    }

    #[test]
    fn backoff_grows_and_saturates() {
        assert!(backoff(0) < backoff(1));
        assert!(backoff(1) < backoff(2));
        assert_eq!(backoff(30), MAX_BACKOFF);
    }
}
