use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};
use once_cell::sync::Lazy;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::{FileChange, FileChangeType, LogEntry, LogSource};
use crate::state::AppState;
use crate::sync;
use crate::tray::update_icon;

// Track last push time per project to debounce rapid file changes
static PUSH_DEBOUNCE: Lazy<Mutex<HashMap<Uuid, Instant>>> = Lazy::new(|| Mutex::new(HashMap::new()));
const PUSH_DEBOUNCE_SECS: u64 = 2;

pub async fn start_watching(
    app_handle: &AppHandle,
    project_id: Uuid,
    local_path: &str,
) -> Result<(), String> {
    let path = Path::new(local_path);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", local_path));
    }

    let app_handle_for_closure = app_handle.clone();
    let app_handle_for_state = app_handle.clone();
    let local_path_for_closure = local_path.to_string();
    let local_path_for_log = local_path.to_string();

    // Create debouncer with 500ms debounce time
    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        move |result: Result<Vec<DebouncedEvent>, notify::Error>| {
            match result {
                Ok(events) => {
                    for event in events {
                        handle_file_event(&app_handle_for_closure, project_id, &local_path_for_closure, event);
                    }
                }
                Err(e) => {
                    eprintln!("Watch error: {:?}", e);
                }
            }
        },
    )
    .map_err(|e| format!("Failed to create watcher: {}", e))?;

    // Watch the directory recursively
    debouncer
        .watcher()
        .watch(path, RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to watch path: {}", e))?;

    // Store the watcher handle
    let state = app_handle_for_state.state::<Arc<AppState>>();
    
    state.add_watcher(project_id, debouncer).await;
    state.set_project_watching(project_id, true).await.ok();

    let log = LogEntry::info(
        Some(project_id),
        LogSource::Watcher,
        format!("Started watching: {}", local_path_for_log),
    );
    state.add_log(log.clone()).await;
    app_handle_for_state.emit("log", &log).ok();

    Ok(())
}

pub async fn stop_watching(app_handle: &AppHandle, project_id: Uuid) -> Result<(), String> {
    let state = app_handle.state::<Arc<AppState>>();

    state.stop_watcher(project_id).await;
    state.set_project_watching(project_id, false).await.ok();

    let log = LogEntry::info(
        Some(project_id),
        LogSource::Watcher,
        "Stopped watching".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    Ok(())
}

fn handle_file_event(
    app_handle: &AppHandle,
    project_id: Uuid,
    base_path: &str,
    event: DebouncedEvent,
) {
    let path = event.path;
    let path_str = path.to_string_lossy().to_string();

    // Determine the type of change based on the file path
    let change_type = classify_file_change(&path_str, base_path);

    // Skip if it's not a file we care about
    if change_type == FileChangeType::Other {
        return;
    }

    let file_change = FileChange::new(path_str.clone(), change_type.clone(), project_id);

    let state = app_handle.state::<Arc<AppState>>();
    let state_arc = state.inner().clone();
    let app_handle_clone = app_handle.clone();

    // Prepare log entry synchronously to avoid moving complex references into async block
    let log = match &change_type {
        FileChangeType::Schema => Some(LogEntry::info(
            Some(project_id),
            LogSource::Schema,
            format!("Schema file changed: {}", get_relative_path(&path_str, base_path)),
        )),
        FileChangeType::EdgeFunction => Some(LogEntry::info(
            Some(project_id),
            LogSource::EdgeFunction,
            format!(
                "Edge function changed: {}",
                get_relative_path(&path_str, base_path)
            ),
        )),
        FileChangeType::Migration => Some(LogEntry::info(
            Some(project_id),
            LogSource::Schema,
            format!(
                "Migration file changed: {}",
                get_relative_path(&path_str, base_path)
            ),
        )),
        FileChangeType::AdminConfig => Some(LogEntry::info(
            Some(project_id),
            LogSource::System,
            format!(
                "Admin config changed: {}",
                get_relative_path(&path_str, base_path)
            ),
        )),
        FileChangeType::Other => None,
    };

    if let Some(log) = log {
        let log_clone = log.clone();
        let state_for_log = state_arc.clone();
        let app_for_log = app_handle_clone.clone();
        tauri::async_runtime::spawn(async move {
            state_for_log.add_log(log_clone.clone()).await;
            app_for_log.emit("log", &log_clone).ok();
        });
    }

    // Emit the file change event to the frontend
    app_handle.emit("file_change", &file_change).ok();

    // Auto-push for schema changes
    if change_type == FileChangeType::Schema {
        let state_for_schema = state_arc.clone();
        let app_for_schema = app_handle_clone.clone();
        let base_path_for_ts = base_path.to_string();
        tauri::async_runtime::spawn(async move {
            // Generate TypeScript types first (doesn't need Supabase connection)
            if let Err(e) = handle_typescript_generation(&state_for_schema, &app_for_schema, project_id, &base_path_for_ts).await {
                eprintln!("TypeScript generation failed: {}", e);
            }

            // Then push to Supabase
            if let Err(e) = handle_schema_push(state_for_schema, app_for_schema, project_id).await {
                eprintln!("Auto-push failed: {}", e);
            }
        });
    }

    // Auto-deploy for edge function changes
    if change_type == FileChangeType::EdgeFunction {
        let path_for_deploy = path_str.clone();
        let base_for_deploy = base_path.to_string();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = handle_edge_function_push(state_arc, app_handle_clone, project_id, &path_for_deploy, &base_for_deploy).await {
                eprintln!("Edge function auto-deploy failed: {}", e);
            }
        });
    }

    // Notify frontend of admin config changes
    if change_type == FileChangeType::AdminConfig {
        #[derive(serde::Serialize, Clone)]
        struct AdminConfigChangedPayload {
            project_id: Uuid,
        }
        app_handle.emit("admin_config_changed", AdminConfigChangedPayload { project_id }).ok();
    }
}

