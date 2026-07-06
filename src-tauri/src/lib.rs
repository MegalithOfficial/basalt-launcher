mod auth;
mod commands;
mod config;
mod download;
mod error;
mod install;
mod java;
mod launch;
mod meta;
mod paths;
mod state;

use paths::Paths;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let paths = Paths::resolve(app.handle())?;
            paths.ensure_dirs()?;
            let settings = config::load_settings(&paths)?;
            app.manage(AppState::new(paths, settings));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::list_instances,
            commands::create_instance,
            commands::delete_instance,
            commands::list_versions,
            commands::list_installed_versions,
            commands::install_instance,
            commands::get_java_status,
            commands::auth_begin,
            commands::list_accounts,
            commands::set_active_account,
            commands::remove_account,
            commands::launch_instance,
            commands::kill_instance,
            commands::list_running,
            commands::get_logs,
            commands::close_running,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
