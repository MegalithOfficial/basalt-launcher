use std::sync::{Arc, Mutex};

use rusqlite::Connection;

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

mod accounts;
mod banners;
mod cache;
mod content;
mod core;
mod datapacks;
mod groups;
mod instances;
mod migrations;
mod models;
mod operations;
mod runs;
mod servers;
mod settings;
mod skins;
mod stats;

pub use datapacks::DatapackRecord;
pub use models::{
    ActiveRun, ActiveServerRun, BannerRecord, CachedResponse, ContentFile, ContentUpdate,
    DayBucket, InstanceGroup, InstanceOrganization, InstancePlacement, InstancePlayStat,
    LoaderPlayStat, PendingOperation, PlaySession, PlayStats, SkinRecord,
};

use migrations::{migrate, SCHEMA_VERSION};
