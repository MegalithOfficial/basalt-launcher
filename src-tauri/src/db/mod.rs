use std::sync::{Arc, Mutex};

use rusqlite::Connection;

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

mod accounts;
mod cache;
mod content;
mod core;
mod instances;
mod migrations;
mod models;
mod operations;
mod settings;
mod skins;

pub use models::{CachedResponse, ContentFile, ContentUpdate, PendingOperation, SkinRecord};

use migrations::{migrate, SCHEMA_VERSION};
