use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::models::{LogEntry, LogSource, Project};
use crate::state::AppState;
use crate::sync;
use crate::tray::update_icon;

#[derive(serde::Serialize)]
pub struct PullDiffResponse {
    pub migration_sql: String,
    pub edge_functions: Vec<sync::EdgeFunctionDiff>,
}

async fn fetch_remote_schema_sql(
    api: &crate::supabase_api::SupabaseApi,
    project_ref: &str,
) -> Result<(String, crate::schema::DbSchema), String> {
    // 1. Introspect Remote
    let introspector = crate::introspection::Introspector::new(api, project_ref.to_string());
    let remote_schema = introspector.introspect().await.map_err(|e| e.to_string())?;

    // 2. Generate SQL (Full Dump)
    let empty_schema = crate::schema::DbSchema::new();
    let diff = crate::diff::compute_diff(&empty_schema, &remote_schema);
    let sql = crate::generator::generate_sql(&diff, &remote_schema);

    Ok((sql, remote_schema))
}

#[tauri::command]
pub async fn get_pull_diff(
    app_handle: AppHandle,
    project_id: String,
) -> Result<PullDiffResponse, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
        .clone()
        .ok_or("Project not linked to Supabase")?;

    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    // 1. Get Schema SQL
    let (migration_sql, _) = fetch_remote_schema_sql(&api, &project_ref).await?;

    // 2. List Edge Functions
    let funcs = api.list_functions(&project_ref).await.map_err(|e| e.to_string())?;
    let edge_functions = funcs
        .into_iter()
        .map(|f| sync::EdgeFunctionDiff {
            slug: f.slug.clone(),
            name: f.name.clone(),
            path: format!("supabase/functions/{}", f.slug),
        })
        .collect();

    Ok(PullDiffResponse {
        migration_sql,
        edge_functions,
    })
}


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

    // 1. Fetch Remote Schema (Introspect + Generate SQL)
    let (sql, remote_schema) = fetch_remote_schema_sql(&api, &project_ref).await?;

    // Cache the schema for AI SQL conversion
    state.set_cached_schema(uuid, remote_schema).await;

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
// Helper to push edge functions (deploy changed functions)
async fn push_edge_functions(
    api: &crate::supabase_api::SupabaseApi,
    project_ref: &str,
    project_id: Uuid,
    project_local_path: &std::path::Path,
    state: &Arc<AppState>,
    app_handle: &AppHandle,
) -> Result<Vec<EdgeFunctionDeploymentResult>, String> {
    let log = LogEntry::info(
        Some(project_id),
        LogSource::EdgeFunction,
        "Checking edge functions for changes...".to_string(),
    );
    println!("[INFO] Checking edge functions for changes...");
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // Use shared logic to find changed functions
    let changed_functions = sync::compute_edge_functions_diff(project_local_path)
        .await
        .map_err(|e| format!("Failed to compute edge function diff: {}", e))?;

    if changed_functions.is_empty() {
        let log = LogEntry::info(
            Some(project_id),
            LogSource::EdgeFunction,
            "No edge function changes detected.".to_string(),
        );
        println!("[INFO] No edge function changes detected.");
        state.add_log(log).await;
        // app_handle.emit("log", &log).ok(); // Optional: don't spam if nothing happened
        return Ok(Vec::new());
    }

    let mut deployment_results = Vec::new();
    let mut deployed_count = 0;

    for func in changed_functions {
        let function_slug = func.slug;
        let function_path = project_local_path.join(&func.path);

        // We need to re-read files to deploy them
        // This is slightly inefficient but safe and reuses logic
        let files = match sync::collect_function_files(&function_path).await {
            Ok(f) => f,
            Err(e) => {
                let log = LogEntry::warning(
                    Some(project_id),
                    LogSource::EdgeFunction,
                    format!("Failed to read {}: {}", function_slug, e),
                );
                state.add_log(log).await;
                 deployment_results.push(EdgeFunctionDeploymentResult {
                    name: function_slug.clone(),
                    status: "error".to_string(),
                    version: None,
                    error: Some(format!("Failed to read files: {}", e)),
                });
                continue;
            }
        };

        if files.is_empty() {
             continue;
        }

        // Determine entrypoint using shared sync module
        let entrypoint = sync::determine_entrypoint(&files);
        
        // Re-compute hash to update the lockfile after deploy
        let local_hash = sync::compute_files_hash(&files);
        let hash_file = function_path.join(".supawatch_hash");

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
                println!("[INFO] Deployed '{}' (v{})", result.name, result.version);
                state.add_log(log.clone()).await;
                app_handle.emit("log", &log).ok();
                deployed_count += 1;
                
                deployment_results.push(EdgeFunctionDeploymentResult {
                    name: result.name,
                    status: "success".to_string(),
                    version: Some(result.version),
                    error: None,
                });
            }
            Err(e) => {
                let log = LogEntry::error(
                    Some(project_id),
                    LogSource::EdgeFunction,
                    format!("Failed to deploy '{}': {}", function_slug, e),
                );
                println!("[ERROR] Failed to deploy '{}': {}", function_slug, e);
                state.add_log(log.clone()).await;
                app_handle.emit("log", &log).ok();
                
                 deployment_results.push(EdgeFunctionDeploymentResult {
                    name: function_slug.clone(),
                    status: "error".to_string(),
                    version: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    if deployed_count > 0 {
        let log = LogEntry::success(
            Some(project_id),
            LogSource::EdgeFunction,
            format!("Deployed {} edge function(s)", deployed_count),
        );
        println!("[INFO] Deployed {} edge function(s)", deployed_count);
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
    }

    Ok(deployment_results)
}

#[derive(serde::Serialize)]
pub struct EdgeFunctionDeploymentResult {
    pub name: String,
    pub status: String, // "success" or "error"
    pub version: Option<i32>,
    pub error: Option<String>,
}

#[derive(serde::Serialize)]
pub struct PushResponse {
    pub migration_sql: String,
    pub edge_function_results: Vec<EdgeFunctionDeploymentResult>,
}

#[tauri::command]
pub async fn push_project(
    app_handle: AppHandle,
    project_id: String,
    force: Option<bool>,
) -> Result<PushResponse, String> {
    update_icon(&app_handle, true);
    let result = push_project_internal(&app_handle, project_id, force).await;
    update_icon(&app_handle, false);
    result
}

async fn push_project_internal(
    app_handle: &AppHandle,
    project_id: String,
    force: Option<bool>,
) -> Result<PushResponse, String> {
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
        let edge_function_results = push_edge_functions(&api, &project_ref, uuid, std::path::Path::new(&project.local_path), state.inner(), app_handle).await?;
        
        return Ok(PushResponse {
            migration_sql: "No changes".to_string(),
            edge_function_results,
        });
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

    // Clear schema cache since remote schema changed
    state.clear_cached_schema(uuid).await;

    // 6. Generate TypeScript types after successful push
    generate_typescript_for_project(&project, &schema_path, state.inner(), app_handle).await;

    // 7. Deploy edge functions if any have changed
    let edge_function_results = push_edge_functions(&api, &project_ref, uuid, std::path::Path::new(&project.local_path), state.inner(), app_handle).await?;

    Ok(PushResponse {
        migration_sql: migration_sql.to_string(),
        edge_function_results,
    })
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
        .map_err(|e| e.to_string())?;

    // Log any API errors
    if let Some(error) = &result.error {
        let log = LogEntry::error(
            Some(uuid),
            LogSource::Schema,
            format!("Query failed: {}", error),
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
    }

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
        .await;

    // Handle deployment errors
    let result = match result {
        Ok(r) => r,
        Err(e) => {
            let log = LogEntry::error(
                Some(uuid),
                LogSource::EdgeFunction,
                format!("Deploy failed: {}", e),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
            return Err(e.to_string());
        }
    };

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

#[tauri::command]
pub async fn run_seeds(
    app_handle: AppHandle,
    project_id: String,
) -> Result<String, String> {
    update_icon(&app_handle, true);
    let result = run_seeds_internal(&app_handle, project_id).await;
    update_icon(&app_handle, false);
    result
}

async fn run_seeds_internal(
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

    let log = LogEntry::info(Some(uuid), LogSource::System, "Running seed files...".to_string());
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    // Find seed directory
    let seed_dir = Path::new(&project.local_path).join("supabase").join("seed");

    if !seed_dir.exists() {
        let log = LogEntry::warning(
            Some(uuid),
            LogSource::System,
            "No seed directory found at supabase/seed".to_string(),
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
        return Ok("No seed directory found".to_string());
    }

    // Collect all .sql files in the seed directory
    let mut seed_files: Vec<std::path::PathBuf> = Vec::new();
    let mut entries = tokio::fs::read_dir(&seed_dir).await.map_err(|e| e.to_string())?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "sql") {
            seed_files.push(path);
        }
    }

    if seed_files.is_empty() {
        let log = LogEntry::warning(
            Some(uuid),
            LogSource::System,
            "No .sql files found in seed directory".to_string(),
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
        return Ok("No seed files found".to_string());
    }

    // Sort files alphabetically by filename
    seed_files.sort_by(|a, b| {
        a.file_name()
            .unwrap_or_default()
            .cmp(b.file_name().unwrap_or_default())
    });

    let total_files = seed_files.len();
    let mut executed_count = 0;

    // Execute each seed file in order
    for (index, seed_path) in seed_files.iter().enumerate() {
        let filename = seed_path.file_name().unwrap_or_default().to_string_lossy();

        let log = LogEntry::info(
            Some(uuid),
            LogSource::System,
            format!("Running seed ({}/{}) {}...", index + 1, total_files, filename),
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();

        // Read the SQL file
        let sql = match tokio::fs::read_to_string(&seed_path).await {
            Ok(content) => content,
            Err(e) => {
                let log = LogEntry::error(
                    Some(uuid),
                    LogSource::System,
                    format!("Failed to read {}: {}", filename, e),
                );
                state.add_log(log.clone()).await;
                app_handle.emit("log", &log).ok();
                continue;
            }
        };

        if sql.trim().is_empty() {
            let log = LogEntry::warning(
                Some(uuid),
                LogSource::System,
                format!("Skipping empty seed file: {}", filename),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
            continue;
        }

        // Execute the SQL
        // Prepend search_path to ensure functions/tables in public/extensions are found
        let run_sql = format!("SET search_path = \"$user\", public, extensions;\n{}", sql);

        match api.run_query(&project_ref, &run_sql, false).await {
            Ok(result) => {
                if let Some(err) = result.error {
                    let log = LogEntry::error(
                        Some(uuid),
                        LogSource::System,
                        format!("Seed {} failed: {}", filename, err),
                    );
                    state.add_log(log.clone()).await;
                    app_handle.emit("log", &log).ok();
                    return Err(format!("Seed {} failed: {}", filename, err));
                }
                executed_count += 1;
            }
            Err(e) => {
                let log = LogEntry::error(
                    Some(uuid),
                    LogSource::System,
                    format!("Seed {} failed: {}", filename, e),
                );
                state.add_log(log.clone()).await;
                app_handle.emit("log", &log).ok();
                return Err(format!("Seed {} failed: {}", filename, e));
            }
        }
    }

    let log = LogEntry::success(
        Some(uuid),
        LogSource::System,
        format!("Successfully executed {} seed file(s)", executed_count),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    Ok(format!("Executed {} seed file(s)", executed_count))
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

#[derive(serde::Serialize)]
pub struct DiffResponse {
    pub summary: String,
    pub migration_sql: String,
    pub is_destructive: bool,
    pub edge_functions: Vec<sync::EdgeFunctionDiff>,
}

#[tauri::command]
pub async fn get_project_diff(
    app_handle: AppHandle,
    project_id: String,
) -> Result<DiffResponse, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
        .clone()
        .ok_or("Project not linked to Supabase")?;

    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    // Find schema path
    let schema_path = sync::find_schema_path(Path::new(&project.local_path))
        .ok_or("Schema file not found (checked supabase/schemas/schema.sql and supabase/schema.sql)")?;

    // Compute diff
    let diff_result = sync::compute_schema_diff(&api, &project_ref, &schema_path).await?;
    let diff = diff_result.diff;
    let summary = diff.summarize();
    let is_destructive = diff.is_destructive();
    let migration_sql = diff_result.migration_sql;

    // Compute edge function diffs
    let edge_functions = sync::compute_edge_functions_diff(Path::new(&project.local_path))
        .await
        .map_err(|e| e.to_string())?;

    Ok(DiffResponse {
        summary,
        migration_sql,
        is_destructive,
        edge_functions,
    })
}

#[tauri::command]
pub async fn get_seed_content(
    app_handle: AppHandle,
    project_id: String,
) -> Result<String, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;

    // Find seed directory
    let seed_dir = Path::new(&project.local_path).join("supabase").join("seed");

    if !seed_dir.exists() {
        return Ok("-- No seed directory found at supabase/seed".to_string());
    }

    // Collect all .sql files in the seed directory
    let mut seed_files: Vec<std::path::PathBuf> = Vec::new();
    match tokio::fs::read_dir(&seed_dir).await {
        Ok(mut entries) => {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "sql") {
                    seed_files.push(path);
                }
            }
        }
        Err(_) => return Ok("-- Failed to read seed directory".to_string()),
    }

    if seed_files.is_empty() {
        return Ok("-- No .sql files found in seed directory".to_string());
    }

    // Sort files alphabetically by filename
    seed_files.sort_by(|a, b| {
        a.file_name()
            .unwrap_or_default()
            .cmp(b.file_name().unwrap_or_default())
    });

    let mut combined_sql = String::new();

    for seed_path in seed_files {
        let filename = seed_path.file_name().unwrap_or_default().to_string_lossy();
        combined_sql.push_str(&format!("-- File: {}\n", filename));
        
        let sql = tokio::fs::read_to_string(&seed_path).await.map_err(|e| e.to_string())?;
        combined_sql.push_str(&sql);
        combined_sql.push_str("\n\n");
    }

    Ok(combined_sql)
}
