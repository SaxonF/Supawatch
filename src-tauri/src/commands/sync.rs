use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::models::{LogEntry, LogSource, Project};
use crate::state::AppState;
use crate::sync;
use crate::tray::update_icon;

#[tauri::command]
pub async fn pull_project(
    app_handle: AppHandle,
    project_id: String,
) -> Result<String, String> {
    update_icon(&app_handle, true);
    let result = pull_project_internal(&app_handle, project_id).await;
    update_icon(&app_handle, false);
    result
}

async fn pull_project_internal(
    app_handle: &AppHandle,
    project_id: String,
) -> Result<String, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
        .clone()
        .ok_or("Project not linked to Supabase")?;

    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    let log = LogEntry::info(Some(uuid), LogSource::System, "Pulling remote schema...".to_string());
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // 1. Introspect Remote
    let introspector = crate::introspection::Introspector::new(&api, project_ref.clone());
    let remote_schema = introspector.introspect().await.map_err(|e| e.to_string())?;

    // 2. Generate SQL (Full Dump)
    // We can use the generator to create CREATE statements for everything in remote schema
    // By "diffing" against an empty schema
    let empty_schema = crate::schema::DbSchema::new();
    let diff = crate::diff::compute_diff(&empty_schema, &remote_schema); // Remote is 'local' (target), Empty is 'remote' (base) -> All creates
    // Wait, compute_diff(remote, local) -> diff to transform remote to local?
    // compute_diff(base, target) -> diff to transform base to target.
    // We want to transform Empty -> Remote. So compute_diff(empty, remote).
    
    let sql = crate::generator::generate_sql(&diff, &remote_schema);

    // 3. Write to file
    let supabase_dir = std::path::Path::new(&project.local_path).join("supabase");
    if !supabase_dir.exists() {
        tokio::fs::create_dir_all(&supabase_dir)
            .await
            .map_err(|e| e.to_string())?;
    }
    
    let schemas_dir = supabase_dir.join("schemas");
    let schema_path = schemas_dir.join("schema.sql");
    // Ensure schemas dir exists
    if !schemas_dir.exists() {
         tokio::fs::create_dir_all(&schemas_dir)
            .await
            .map_err(|e| e.to_string())?;
    }

    tokio::fs::write(&schema_path, &sql)
        .await
        .map_err(|e| e.to_string())?;

    let log = LogEntry::success(
        Some(uuid),
        LogSource::System,
        "Project schema pulled to supabase/schemas/schema.sql".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // 4. Generate TypeScript types from the pulled schema
    generate_typescript_for_project(&project, &schema_path, state.inner(), app_handle).await;

    // 5. Pull Edge Functions
    sync::pull_edge_functions(&api, &project_ref, Some(uuid), std::path::Path::new(&project.local_path), state.inner(), app_handle).await?;

    Ok(sql)
}

// Helper to push edge functions (deploy changed functions)
async fn push_edge_functions(
    api: &crate::supabase_api::SupabaseApi,
    project_ref: &str,
    project_id: Uuid,
    project_local_path: &std::path::Path,
    state: &Arc<AppState>,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let functions_dir = project_local_path.join("supabase").join("functions");

    if !functions_dir.exists() {
        return Ok(()); // No functions directory, nothing to deploy
    }

    let log = LogEntry::info(
        Some(project_id),
        LogSource::EdgeFunction,
        "Checking edge functions for changes...".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // Read the functions directory
    let mut entries = match tokio::fs::read_dir(&functions_dir).await {
        Ok(e) => e,
        Err(_) => return Ok(()), // Can't read dir, skip
    };

    let mut deployed_count = 0;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();

        // Skip if not a directory (each function should be in its own folder)
        if !path.is_dir() {
            continue;
        }

        let function_slug = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Collect all files in the function directory using shared sync module
        let files = match sync::collect_function_files(&path).await {
            Ok(f) => f,
            Err(e) => {
                let log = LogEntry::warning(
                    Some(project_id),
                    LogSource::EdgeFunction,
                    format!("Failed to read {}: {}", function_slug, e),
                );
                state.add_log(log).await;
                continue;
            }
        };

        if files.is_empty() {
            continue;
        }

        // Compute hash of all local files for comparison using shared sync module
        let local_hash = sync::compute_files_hash(&files);

        // Check if we have a stored hash from last deploy
        let hash_file = path.join(".supawatch_hash");
        let should_deploy = match tokio::fs::read_to_string(&hash_file).await {
            Ok(stored_hash) => stored_hash.trim() != local_hash,
            Err(_) => true, // No hash file = never deployed or new function
        };

        if !should_deploy {
            let log = LogEntry::info(
                Some(project_id),
                LogSource::EdgeFunction,
                format!("Function '{}' unchanged, skipping", function_slug),
            );
            state.add_log(log).await;
            continue;
        }

        // Determine entrypoint using shared sync module
        let entrypoint = sync::determine_entrypoint(&files);

        // Deploy with all files
        match api
            .deploy_function(project_ref, &function_slug, &function_slug, &entrypoint, files)
            .await
        {
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
                deployed_count += 1;
            }
            Err(e) => {
                let log = LogEntry::error(
                    Some(project_id),
                    LogSource::EdgeFunction,
                    format!("Failed to deploy '{}': {}", function_slug, e),
                );
                state.add_log(log.clone()).await;
                app_handle.emit("log", &log).ok();
            }
        }
    }

    if deployed_count > 0 {
        let log = LogEntry::success(
            Some(project_id),
            LogSource::EdgeFunction,
            format!("Deployed {} edge function(s)", deployed_count),
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
    }

    Ok(())
}

