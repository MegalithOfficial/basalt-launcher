mod auth;
mod build_info;
mod commands;
mod config;
mod content;
mod db;
mod download;
mod error;
mod files;
mod install;
mod java;
mod launch;
mod loaders;
mod logging;
mod meta;
mod modpack;
mod network;
mod paths;
mod search;
mod skin;
mod state;
mod sysinfo_probe;
mod tasks;
mod update;

use paths::Paths;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_window::init_with(
            tauri_plugin_single_window::Config::new().with_target_window("main"),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png"))?;
                let _ = window.set_icon(icon);
            }
            let paths = Paths::resolve(app.handle())?;
            paths.ensure_dirs()?;

            let log_state = logging::init(app.handle(), &paths, logging::DEFAULT_LEVEL)?;
            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                data_dir = %paths.root.display(),
                log_file = %logging::log_file(&paths).display(),
                "basalt starting"
            );
            app.manage(log_state);

            let db = db::Db::open(&paths)?;
            match db.load_settings() {
                Ok(settings) => {
                    if let Err(e) = logging::set_level(&settings.log_level) {
                        tracing::warn!(error = %e, "could not apply stored log level");
                    }
                }
                Err(e) => tracing::warn!(error = %e, "could not read settings for log level"),
            }

            let state = AppState::new(paths, db);
            if let Err(e) = skin::reconcile_library(&state) {
                tracing::warn!(error = %e, "could not reconcile the skin library");
            }
            app.manage(state);
            tracing::info!("startup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::get_app_info,
            commands::list_javas,
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
            commands::set_instance_logo,
            commands::clear_instance_logo,
            commands::backfill_pack_logos,
            commands::list_tasks,
            commands::clear_finished_tasks,
            commands::cancel_task,
            commands::recover_interrupted,
            commands::install_instance,
            commands::get_java_status,
            commands::auth_begin,
            commands::list_accounts,
            commands::set_active_account,
            commands::remove_account,
            commands::search_content,
            commands::get_project_details,
            commands::list_project_versions,
            commands::get_version_changelog,
            commands::resolve_projects,
            commands::get_installed_project_file,
            commands::install_content,
            commands::install_modpack,
            commands::plan_content_install,
            commands::get_filter_taxonomy,
            commands::check_content_updates,
            commands::get_content_updates,
            commands::apply_content_update,
            commands::get_content_dependents,
            commands::list_instance_content,
            commands::toggle_instance_content,
            commands::delete_instance_content,
            commands::add_instance_content,
            commands::launch_instance,
            commands::kill_instance,
            commands::list_running,
            commands::get_logs,
            commands::close_running,
            commands::get_log_records,
            commands::clear_log_records,
            commands::get_log_config,
            commands::set_log_level,
            commands::frontend_log,
            commands::check_for_updates,
            commands::get_about_links,
            commands::get_system_stats,
            commands::get_system_usage,
            commands::preview_launch_args,
            commands::get_appearance,
            commands::list_skins,
            commands::add_skin_from_file,
            commands::add_skin_from_reference,
            commands::delete_skin,
            commands::rename_skin,
            commands::get_worn_skin,
            commands::apply_saved_skin,
            commands::reset_skin,
            commands::set_cape,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
