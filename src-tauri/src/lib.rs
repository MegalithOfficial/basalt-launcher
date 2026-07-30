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
mod migrate;
mod modpack;
mod network;
mod paths;
mod search;
mod skin;
mod state;
mod sysinfo_probe;
mod tasks;
mod update;
mod worlds;

use files::FileManager;
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
            let files = FileManager::new(paths.clone())?;
            files.ensure_base_dirs()?;

            let log_state = logging::init(app.handle(), &files, logging::DEFAULT_LEVEL)?;
            tracing::info!(
                version = env!("CARGO_PKG_VERSION"),
                data_dir = %paths.root.display(),
                log_file = %logging::log_file(&files).display(),
                "basalt starting"
            );
            app.manage(log_state);

            let db = db::Db::open(&files)?;
            match db.load_settings() {
                Ok(settings) => {
                    if let Err(e) = logging::set_level(&settings.log_level) {
                        tracing::warn!(error = %e, "could not apply stored log level");
                    }
                }
                Err(e) => tracing::warn!(error = %e, "could not read settings for log level"),
            }

            let state = AppState::new(files, db);
            if let Err(e) = skin::reconcile_library(&state) {
                tracing::warn!(error = %e, "could not reconcile the skin library");
            }
            if let Err(e) = launch::process::recover_processes(
                app.handle(),
                &state.running,
                &state.files,
                &state.db,
            ) {
                tracing::warn!(error = %e, "could not recover running game processes");
            }
            app.manage(state);
            tracing::info!("startup complete");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_settings,
            commands::app::update_settings,
            commands::app::get_app_info,
            commands::app::list_javas,
            commands::instances::list_instances,
            commands::instances::create_instance,
            commands::instances::update_instance,
            commands::instances::delete_instance,
            commands::instances::list_versions,
            commands::instances::list_loader_versions,
            commands::instances::list_installed_versions,
            commands::instances::get_instance_media,
            commands::instances::set_instance_banner,
            commands::instances::clear_instance_banner,
            commands::instances::set_instance_logo,
            commands::instances::clear_instance_logo,
            commands::instances::backfill_pack_logos,
            commands::tasks::list_tasks,
            commands::tasks::clear_finished_tasks,
            commands::tasks::cancel_task,
            commands::tasks::recover_interrupted,
            commands::instances::install_instance,
            commands::instances::get_java_status,
            commands::accounts::auth_begin,
            commands::accounts::list_accounts,
            commands::accounts::set_active_account,
            commands::accounts::remove_account,
            commands::content_commands::search_content,
            commands::content_commands::get_project_details,
            commands::content_commands::list_project_versions,
            commands::content_commands::get_version_changelog,
            commands::content_commands::resolve_projects,
            commands::content_commands::get_installed_project_file,
            commands::content_commands::install_content,
            commands::content_commands::install_modpack,
            commands::content_commands::plan_content_install,
            commands::content_commands::get_filter_taxonomy,
            commands::content_commands::check_content_updates,
            commands::content_commands::get_content_updates,
            commands::content_commands::apply_content_update,
            commands::content_commands::get_content_dependents,
            commands::content_commands::plan_content_removal,
            commands::content_commands::list_instance_content,
            commands::content_commands::list_instance_content_bundle,
            commands::content_commands::toggle_instance_content,
            commands::content_commands::delete_instance_content,
            commands::content_commands::add_instance_content,
            commands::worlds::list_instance_worlds,
            commands::worlds::inspect_world_source,
            commands::worlds::import_worlds,
            commands::worlds::delete_instance_world,
            commands::migrate_commands::detect_launchers,
            commands::migrate_commands::scan_launcher,
            commands::migrate_commands::migrate_instances,
            commands::launch_commands::launch_instance,
            commands::launch_commands::kill_instance,
            commands::launch_commands::list_running,
            commands::launch_commands::get_logs,
            commands::launch_commands::close_running,
            commands::logging_commands::get_log_records,
            commands::logging_commands::clear_log_records,
            commands::logging_commands::get_log_config,
            commands::logging_commands::set_log_level,
            commands::logging_commands::frontend_log,
            commands::logging_commands::list_instance_logs,
            commands::logging_commands::search_instance_log,
            commands::logging_commands::delete_instance_log,
            commands::app::test_network,
            commands::app::check_for_updates,
            commands::app::get_about_links,
            commands::app::get_system_stats,
            commands::app::get_system_usage,
            commands::app::preview_launch_args,
            commands::skins::get_appearance,
            commands::skins::list_skins,
            commands::skins::add_skin_from_file,
            commands::skins::add_skin_from_reference,
            commands::skins::delete_skin,
            commands::skins::rename_skin,
            commands::skins::get_worn_skin,
            commands::skins::apply_saved_skin,
            commands::skins::reset_skin,
            commands::skins::set_cape,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
