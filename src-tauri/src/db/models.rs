#[derive(Debug, Clone, serde::Serialize)]
pub struct SkinRecord {
    pub id: String,
    pub name: String,
    pub variant: String,
    pub file_name: String,
    pub source: Option<String>,
    pub hash: Option<String>,
    pub remote_hash: Option<String>,
    pub added_at: i64,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ContentFile {
    pub file_name: String,
    pub sha1: Option<String>,
    pub sha512: Option<String>,
    pub murmur2: Option<i64>,
    pub provider: Option<String>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub title: Option<String>,
    pub icon_url: Option<String>,
    pub mod_id: Option<String>,
    pub mod_version: Option<String>,
    pub dependencies: Option<String>,
    pub origin: String,
    pub pack_version_id: Option<String>,
    pub installed_at: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PendingOperation {
    pub id: String,
    pub kind: TaskKind,
    pub instance_id: Option<String>,
    pub title: String,
    pub payload: Option<String>,
    pub started_at: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContentUpdate {
    pub kind: String,
    pub file_name: String,
    pub latest_version_id: String,
    pub latest_name: String,
    pub latest_file_name: String,
}

pub struct CachedResponse {
    pub body: String,
    pub etag: Option<String>,
    pub fresh: bool,
}
use crate::tasks::TaskKind;
