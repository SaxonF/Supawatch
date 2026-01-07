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
        tauri::async_runtime::spawn(async move {
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

    // 1. Introspect Remote
    let introspector = crate::introspection::Introspector::new(&api, project_ref.clone());
    let remote_schema = match introspector.introspect().await {
        Ok(schema) => schema,
        Err(e) => {
            let log = LogEntry::error(
                Some(project_id),
                LogSource::Schema,
                format!("Introspection failed: {}", e),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
            update_icon(&app_handle, false);
            return Err(e);
        }
    };

    // 2. Parse Local - try multiple schema paths
    let schema_paths = [
        std::path::Path::new(&project.local_path).join("supabase/schemas/schema.sql"),
        std::path::Path::new(&project.local_path).join("supabase/schema.sql"),
    ];
    
    let schema_path = schema_paths.iter().find(|p| p.exists());
    
    let schema_path = match schema_path {
        Some(p) => p.clone(),
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

    let local_sql = tokio::fs::read_to_string(&schema_path).await.map_err(|e| e.to_string())?;
    let local_schema = match crate::parsing::parse_schema_sql(&local_sql) {
        Ok(schema) => schema,
        Err(e) => {
            let log = LogEntry::error(
                Some(project_id),
                LogSource::Schema,
                format!("Failed to parse schema: {}", e),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
            update_icon(&app_handle, false);
            return Err(e);
        }
    };

    // 3. Diff (Remote -> Local)
    let diff = crate::diff::compute_diff(&remote_schema, &local_schema);

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

    // 4. Generate Migration SQL
    let migration_sql = crate::generator::generate_sql(&diff, &local_schema);

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

    let files = match collect_function_files(&function_dir).await {
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

    // Determine entrypoint (as owned String to avoid borrow conflict)
    let entrypoint = if files.iter().any(|(p, _)| p == "index.ts") {
        "index.ts".to_string()
    } else if files.iter().any(|(p, _)| p == "index.js") {
        "index.js".to_string()
    } else {
        files.first().map(|(p, _)| p.clone()).unwrap_or_else(|| "index.ts".to_string())
    };

    // Compute hash of all files for tracking
    let local_hash = compute_files_hash_watcher(&files);

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

/// Collect all files in a function directory recursively
async fn collect_function_files(dir: &std::path::Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut files = Vec::new();
    collect_files_recursive(dir, dir, &mut files).await?;
    Ok(files)
}

#[async_recursion::async_recursion]
async fn collect_files_recursive(
    base: &std::path::Path,
    current: &std::path::Path,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), String> {
    let mut entries = tokio::fs::read_dir(current).await.map_err(|e| e.to_string())?;
    
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        
        if path.is_dir() {
            // Skip node_modules and hidden directories
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "node_modules" || name.starts_with('.') {
                continue;
            }
            collect_files_recursive(base, &path, files).await?;
        } else {
            // Only include source files
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "ts" | "js" | "json" | "tsx" | "jsx" | "mts" | "mjs") {
                let relative = path.strip_prefix(base)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .to_string();
                let content = tokio::fs::read(&path).await.map_err(|e| e.to_string())?;
                files.push((relative, content));
            }
        }
    }
    
    Ok(())
}

/// Compute a hash of all files for change detection
fn compute_files_hash_watcher(files: &[(String, Vec<u8>)]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    
    // Sort files by path for deterministic ordering
    let mut sorted_files: Vec<_> = files.iter().collect();
    sorted_files.sort_by(|a, b| a.0.cmp(&b.0));
    
    for (path, content) in sorted_files {
        path.hash(&mut hasher);
        content.hash(&mut hasher);
    }
    
    format!("{:x}", hasher.finish())
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
