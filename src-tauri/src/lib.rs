mod auth;
mod commands;
mod config;
mod db;
mod download;
mod error;
mod install;
mod java;
mod launch;
mod loaders;
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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png"))?;
                let _ = window.set_icon(icon);
            }
            let paths = Paths::resolve(app.handle())?;
            paths.ensure_dirs()?;
            let db = db::Db::open(&paths)?;
            app.manage(AppState::new(paths, db));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::list_instances,
            commands::create_instance,
            commands::update_instance,
            commands::delete_instance,
            commands::list_versions,
            commands::list_loader_versions,
            commands::list_installed_versions,
            commands::get_instance_media,
            commands::set_instance_banner,
            commands::clear_instance_banner,
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
