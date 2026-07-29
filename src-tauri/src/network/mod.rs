use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures::{Stream, StreamExt};
use reqwest::{IntoUrl, Method, Request, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::error::{Error, Result};

const REQUESTS_PER_MINUTE: usize = 250;
const MAX_CONCURRENT_REQUESTS: usize = 8;
const MAX_ATTEMPTS: u32 = 4;
const BASE_BACKOFF_MS: u64 = 400;
const MAX_BACKOFF: Duration = Duration::from_secs(20);
const MAX_RETRY_AFTER: Duration = Duration::from_secs(5 * 60);
const MAX_BUFFERED_BODY: usize = 16 * 1024 * 1024;

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

    pub async fn send_once(&self, request: RequestBuilder) -> Result<ManagedResponse> {
        self.execute_once(request.build()?).await
    }

    async fn execute_once(&self, request: Request) -> Result<ManagedResponse> {
        let lease = self.limiter.acquire().await;
        let response = self.client.execute(request).await?;
        Ok(ManagedResponse {
            response,
            _lease: lease,
        })
    }

    pub async fn send(&self, request: RequestBuilder) -> Result<BufferedResponse> {
        let request = request.build()?;
        let method = request.method().clone();
        if !method_is_retryable(&method) {
            return self.execute_once(request).await?.buffer().await;
        }

        let mut attempt = 0;
        loop {
            let attempt_request = request
                .try_clone()
                .ok_or_else(|| Error::other("request body cannot be retried"))?;
            match self.execute_once(attempt_request).await {
                Ok(response) if should_retry(response.status()) => {
                    let wait = retry_after(&response).unwrap_or_else(|| backoff(attempt));
                    attempt += 1;
                    if attempt >= MAX_ATTEMPTS {
                        return Err(Error::HttpStatus(response.status()));
                    }
                    drop(response);
                    tokio::time::sleep(wait).await;
                }
                Ok(response) => match response.buffer().await {
                    Ok(response) => return Ok(response),
                    Err(Error::Http(error))
                        if error.is_body() || error.is_decode() || error.is_timeout() =>
                    {
                        attempt += 1;
                        if attempt >= MAX_ATTEMPTS {
                            return Err(error.into());
                        }
                        tokio::time::sleep(backoff(attempt - 1)).await;
                    }
                    Err(error) => return Err(error),
                },
                Err(Error::Http(error)) if error.is_timeout() || error.is_connect() => {
                    attempt += 1;
                    if attempt >= MAX_ATTEMPTS {
                        return Err(error.into());
                    }
                    tokio::time::sleep(backoff(attempt - 1)).await;
                }
                Err(error) => return Err(error),
            }
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
    concurrency: Arc<Semaphore>,
    window: Mutex<VecDeque<Instant>>,
    limit: usize,
    period: Duration,
}

struct Lease {
    _permit: OwnedSemaphorePermit,
}

impl RateLimiter {
    fn new(limit: usize, period: Duration, concurrency: usize) -> Self {
        Self {
            concurrency: Arc::new(Semaphore::new(concurrency.max(1))),
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

    async fn acquire(&self) -> Lease {
        let permit = self
            .concurrency
            .clone()
            .acquire_owned()
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

pub struct ManagedResponse {
    response: Response,
    _lease: Lease,
}

impl ManagedResponse {
    pub fn status(&self) -> StatusCode {
        self.response.status()
    }

    pub fn headers(&self) -> &reqwest::header::HeaderMap {
        self.response.headers()
    }

    pub fn content_length(&self) -> Option<u64> {
        self.response.content_length()
    }

    pub fn error_for_status(self) -> Result<Self> {
        self.response.error_for_status_ref()?;
        Ok(self)
    }

    pub async fn bytes(self) -> Result<Vec<u8>> {
        let ManagedResponse { response, _lease } = self;
        if let Some(actual) = response.content_length() {
            if actual > MAX_BUFFERED_BODY as u64 {
                return Err(Error::ResponseTooLarge {
                    limit: MAX_BUFFERED_BODY,
                    actual,
                });
            }
        }

        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if body.len() + chunk.len() > MAX_BUFFERED_BODY {
                return Err(Error::ResponseTooLarge {
                    limit: MAX_BUFFERED_BODY,
                    actual: (body.len() + chunk.len()) as u64,
                });
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    pub async fn text(self) -> Result<String> {
        String::from_utf8(self.bytes().await?)
            .map_err(|error| Error::other(format!("response body was not valid UTF-8: {error}")))
    }

    pub fn bytes_stream(
        self,
    ) -> impl Stream<Item = std::result::Result<bytes::Bytes, reqwest::Error>> {
        let ManagedResponse { response, _lease } = self;
        response.bytes_stream().map(move |item| {
            let _ = &_lease;
            item
        })
    }

    async fn buffer(self) -> Result<BufferedResponse> {
        let status = self.status();
        let headers = self.headers().clone();
        let body = self.bytes().await?;
        Ok(BufferedResponse {
            status,
            headers,
            body,
        })
    }
}

pub struct BufferedResponse {
    status: StatusCode,
    headers: reqwest::header::HeaderMap,
    body: Vec<u8>,
}

impl BufferedResponse {
    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn headers(&self) -> &reqwest::header::HeaderMap {
        &self.headers
    }

    pub fn error_for_status(self) -> Result<Self> {
        if self.status.is_client_error() || self.status.is_server_error() {
            return Err(Error::HttpStatus(self.status));
        }
        Ok(self)
    }

    pub async fn bytes(self) -> Result<Vec<u8>> {
        Ok(self.body)
    }

    pub async fn text(self) -> Result<String> {
        String::from_utf8(self.body)
            .map_err(|error| Error::other(format!("response body was not valid UTF-8: {error}")))
    }

    pub async fn json<T: DeserializeOwned>(self) -> Result<T> {
        Ok(serde_json::from_slice(&self.body)?)
    }
}

fn retry_after(response: &ManagedResponse) -> Option<Duration> {
    let header = response.headers().get(reqwest::header::RETRY_AFTER)?;
    let value = header.to_str().ok()?.trim();
    parse_retry_after(value, std::time::SystemTime::now())
}

fn parse_retry_after(value: &str, now: std::time::SystemTime) -> Option<Duration> {
    let delay = if let Ok(seconds) = value.parse::<u64>() {
        Duration::from_secs(seconds)
    } else {
        httpdate::parse_http_date(value)
            .ok()?
            .duration_since(now)
            .unwrap_or_default()
    };
    Some(delay.min(MAX_RETRY_AFTER))
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(BASE_BACKOFF_MS * 2u64.pow(attempt.min(6))).min(MAX_BACKOFF)
}

fn should_retry(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn method_is_retryable(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    )
}

pub struct Fetched {
    pub status: StatusCode,
    pub etag: Option<String>,
    pub body: String,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use reqwest::Method;

    use super::{
        backoff, method_is_retryable, parse_retry_after, NetworkManager, RateLimiter, MAX_BACKOFF,
        MAX_RETRY_AFTER,
    };

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

    #[test]
    fn only_read_only_methods_are_retried() {
        assert!(method_is_retryable(&Method::GET));
        assert!(method_is_retryable(&Method::HEAD));
        assert!(!method_is_retryable(&Method::POST));
        assert!(!method_is_retryable(&Method::PUT));
        assert!(!method_is_retryable(&Method::DELETE));
        assert!(!method_is_retryable(&Method::PATCH));
    }

    #[test]
    fn retry_after_accepts_seconds_and_http_dates() {
        let now = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert_eq!(parse_retry_after("12", now), Some(Duration::from_secs(12)));
        let date = httpdate::fmt_http_date(now + Duration::from_secs(45));
        assert_eq!(parse_retry_after(&date, now), Some(Duration::from_secs(45)));
        assert_eq!(parse_retry_after("999999", now), Some(MAX_RETRY_AFTER));
    }

    #[tokio::test]
    async fn interrupted_get_bodies_are_retried() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for attempt in 0..2 {
                let (mut socket, _) = listener.accept().unwrap();
                let mut request = [0_u8; 1024];
                let _ = socket.read(&mut request);
                if attempt == 0 {
                    socket
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nbad",
                        )
                        .unwrap();
                } else {
                    socket
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\ncomplete",
                        )
                        .unwrap();
                }
            }
        });

        let network = NetworkManager::with_client(reqwest::Client::new());
        let body = network
            .send(network.get(format!("http://{address}/body")))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(body, "complete");
    }

    #[tokio::test]
    async fn non_cloneable_mutation_requests_execute_once() {
        use std::io::{Read, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request);
            socket
                .write_all(
                    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });

        let network = NetworkManager::with_client(reqwest::Client::new());
        let body = reqwest::Body::wrap_stream(futures::stream::once(async {
            Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"payload"))
        }));
        let response = network
            .send(
                network
                    .post(format!("http://{address}/mutation"))
                    .body(body),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn concurrency_lease_lasts_until_the_response_is_dropped() {
        let limiter = RateLimiter::new(10, Duration::from_secs(60), 1);
        let first = limiter.acquire().await;
        assert!(
            tokio::time::timeout(Duration::from_millis(25), limiter.acquire())
                .await
                .is_err()
        );
        drop(first);
        assert!(
            tokio::time::timeout(Duration::from_millis(25), limiter.acquire())
                .await
                .is_ok()
        );
    }
}
