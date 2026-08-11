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
    pub checkpointed_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct ActiveServerRun {
    pub running_id: String,
    pub server_id: String,
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PlaySession {
    pub id: i64,
    pub instance_id: String,
    pub instance_name: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub played_secs: i64,
    pub crashed: bool,
    pub version_id: Option<String>,
    pub loader: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DayBucket {
    pub date: String,
    pub secs: i64,
    pub sessions: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct InstancePlayStat {
    pub instance_id: String,
    pub name: String,
    pub secs: i64,
    pub sessions: i64,
    pub crashes: i64,
    pub last_played_at: Option<i64>,
    pub lifetime_secs: i64,
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LoaderPlayStat {
    pub loader: String,
    pub secs: i64,
    pub sessions: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PlayStats {
    pub lifetime_secs: i64,
    pub tracked_since: Option<i64>,
    pub window_days: Option<u32>,
    pub window_secs: i64,
    pub session_count: i64,
    pub crash_count: i64,
    pub longest_session_secs: i64,
    pub average_session_secs: i64,
    pub active_days: i64,
    pub current_streak_days: i64,
    pub longest_streak_days: i64,
    pub busiest_day: Option<DayBucket>,
    pub daily: Vec<DayBucket>,
    pub hourly: Vec<i64>,
    pub weekday: Vec<i64>,
    pub instances: Vec<InstancePlayStat>,
    pub loaders: Vec<LoaderPlayStat>,
    pub recent: Vec<PlaySession>,
    pub recent_total: i64,
    pub recent_page: Option<u32>,
}
