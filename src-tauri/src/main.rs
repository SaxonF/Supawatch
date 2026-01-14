// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod defaults;
mod diff;
mod fns;
mod generator;
mod introspection;
mod models;
mod parsing;
mod schema;
mod state;
mod supabase_api;
mod sync;
mod tray;
mod watcher;

use std::sync::Arc;

use state::AppState;
use tauri::Manager;

fn main() {
    let app_state = Arc::new(AppState::new());

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            // Init commands
            commands::init,
            commands::pick_project_folder,
            // Access token commands
            commands::set_access_token,
            commands::has_access_token,
            commands::clear_access_token,
            commands::validate_access_token,
            // Remote project commands
            commands::list_remote_projects,
            commands::list_organizations,
            commands::pull_project,
            commands::push_project,
            // Project commands
            commands::create_project,
            commands::get_projects,
            commands::get_project,
            commands::update_project,
            commands::delete_project,
            commands::link_supabase_project,
            commands::reveal_in_finder,
            // Template commands
            commands::templates::is_folder_empty,
            commands::templates::get_templates,
            commands::templates::copy_template,
            // Watcher commands
            commands::start_watching,
            commands::stop_watching,
            commands::is_watching,
            // Log commands
            commands::get_logs,
            commands::clear_logs,
            // Supabase API commands
            commands::run_query,
            commands::deploy_edge_function,
            commands::get_remote_schema,
            // Supabase Logs API commands
            commands::query_supabase_logs,
            commands::get_edge_function_logs,
            commands::get_postgres_logs,
            commands::get_auth_logs,
        ])
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_handle = app.app_handle();

            // Create tray icon for sync status indicator
            tray::create(app_handle)?;

            // Restart watchers for projects that were being watched
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle_clone.state::<Arc<AppState>>();
                let projects = state.get_projects().await;

                for project in projects {
                    if project.is_watching {
                        if let Err(e) = watcher::start_watching(&app_handle_clone, project.id, &project.local_path).await {
                           eprintln!("Failed to restart watcher for {}: {}", project.name, e);
                        }
                    }
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|_app_handle, _event| {});
}
