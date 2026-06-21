use serde::{Serialize, Serializer};

pub type Result<T> = std::result::Result<T, Error>;

/// Unified error type for the launcher backend. Implements `Serialize` so it can be
/// returned directly from Tauri commands and surfaced to the frontend as a string.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("checksum mismatch for {path}: expected {expected}, got {actual}")]
    Checksum {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
