use crate::tasks::TaskKind;

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstanceGroup {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstancePlacement {
    pub instance_id: String,
    pub group_id: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstanceOrganization {
    pub groups: Vec<InstanceGroup>,
    pub placements: Vec<InstancePlacement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveRun {
    pub running_id: String,
    pub instance_id: String,
    pub pid: u32,
    pub process_started_at: u64,
    pub started_at: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BannerRecord {
    pub id: String,
    pub file_name: String,
    pub original_name: Option<String>,
    pub kind: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bytes: i64,
    pub accent: Option<String>,
    pub added_at: i64,
}

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

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
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
    pub age_secs: i64,
}