#[tauri::command]
pub async fn push_project(
    app_handle: AppHandle,
    project_id: String,
    force: Option<bool>,
) -> Result<String, String> {
    update_icon(&app_handle, true);
    let result = push_project_internal(&app_handle, project_id, force).await;
    update_icon(&app_handle, false);
    result
}

async fn push_project_internal(
    app_handle: &AppHandle,
    project_id: String,
    force: Option<bool>,
) -> Result<String, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
        .clone()
        .ok_or("Project not linked to Supabase")?;

    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    let log = LogEntry::info(Some(uuid), LogSource::System, "Pushing schema changes...".to_string());
    println!("[INFO] Pushing schema changes for project {}", uuid);
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // Find schema path using shared sync module
    let schema_path = sync::find_schema_path(Path::new(&project.local_path))
        .ok_or("Schema file not found (checked supabase/schemas/schema.sql and supabase/schema.sql)")?;

    // Compute diff using shared sync module (introspect remote, parse local, compute diff)
    let diff_result = sync::compute_schema_diff(&api, &project_ref, &schema_path).await?;
    let diff = diff_result.diff;

    let summary = diff.summarize();
    
    // Check for destructive changes
    if !force.unwrap_or(false) && diff.is_destructive() {
        let log = LogEntry::warning(
            Some(uuid),
            LogSource::System,
            "Destructive changes detected. Confirmation required.".to_string(),
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
        
        return Err(format!("CONFIRMATION_NEEDED:{}", summary));
    }

    println!("[INFO] Diff Summary:\n{}", summary);
    let log = LogEntry::info(
        Some(uuid),
        LogSource::System,
        format!("Diff Summary:\n{}", summary),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // Use migration SQL from diff result
    let migration_sql = &diff_result.migration_sql;

    if migration_sql.trim().is_empty() {
         let log = LogEntry::success(
            Some(uuid),
            LogSource::System,
            "No schema changes detected.".to_string(),
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
        
        // Still deploy edge functions even if no schema changes
        push_edge_functions(&api, &project_ref, uuid, std::path::Path::new(&project.local_path), state.inner(), app_handle).await?;
        
        return Ok("No changes".to_string());
    }

    let log = LogEntry::info(
        Some(uuid),
        LogSource::System,
        format!("Applying changes:\n{}", migration_sql),
    );
    println!("[INFO] Applying changes:\n{}", migration_sql);
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // 5. Execute
    let result = api.run_query(&project_ref, &migration_sql, false).await.map_err(|e| e.to_string())?;

    if let Some(err) = result.error {
        let log = LogEntry::error(Some(uuid), LogSource::System, format!("Migration failed: {}", err));
        println!("[ERROR] Migration failed: {}", err);
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
        return Err(err);
    }

    let log = LogEntry::success(
        Some(uuid),
        LogSource::System,
        "Schema changes pushed successfully.".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // 6. Generate TypeScript types after successful push
    generate_typescript_for_project(&project, &schema_path, state.inner(), app_handle).await;

    // 7. Deploy edge functions if any have changed
    push_edge_functions(&api, &project_ref, uuid, std::path::Path::new(&project.local_path), state.inner(), app_handle).await?;

    Ok(migration_sql.to_string())
}

#[tauri::command]
pub async fn run_query(
    app_handle: AppHandle,
    project_id: String,
    query: String,
    read_only: Option<bool>,
) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
        .ok_or("Project not linked to Supabase")?;

    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    let log = LogEntry::info(
        Some(uuid),
        LogSource::Schema,
        "Running SQL query...".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    let result = api
        .run_query(&project_ref, &query, read_only.unwrap_or(false))
        .await
        .map_err(|e| {
            let log = LogEntry::error(
                Some(uuid),
                LogSource::Schema,
                format!("Query failed: {}", e),
            );
            tauri::async_runtime::block_on(async {
                state.add_log(log.clone()).await;
            });
            app_handle.emit("log", &log).ok();
            e.to_string()
        })?;

    if let Some(error) = result.error {
        let log = LogEntry::error(Some(uuid), LogSource::Schema, format!("Query error: {}", error));
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
        return Err(error);
    }

    let log = LogEntry::success(
        Some(uuid),
        LogSource::Schema,
        "Query executed successfully".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    Ok(result.result.unwrap_or(serde_json::Value::Null))
}

#[tauri::command]
pub async fn deploy_edge_function(
    app_handle: AppHandle,
    project_id: String,
    function_slug: String,
    function_name: String,
    function_path: String,
) -> Result<String, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
        .ok_or("Project not linked to Supabase")?;

    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    let log = LogEntry::info(
        Some(uuid),
        LogSource::EdgeFunction,
        format!("Deploying edge function: {}", function_name),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // Get function directory from path
    let full_path = Path::new(&project.local_path).join(&function_path);
    let function_dir = full_path.parent().unwrap_or(&full_path);

    // Collect all files from the function directory using shared sync module
    let files = sync::collect_function_files(function_dir)
        .await
        .map_err(|e| format!("Failed to read function files: {}", e))?;

    if files.is_empty() {
        return Err("No files found in function directory".to_string());
    }

    // Determine entrypoint using shared sync module
    let entrypoint = sync::determine_entrypoint(&files);

    let result = api
        .deploy_function(
            &project_ref,
            &function_slug,
            &function_name,
            &entrypoint,
            files,
        )
        .await
        .map_err(|e| {
            let log = LogEntry::error(
                Some(uuid),
                LogSource::EdgeFunction,
                format!("Deploy failed: {}", e),
            );
            tauri::async_runtime::block_on(async {
                state.add_log(log.clone()).await;
            });
            app_handle.emit("log", &log).ok();
            e.to_string()
        })?;

    let log = LogEntry::success(
        Some(uuid),
        LogSource::EdgeFunction,
        format!("Deployed {} (v{})", result.name, result.version),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    Ok(format!(
        "Successfully deployed {} version {}",
        result.name, result.version
    ))
}

#[tauri::command]
pub async fn get_remote_schema(
    app_handle: AppHandle,
    project_id: String,
) -> Result<String, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
        .ok_or("Project not linked to Supabase")?;

    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    let log = LogEntry::info(
        Some(uuid),
        LogSource::Schema,
        "Fetching remote schema...".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    let schema = api
        .get_schema(&project_ref)
        .await
        .map_err(|e| e.to_string())?;

    let log = LogEntry::success(
        Some(uuid),
        LogSource::Schema,
        "Remote schema fetched".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    Ok(schema)
}

/// Helper to generate TypeScript types for a project.
/// This is called after pull and push operations.
async fn generate_typescript_for_project(
    project: &Project,
    schema_path: &Path,
    state: &AppState,
    app_handle: &AppHandle,
) {
    // Check if TypeScript generation is enabled for this project
    if !project.generate_typescript {
        return;
    }

    let project_path = Path::new(&project.local_path);

    // Get TypeScript output path (use custom path if configured)
    let ts_output_path = sync::get_typescript_output_path(
        project_path,
        project.typescript_output_path.as_deref(),
    );

    let log = LogEntry::info(
        Some(project.id),
        LogSource::Schema,
        "Generating TypeScript types...".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // Generate TypeScript types
    match sync::generate_typescript_types(schema_path, &ts_output_path).await {
        Ok(()) => {
            let relative_output = ts_output_path
                .strip_prefix(project_path)
                .unwrap_or(&ts_output_path)
                .to_string_lossy();
            let log = LogEntry::success(
                Some(project.id),
                LogSource::Schema,
                format!("TypeScript types generated: {}", relative_output),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
        }
        Err(e) => {
            let log = LogEntry::error(
                Some(project.id),
                LogSource::Schema,
                format!("TypeScript generation failed: {}", e),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
        }
    }
}
