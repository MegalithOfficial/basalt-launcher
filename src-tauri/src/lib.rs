mod commands;
mod config;
mod download;
mod error;
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
            commands::list_versions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
