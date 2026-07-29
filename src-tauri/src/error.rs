use serde::{Serialize, Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

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
