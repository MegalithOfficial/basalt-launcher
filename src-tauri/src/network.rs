use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::{IntoUrl, RequestBuilder, Response, StatusCode};
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::error::{Error, Result};

const REQUESTS_PER_MINUTE: usize = 250;
const MAX_CONCURRENT_REQUESTS: usize = 8;
const MAX_ATTEMPTS: u32 = 4;
const BASE_BACKOFF_MS: u64 = 400;
const MAX_BACKOFF: Duration = Duration::from_secs(20);

pub struct NetworkManager {
    client: reqwest::Client,
    limiter: RateLimiter,
}

impl NetworkManager {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!(
                "MegalithOfficial/basalt-launcher/",
                env!("CARGO_PKG_VERSION"),
                " (github.com/MegalithOfficial/basalt-launcher)"
            ))
            .timeout(Duration::from_secs(45))
            .build()
            .expect("failed to build HTTP client");
        Self::with_client(client)
    }

    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            client,
            limiter: RateLimiter::new(
                REQUESTS_PER_MINUTE,
                Duration::from_secs(60),
                MAX_CONCURRENT_REQUESTS,
            ),
        }
    }

    pub fn get(&self, url: impl IntoUrl) -> RequestBuilder {
        self.client.get(url)
    }

    pub fn post(&self, url: impl IntoUrl) -> RequestBuilder {
        self.client.post(url)
    }

    pub fn put(&self, url: impl IntoUrl) -> RequestBuilder {
        self.client.put(url)
    }

    pub fn delete(&self, url: impl IntoUrl) -> RequestBuilder {
        self.client.delete(url)
    }

    pub async fn send_once(&self, request: RequestBuilder) -> Result<Response> {
        let _lease = self.limiter.acquire().await;
        Ok(request.send().await?)
    }

    pub async fn send(&self, request: RequestBuilder) -> Result<Response> {
        let mut attempt = 0;
        loop {
            let attempt_request = request
                .try_clone()
                .ok_or_else(|| Error::other("request body cannot be retried"))?;
            let outcome = self.send_once(attempt_request).await;

            let wait = match &outcome {
                Ok(response) if should_retry(response.status()) => {
                    retry_after(response).unwrap_or_else(|| backoff(attempt))
                }
                Ok(_) => return outcome,
                Err(Error::Http(error)) if error.is_timeout() || error.is_connect() => {
                    backoff(attempt)
                }
                Err(_) => return outcome,
            };

            attempt += 1;
            if attempt >= MAX_ATTEMPTS {
                let response = outcome?;
                return Ok(response.error_for_status()?);
            }
            tokio::time::sleep(wait).await;
        }
    }

    pub async fn fetch_body(&self, request: RequestBuilder) -> Result<Fetched> {
        let response = self.send(request).await?;
        let status = response.status();
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response.text().await?;
        Ok(Fetched { status, etag, body })
    }
}

struct RateLimiter {
    concurrency: Semaphore,
    window: Mutex<VecDeque<Instant>>,
    limit: usize,
    period: Duration,
}

struct Lease<'a> {
    _permit: SemaphorePermit<'a>,
}

impl RateLimiter {
    fn new(limit: usize, period: Duration, concurrency: usize) -> Self {
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

    async fn acquire(&self) -> Lease<'_> {
        let permit = self
            .concurrency
            .acquire()
            .await
            .expect("network semaphore is never closed");
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
    let seconds: u64 = header.to_str().ok()?.trim().parse().ok()?;
    Some(Duration::from_secs(seconds).min(MAX_BACKOFF))
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(BASE_BACKOFF_MS * 2u64.pow(attempt.min(6))).min(MAX_BACKOFF)
}

fn should_retry(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

pub struct Fetched {
    pub status: StatusCode,
    pub etag: Option<String>,
    pub body: String,
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
        assert!(limiter.reserve().unwrap_err() <= Duration::from_secs(61));
    }

    #[test]
    fn backoff_grows_and_saturates() {
        assert!(backoff(0) < backoff(1));
        assert!(backoff(1) < backoff(2));
        assert_eq!(backoff(30), MAX_BACKOFF);
    }
}
