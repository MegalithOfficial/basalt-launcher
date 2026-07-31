use tauri::State;

use crate::{error::Result, state::AppState, tasks::TaskKind};

#[tauri::command]
pub fn list_tasks(state: State<AppState>) -> Vec<crate::tasks::Task> {
    state.tasks.list()
}

#[tauri::command]
pub fn clear_finished_tasks(state: State<AppState>) {
    state.tasks.clear_finished();
}

#[tauri::command]
pub fn cancel_task(state: State<AppState>, task_id: String) -> bool {
    state.tasks.cancel(&task_id)
}

fn sweep_partials(files: &crate::files::FileManager, dir: &std::path::Path) -> usize {
    let Ok(entries) = files.read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for path in entries {
        if files
            .metadata(&path)
            .is_ok_and(|metadata| metadata.is_dir())
        {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(".basalt-import-"))
            {
                if files.remove_managed_dir_all_if_exists(&path).is_ok() {
                    removed += 1;
                }
            } else {
                removed += sweep_partials(files, &path);
            }
        } else if path.extension().is_some_and(|e| e == "part")
            && files.remove_file_if_exists(&path).is_ok()
        {
            removed += 1;
        }
    }
    removed
}

#[tauri::command]
pub fn recover_interrupted(state: State<AppState>) -> Result<Vec<crate::db::PendingOperation>> {
    let pending = state.db.pending_operations()?;
    let removed_partials = sweep_partials(&state.files, &state.paths.root);
    if removed_partials > 0 {
        tracing::info!(removed_partials, "removed interrupted download files");
    }
    if pending.is_empty() {
        return Ok(Vec::new());
    }

    for op in &pending {
        let Some(instance_id) = op.instance_id.as_deref() else {
            continue;
        };
        if op.kind == TaskKind::ModpackInstall {
            let _ = state.db.delete_instance_content_files(instance_id);
            let _ = state.db.delete_instance(instance_id);
            let _ = state.files.remove_instance_dir(instance_id);
        }
    }

    state.db.clear_pending_operations()?;
    Ok(pending)
}
