use serde::{Serialize, Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("http status {0}")]
    HttpStatus(reqwest::StatusCode),

    #[error("http status {status}: {message}")]
    HttpResponse {
        status: reqwest::StatusCode,
        message: String,
    },

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("cancelled")]
    Cancelled,

    #[error("checksum mismatch for {path}: expected {expected}, got {actual}")]
    Checksum {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("size mismatch for {path}: expected {expected} bytes, got {actual}")]
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },

    #[error("response body exceeded the {limit} byte limit after {actual} bytes")]
    ResponseTooLarge { limit: usize, actual: u64 },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }

    pub fn http_response(status: reqwest::StatusCode, body: &[u8]) -> Self {
        match response_message(body) {
            Some(message) => Error::HttpResponse { status, message },
            None => Error::HttpStatus(status),
        }
    }
}

fn response_message(body: &[u8]) -> Option<String> {
    const FIELDS: &[&str] = &[
        "message",
        "error_description",
        "errorMessage",
        "description",
        "detail",
        "title",
        "reason",
        "error",
        "errors",
    ];

    let text = std::str::from_utf8(body).ok()?.trim();
    if text.is_empty() {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(message) = value.as_str() {
            return clean_message(message);
        }
        for field in FIELDS {
            if let Some(message) = value.get(field).and_then(find_message) {
                return clean_message(message);
            }
        }
        return None;
    }

    if text.starts_with('<') {
        return None;
    }
    clean_message(text)
}

fn find_message(value: &serde_json::Value) -> Option<&str> {
    const NESTED_FIELDS: &[&str] = &[
        "message",
        "error_description",
        "errorMessage",
        "description",
        "detail",
        "title",
        "reason",
    ];

    if let Some(message) = value.as_str() {
        return Some(message);
    }
    if let Some(values) = value.as_array() {
        return values.iter().find_map(find_message);
    }
    NESTED_FIELDS
        .iter()
        .find_map(|field| value.get(field).and_then(find_message))
}

fn clean_message(message: &str) -> Option<String> {
    const MAX_CHARS: usize = 500;

    let mut cleaned = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return None;
    }
    if cleaned.chars().count() > MAX_CHARS {
        cleaned = cleaned.chars().take(MAX_CHARS).collect();
        cleaned.push('…');
    }
    Some(cleaned)
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::Error;

    #[test]
    fn extracts_json_http_error_messages() {
        let error = Error::http_response(
            StatusCode::FORBIDDEN,
            br#"{"error":{"message":"API key rejected"}}"#,
        );
        assert_eq!(
            error.to_string(),
            "http status 403 Forbidden: API key rejected"
        );
    }

    #[test]
    fn extracts_plain_text_http_error_messages() {
        let error = Error::http_response(
            StatusCode::SERVICE_UNAVAILABLE,
            b"  service temporarily unavailable\n",
        );
        assert_eq!(
            error.to_string(),
            "http status 503 Service Unavailable: service temporarily unavailable"
        );
    }

    #[test]
    fn extracts_provider_specific_and_validation_messages() {
        let modrinth = Error::http_response(
            StatusCode::BAD_REQUEST,
            br#"{"error":"invalid_input","description":"Unsupported loader"}"#,
        );
        assert_eq!(
            modrinth.to_string(),
            "http status 400 Bad Request: Unsupported loader"
        );

        let validation = Error::http_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            br#"{"errors":[{"detail":"Version is required"}]}"#,
        );
        assert_eq!(
            validation.to_string(),
            "http status 422 Unprocessable Entity: Version is required"
        );
    }

    #[test]
    fn ignores_html_error_pages() {
        let error = Error::http_response(
            StatusCode::BAD_GATEWAY,
            b"<html><title>upstream unavailable</title></html>",
        );
        assert_eq!(error.to_string(), "http status 502 Bad Gateway");
    }
}