async fn handle_schema_push(
    state: Arc<AppState>,
    app_handle: AppHandle,
    project_id: Uuid,
) -> Result<(), String> {
    // Check debounce - skip if we pushed recently
    {
        let mut debounce = PUSH_DEBOUNCE.lock().await;
        let now = Instant::now();
        if let Some(last_push) = debounce.get(&project_id) {
            if now.duration_since(*last_push) < Duration::from_secs(PUSH_DEBOUNCE_SECS) {
                println!("[DEBUG] Skipping duplicate push for project {} (debounced)", project_id);
                return Ok(());
            }
        }
        debounce.insert(project_id, now);
    }

    // Get project details
    let project = state.get_project(project_id).await.map_err(|e| e.to_string())?;
    
    let project_ref = match &project.supabase_project_ref {
        Some(r) => r.clone(),
        None => {
            let log = LogEntry::warning(
                Some(project_id),
                LogSource::Schema,
                "Auto-push skipped: project not linked to Supabase".to_string(),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
            return Ok(());
        }
    };

    // Get API client
    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    update_icon(&app_handle, true);

    let log = LogEntry::info(
        Some(project_id),
        LogSource::Schema,
        "Auto-pushing schema changes...".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // Find schema path using shared sync module
    let schema_path = match sync::find_schema_path(Path::new(&project.local_path)) {
        Some(p) => p,
        None => {
            let log = LogEntry::error(
                Some(project_id),
                LogSource::Schema,
                "Schema file not found (checked supabase/schemas/schema.sql and supabase/schema.sql)".to_string(),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
            update_icon(&app_handle, false);
            return Err("Schema file not found".to_string());
        }
    };

    // Compute diff using shared sync module (introspect remote, parse local, compute diff)
    let diff_result = match sync::compute_schema_diff(&api, &project_ref, &schema_path).await {
        Ok(r) => r,
        Err(e) => {
            let log = LogEntry::error(
                Some(project_id),
                LogSource::Schema,
                format!("Failed to compute schema diff: {}", e),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
            update_icon(&app_handle, false);
            return Err(e);
        }
    };

    let diff = diff_result.diff;

    if diff.is_destructive() {
        let summary = diff.summarize();
        let log = LogEntry::warning(
            Some(project_id),
            LogSource::Schema,
            "Destructive changes detected. Waiting for user confirmation...".to_string(),
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();

        #[derive(serde::Serialize, Clone)]
        struct ConfirmationPayload {
            project_id: Uuid,
            summary: String,
        }

        app_handle.emit("schema-push-confirmation-needed", ConfirmationPayload {
            project_id,
            summary,
        }).ok();

        // Request user attention
        let _ = app_handle.get_webview_window("main").map(|w| w.request_user_attention(Some(tauri::UserAttentionType::Critical)));

        update_icon(&app_handle, false);
        return Ok(());
    }

    // Use migration SQL from diff result
    let migration_sql = &diff_result.migration_sql;

    if migration_sql.trim().is_empty() {
        let log = LogEntry::success(
            Some(project_id),
            LogSource::Schema,
            "No schema changes detected.".to_string(),
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
        update_icon(&app_handle, false);
        return Ok(());
    }

    let log = LogEntry::info(
        Some(project_id),
        LogSource::Schema,
        format!("Applying changes:\n{}", diff.summarize()),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // 5. Execute
    let result = api.run_query(&project_ref, &migration_sql, false).await.map_err(|e| e.to_string())?;

    if let Some(err) = result.error {
        let log = LogEntry::error(
            Some(project_id),
            LogSource::Schema,
            format!("Migration failed: {}", err),
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
        update_icon(&app_handle, false);
        return Err(err);
    }

    let log = LogEntry::success(
        Some(project_id),
        LogSource::Schema,
        "Schema changes pushed successfully.".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    update_icon(&app_handle, false);
    Ok(())
}

async fn handle_edge_function_push(
    state: Arc<AppState>,
    app_handle: AppHandle,
    project_id: Uuid,
    file_path: &str,
    base_path: &str,
) -> Result<(), String> {
    // Get project details
    let project = state.get_project(project_id).await.map_err(|e| e.to_string())?;
    
    let project_ref = match &project.supabase_project_ref {
        Some(r) => r.clone(),
        None => {
            let log = LogEntry::warning(
                Some(project_id),
                LogSource::EdgeFunction,
                "Auto-deploy skipped: project not linked to Supabase".to_string(),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
            return Ok(());
        }
    };

    // Extract function slug from path
    // Path like: /path/to/project/supabase/functions/my-function/index.ts
    // We want: my-function
    let relative = get_relative_path(file_path, base_path);
    let function_slug = extract_function_slug(&relative);
    
    if function_slug.is_empty() {
        let log = LogEntry::warning(
            Some(project_id),
            LogSource::EdgeFunction,
            format!("Could not determine function slug from path: {}", relative),
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
        return Ok(());
    }

    // Get API client
    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    update_icon(&app_handle, true);

    // Find function directory and collect ALL files
    let function_dir = std::path::Path::new(base_path)
        .join("supabase")
        .join("functions")
        .join(&function_slug);

    // Use shared sync module for file collection
    let files = match sync::collect_function_files(&function_dir).await {
        Ok(f) => f,
        Err(e) => {
            let log = LogEntry::error(
                Some(project_id),
                LogSource::EdgeFunction,
                format!("Failed to read function directory: {}", e),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
            update_icon(&app_handle, false);
            return Err(e);
        }
    };

    if files.is_empty() {
        let log = LogEntry::warning(
            Some(project_id),
            LogSource::EdgeFunction,
            format!("No files found in function directory: {}", function_slug),
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
        update_icon(&app_handle, false);
        return Ok(());
    }

    // Determine entrypoint using shared sync module
    let entrypoint = sync::determine_entrypoint(&files);

    // Compute hash of all files for tracking using shared sync module
    let local_hash = sync::compute_files_hash(&files);

    let log = LogEntry::info(
        Some(project_id),
        LogSource::EdgeFunction,
        format!("Deploying '{}' ({} files)...", function_slug, files.len()),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // Save hash path for later
    let hash_file = function_dir.join(".supawatch_hash");

    // Deploy with all files
    match api.deploy_function(&project_ref, &function_slug, &function_slug, &entrypoint, files).await {
        Ok(result) => {
            // Save hash after successful deploy
            let _ = tokio::fs::write(&hash_file, &local_hash).await;
            
            let log = LogEntry::success(
                Some(project_id),
                LogSource::EdgeFunction,
                format!("Deployed '{}' (v{})", result.name, result.version),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
        }
        Err(e) => {
            let log = LogEntry::error(
                Some(project_id),
                LogSource::EdgeFunction,
                format!("Deploy failed: {}", e),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
            update_icon(&app_handle, false);
            return Err(e.to_string());
        }
    }

    update_icon(&app_handle, false);
    Ok(())
}

async fn handle_typescript_generation(
    state: &Arc<AppState>,
    app_handle: &AppHandle,
    project_id: Uuid,
    base_path: &str,
) -> Result<(), String> {
    // Get project settings
    let project = match state.get_project(project_id).await {
        Ok(p) => p,
        Err(_) => return Ok(()), // Project not found, skip
    };

    // Check if TypeScript generation is enabled for this project
    if !project.generate_typescript {
        return Ok(());
    }

    let project_path = Path::new(base_path);

    // Find schema path
    let schema_path = match sync::find_schema_path(project_path) {
        Some(p) => p,
        None => {
            // No schema file found, skip TypeScript generation
            return Ok(());
        }
    };

    // Get TypeScript output path (use custom path if configured)
    let ts_output_path = sync::get_typescript_output_path(
        project_path,
        project.typescript_output_path.as_deref(),
    );

    let log = LogEntry::info(
        Some(project_id),
        LogSource::Schema,
        "Generating TypeScript types...".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // Generate TypeScript types
    match sync::generate_typescript_types(&schema_path, &ts_output_path).await {
        Ok(()) => {
            let relative_output = ts_output_path
                .strip_prefix(project_path)
                .unwrap_or(&ts_output_path)
                .to_string_lossy();
            let log = LogEntry::success(
                Some(project_id),
                LogSource::Schema,
                format!("TypeScript types generated: {}", relative_output),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
            Ok(())
        }
        Err(e) => {
            let log = LogEntry::error(
                Some(project_id),
                LogSource::Schema,
                format!("TypeScript generation failed: {}", e),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
            Err(e)
        }
    }
}

/// Extract function slug from a relative path like "supabase/functions/my-function/index.ts"
fn extract_function_slug(relative_path: &str) -> String {
    let parts: Vec<&str> = relative_path.split('/').collect();
    
    // Look for "functions" in the path and get the next part
    for (i, part) in parts.iter().enumerate() {
        if *part == "functions" && i + 1 < parts.len() {
            return parts[i + 1].to_string();
        }
    }
    
    String::new()
}

fn classify_file_change(path: &str, base_path: &str) -> FileChangeType {
    let relative = get_relative_path(path, base_path);
    let relative_lower = relative.to_lowercase();

    // Check for admin config file (supabase/admin.json or admin.json)
    if relative_lower == "supabase/admin.json" || relative_lower == "admin.json" {
        return FileChangeType::AdminConfig;
    }

    // Check for schema files (supabase/schema/*.sql, supabase/schemas/*.sql, or schema/*.sql)
    if (relative_lower.contains("/schema") || relative_lower.starts_with("schema"))
        && relative_lower.ends_with(".sql")
    {
        return FileChangeType::Schema;
    }

    // Check for edge functions (supabase/functions/* or functions/*)
    if (relative_lower.contains("/functions/") || relative_lower.starts_with("functions/"))
        && (relative_lower.ends_with(".ts") || relative_lower.ends_with(".js"))
    {
        return FileChangeType::EdgeFunction;
    }

    // Check for migrations (supabase/migrations/*.sql or migrations/*.sql)
    if (relative_lower.contains("/migrations/") || relative_lower.starts_with("migrations/"))
        && relative_lower.ends_with(".sql")
    {
        return FileChangeType::Migration;
    }

    FileChangeType::Other
}

fn get_relative_path(path: &str, base_path: &str) -> String {
    path.strip_prefix(base_path)
        .unwrap_or(path)
        .trim_start_matches('/')
        .to_string()
}
