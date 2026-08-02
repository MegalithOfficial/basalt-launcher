use crate::{
    config::Instance,
    error::{Error, Result},
    state::AppState,
};

pub(crate) mod accounts;
pub(crate) mod app;
pub(crate) mod content_commands;
pub(crate) mod instances;
pub(crate) mod launch_commands;
pub(crate) mod logging_commands;
pub(crate) mod migrate_commands;
pub(crate) mod pack_commands;
pub(crate) mod skins;
pub(crate) mod snapshots;
pub(crate) mod tasks;
pub(crate) mod worlds;

fn find_instance(state: &AppState, instance_id: &str) -> Result<Instance> {
    state
        .db
        .list_instances(&state.files)?
        .into_iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| Error::NotFound(format!("instance {instance_id}")))
}
