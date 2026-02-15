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
use tauri::{
    menu::{Menu, MenuItemBuilder, SubmenuBuilder},
    Emitter, Manager,
};

fn main() {
    let app_state = Arc::new(AppState::new());

    let mut builder = tauri::Builder::default();

    // Single instance plugin (desktop only) - must be registered first
    // This ensures deep links are forwarded to the existing instance
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // When a new instance is opened with arguments (including deep links),
            // the deep-link plugin handles forwarding the URL to on_open_url
            println!("New app instance opened with args: {:?}", argv);

            // Focus the main window
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }));
    }

    builder
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
            // OpenAI key commands
            commands::set_openai_key,
            commands::has_openai_key,
            commands::clear_openai_key,
            // Remote project commands
            commands::list_remote_projects,
            commands::list_organizations,
            commands::pull_project,
            commands::get_pull_diff,
            commands::push_project,
            commands::get_project_diff,
            // Project commands
            commands::create_project,
            commands::get_projects,
            commands::get_project,
            commands::update_project,
            commands::delete_project,
            commands::link_supabase_project,
            commands::get_project_keys,
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
            // Admin config commands
            commands::has_admin_config,
            commands::get_sidebar_spec,
            commands::write_sidebar_spec,
            commands::add_sidebar_item,
            commands::add_sidebar_group,
            // Supabase API commands
            commands::run_query,
            commands::deploy_edge_function,
            commands::get_remote_schema,
            commands::run_seeds,
            commands::get_seed_content,
            // Supabase Logs API commands
            commands::query_supabase_logs,
            commands::get_edge_function_logs,
            commands::get_postgres_logs,
            commands::get_auth_logs,
            // SQL validation and AI commands
            commands::validate_sql,
            commands::convert_with_ai,
            commands::split_schema,
        ])
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let app_handle = app.app_handle();

            // Create application menu
            let import_template = MenuItemBuilder::new("Import Template...")
                .id("import-template")
                .accelerator("CmdOrCtrl+Shift+I")
                .build(app)?;

            let file_menu = SubmenuBuilder::new(app, "File")
                .item(&import_template)
                .separator()
                .close_window()
                .build()?;

            let edit_menu = SubmenuBuilder::new(app, "Edit")
                .undo()
                .redo()
                .separator()
                .cut()
                .copy()
                .paste()
                .select_all()
                .build()?;

            let menu = Menu::with_items(app, &[&file_menu, &edit_menu])?;

            app.set_menu(menu)?;

            // Handle menu events
            app.on_menu_event(move |app_handle, event| {
                if event.id().as_ref() == "import-template" {
                    let _ = app_handle.emit("menu-import-template", ());
                }
            });

            // Create tray icon for sync status indicator
            tray::create(app_handle)?;

            // Register deep link scheme at runtime for development (Linux/Windows)
            #[cfg(any(target_os = "linux", all(debug_assertions, windows)))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                if let Err(e) = app.deep_link().register_all() {
                    eprintln!("Failed to register deep link schemes: {}", e);
                }
            }

            // Handle deep link URLs - emit to frontend for processing
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let handle = app_handle.clone();

                // Check if app was started via deep link
                if let Ok(Some(urls)) = app.deep_link().get_current() {
                    for url in urls {
                        println!("App started with deep link: {}", url);
                        let _ = handle.emit("deep-link-received", url.to_string());
                    }
                }

                // Listen for deep links while app is running
                app.deep_link().on_open_url(move |event| {
                    for url in event.urls() {
                        println!("Deep link received: {}", url);
                        let _ = handle.emit("deep-link-received", url.to_string());
                    }
                });
            }

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
