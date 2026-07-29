use std::time::Duration;

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, State};

use crate::{
    auth::{
        account::{Account, AccountView},
        microsoft::{self, PollOutcome},
    },
    config::{Instance, LauncherSettings},
    content::{self, ContentItem},
    db::Db,
    error::{Error, Result},
    install,
    java::{self, JavaStatus},
    launch::{
        self,
        process::{LogLine, RunningInfo},
    },
    loaders,
    logging::{self, LogConfig, LogRecord, LogState},
    meta::{
        manifest::{self, VersionEntry},
        media::{self, VersionMedia},
    },
    search,
    skin::{self, Appearance, SkinEntry},
    state::AppState,
    sysinfo_probe::{self, SystemStats, SystemUsage},
    update::{self, UpdateInfo},
};

pub(crate) mod accounts;
pub(crate) mod app;
pub(crate) mod content_commands;
pub(crate) mod instances;
pub(crate) mod launch_commands;
pub(crate) mod logging_commands;
pub(crate) mod skins;
pub(crate) mod tasks;

fn find_instance(state: &AppState, instance_id: &str) -> Result<Instance> {
    state
        .db
        .list_instances(&state.files)?
        .into_iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| Error::NotFound(format!("instance {instance_id}")))
}
