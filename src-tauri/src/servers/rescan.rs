use std::path::PathBuf;

use crate::{db::Db, error::Result};

use super::{import, Server};

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Rescan {
    pub changed: bool,
    pub launch_ready: bool,
    pub software: Option<String>,
    pub version_id: Option<String>,
    pub flavor_version: Option<String>,
}

pub fn run(db: &Db, server: &Server) -> Result<Rescan> {
    let dir = PathBuf::from(&server.dir);
    if !dir.is_dir() {
        return Ok(Rescan::default());
    }

    let found = import::inspect(&dir);
    let runnable = found.launch_jar.is_some() || !found.launch_argfiles.is_empty();
    if !runnable {
        return Ok(Rescan::default());
    }

    let software = found.flavor.filter(|found| *found != server.flavor);
    let version_id = found
        .version_id
        .filter(|found| !found.is_empty() && *found != server.version_id);
    let flavor_version = found
        .flavor_version
        .filter(|found| Some(found.as_str()) != server.flavor_version.as_deref());
    let launch_changed =
        found.launch_jar != server.launch_jar || found.launch_argfiles != server.launch_argfiles;

    if software.is_none() && version_id.is_none() && flavor_version.is_none() && !launch_changed {
        return Ok(Rescan {
            launch_ready: true,
            ..Rescan::default()
        });
    }

    if let Some(software) = software {
        db.set_server_software(&server.id, software.id())?;
    }
    if let Some(version) = &version_id {
        db.set_server_version(&server.id, version)?;
    }
    if launch_changed || flavor_version.is_some() {
        db.set_server_launch(
            &server.id,
            found.launch_jar.as_deref(),
            &found.launch_argfiles,
            flavor_version
                .as_deref()
                .or(server.flavor_version.as_deref()),
            server
                .installed_at
                .unwrap_or_else(|| chrono::Utc::now().timestamp()),
        )?;
    }

    tracing::info!(
        server_id = %server.id,
        software = ?software.map(|found| found.id()),
        version = ?version_id,
        loader = ?flavor_version,
        "the folder now says something different about this server"
    );

    Ok(Rescan {
        changed: true,
        launch_ready: true,
        software: software.map(|found| found.id().to_string()),
        version_id,
        flavor_version,
    })
}
