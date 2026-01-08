use std::path::Path;
use std::sync::{Arc, Once};

use tauri::{Emitter, Manager};
use uuid::Uuid;

use crate::fns::{
    setup_menubar_panel_listeners, swizzle_to_menubar_panel, update_menubar_appearance,
};
use crate::models::{LogEntry, LogSource, Project, RemoteProject};
use crate::state::AppState;
use crate::supabase_api::Organization;
use crate::sync;
use crate::tray::update_icon;
use crate::watcher;

pub mod templates;

static INIT: Once = Once::new();

#[tauri::command]
pub fn init(app_handle: tauri::AppHandle) {
    INIT.call_once(|| {
        swizzle_to_menubar_panel(&app_handle);

        update_menubar_appearance(&app_handle);

        setup_menubar_panel_listeners(&app_handle);
    });
}

#[tauri::command]
pub fn show_menubar_panel(app_handle: tauri::AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub async fn pick_project_folder(app_handle: tauri::AppHandle) -> Result<Option<String>, String> {
    use crate::fns::IS_DIALOG_OPEN;
    use std::sync::atomic::Ordering;
    use tauri_plugin_dialog::DialogExt;

    IS_DIALOG_OPEN.store(true, Ordering::Relaxed);
    
    let (tx, rx) = tokio::sync::oneshot::channel();

    app_handle
        .dialog()
        .file()
        .set_title("Select Supabase Project Folder")
        .set_directory(dirs::home_dir().unwrap_or_default())
        .pick_folder(move |path| {
            let _ = tx.send(path);
        });

    let result = rx
        .await
        .map_err(|e| e.to_string())
        .map(|path| path.map(|p| p.to_string()));

    IS_DIALOG_OPEN.store(false, Ordering::Relaxed);

    // After closing dialog, ensure we re-focus if needed, 
    // though the blur event might have already fired.
    // The key is that hide_menubar_panel checked the flag during the blur.
    
    // If the panel was hidden for some reason, show it again?
    // Actually, if we prevented the hide, it should still be there.
    // But focusing the webview again is good practice.
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_focus();
    }

    result
}

// Access token commands

#[tauri::command]
pub async fn set_access_token(
    app_handle: tauri::AppHandle,
    token: String,
) -> Result<(), String> {
    let state = app_handle.state::<Arc<AppState>>();
    state.set_access_token(token).await.map_err(|e| e.to_string())?;

    let log = LogEntry::success(None, LogSource::System, "Access token saved".to_string());
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    Ok(())
}



#[tauri::command]
pub async fn has_access_token(app_handle: tauri::AppHandle) -> Result<bool, String> {
    let state = app_handle.state::<Arc<AppState>>();
    Ok(state.has_access_token().await)
}

#[tauri::command]
pub async fn clear_access_token(app_handle: tauri::AppHandle) -> Result<(), String> {
    let state = app_handle.state::<Arc<AppState>>();
    state.clear_access_token().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn validate_access_token(app_handle: tauri::AppHandle) -> Result<bool, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    match api.list_projects().await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

// Remote Supabase project commands

#[tauri::command]
pub async fn list_remote_projects(
    app_handle: tauri::AppHandle,
) -> Result<Vec<RemoteProject>, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    let projects = api.list_projects().await.map_err(|e| e.to_string())?;

    Ok(projects
        .into_iter()
        .map(|p| RemoteProject {
            id: p.id,
            name: p.name,
            organization_id: p.organization_id,
            region: p.region,
            created_at: p.created_at,
        })
        .collect())
}

// Sync commands

#[tauri::command]
pub async fn list_organizations(app_handle: tauri::AppHandle) -> Result<Vec<Organization>, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let api = state.get_api_client().await.map_err(|e| e.to_string())?;
    api.list_organizations()
        .await
        .map_err(|e| format!("Failed to list organizations: {}", e))
}

#[tauri::command]
pub async fn pull_project(
    app_handle: tauri::AppHandle,
    project_id: String,
) -> Result<String, String> {
    update_icon(&app_handle, true);
    let result = pull_project_internal(&app_handle, project_id).await;
    update_icon(&app_handle, false);
    result
}

async fn pull_project_internal(
    app_handle: &tauri::AppHandle,
    project_id: String,
) -> Result<String, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
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

    // 4. Pull Edge Functions
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
    app_handle: &tauri::AppHandle,
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
    app_handle: tauri::AppHandle,
    project_id: String,
    force: Option<bool>,
) -> Result<String, String> {
    update_icon(&app_handle, true);
    let result = push_project_internal(&app_handle, project_id, force).await;
    update_icon(&app_handle, false);
    result
}

async fn push_project_internal(
    app_handle: &tauri::AppHandle,
    project_id: String,
    force: Option<bool>,
) -> Result<String, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
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

    // 6. Deploy edge functions if any have changed
    push_edge_functions(&api, &project_ref, uuid, std::path::Path::new(&project.local_path), state.inner(), app_handle).await?;

    Ok(migration_sql.to_string())
}

// Project commands

#[tauri::command]
pub async fn create_project(
    app_handle: tauri::AppHandle,
    name: String,
    local_path: String,
    supabase_project_id: Option<String>,
    supabase_project_ref: Option<String>,
    organization_id: Option<String>,
) -> Result<Project, String> {
    let state = app_handle.state::<Arc<AppState>>();

    let (project_id, project_ref) = if let Some(refer) = supabase_project_ref {
        // Sync/Link Mode
        let log = LogEntry::info(
            None, 
            LogSource::System, 
            format!("Linking to existing project: {}", refer)
        );
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();

        // Resolve Project ID (if missing)
        let pid = if let Some(id) = supabase_project_id {
            id
        } else {
            // Try to fetch from API to get the canonical ID
             if state.has_access_token().await {
                 match state.get_api_client().await {
                     Ok(api) => {
                         match api.get_project(&refer).await {
                             Ok(p) => p.id,
                             Err(_) => refer.clone() // Fallback
                         }
                     },
                     Err(_) => refer.clone() // Fallback
                 }
             } else {
                 refer.clone() // Fallback
             }
        };

        // Auto-pull schema logic
        println!("[DEBUG] Starting auto-pull schema logic");
        let supabase_dir = std::path::Path::new(&local_path).join("supabase");
        let schemas_dir = supabase_dir.join("schemas");
        let schema_path = schemas_dir.join("schema.sql");
        
        println!("[DEBUG] Checking if schema exists at: {:?}", schema_path);
        if !schema_path.exists() {
             println!("[DEBUG] Schema does not exist, checking access token...");
             if state.has_access_token().await {
                println!("[DEBUG] Has access token, getting API client...");
                if let Ok(api) = state.get_api_client().await {
                   println!("[DEBUG] Got API client, creating introspector...");
                   let introspector = crate::introspection::Introspector::new(&api, refer.clone());
                   
                   let log = LogEntry::info(
                        None, 
                        LogSource::System, 
                        format!("Auto-pulling schema for linked project: {}", refer)
                    );
                   state.add_log(log.clone()).await;
                   app_handle.emit("log", &log).ok();

                   println!("[DEBUG] Starting introspection...");
                   match introspector.introspect().await {
                        Ok(remote_schema) => {
                             println!("[DEBUG] Introspection successful, generating SQL...");
                             let empty_schema = crate::schema::DbSchema::new();
                             let diff = crate::diff::compute_diff(&empty_schema, &remote_schema);
                             let sql = crate::generator::generate_sql(&diff, &remote_schema);
                             
                             println!("[DEBUG] Creating schema dir: {:?}", schemas_dir);
                             if !schemas_dir.exists() {
                                tokio::fs::create_dir_all(&schemas_dir).await.map_err(|e| e.to_string())?;
                             }
                             println!("[DEBUG] Writing schema.sql ({} bytes)", sql.len());
                             tokio::fs::write(&schema_path, &sql).await.map_err(|e| e.to_string())?;

                             let log = LogEntry::success(
                                None,
                                LogSource::System,
                                "Schema pulled successfully".to_string(),
                            );
                            state.add_log(log.clone()).await;
                            app_handle.emit("log", &log).ok();
                        },
                        Err(e) => {
                             println!("[DEBUG] Introspection failed: {}", e);
                             let log = LogEntry::error(
                                None, 
                                LogSource::System, 
                                format!("Failed to auto-pull schema: {}", e)
                            );
                            state.add_log(log.clone()).await;
                            app_handle.emit("log", &log).ok();
                        }
                   }

                   // Auto-pull Edge Functions using shared sync module
                   println!("[DEBUG] Starting function sync...");
                   let _ = sync::pull_edge_functions(
                       &api,
                       &refer,
                       None,
                       std::path::Path::new(&local_path),
                       state.inner(),
                       &app_handle,
                   )
                   .await;
                }
             }
        }
        
        (Some(pid), Some(refer))
    } else {
        // Create Mode
        
        // Ensure standard Supabase folder structure exists for new projects
        let supabase_dir = std::path::Path::new(&local_path).join("supabase");
        if !supabase_dir.exists() {
            let schemas_dir = supabase_dir.join("schemas");
            let functions_dir = supabase_dir.join("functions");
            let schema_path = schemas_dir.join("schema.sql");

            // Create directories
            tokio::fs::create_dir_all(&schemas_dir)
                .await
                .map_err(|e| format!("Failed to create schemas directory: {}", e))?;
            tokio::fs::create_dir_all(&functions_dir)
                .await
                .map_err(|e| format!("Failed to create functions directory: {}", e))?;

            // Create placeholder schema.sql
            let placeholder = "-- Supabase schema\n\n-- Add your table definitions and other schema elements here.\n";
            tokio::fs::write(&schema_path, placeholder)
                .await
                .map_err(|e| format!("Failed to create schema.sql: {}", e))?;

            let log = LogEntry::success(
                None,
                LogSource::System,
                "Created local supabase directory structure".to_string(),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();
        }

       // Try to create remote project if authenticated
        if state.has_access_token().await {
            let api = state.get_api_client().await.map_err(|e| e.to_string())?;
            
            // Get organizations
            let orgs = api.list_organizations().await.map_err(|e| format!("Failed to list organizations: {}", e))?;
            
            let org = if let Some(oid) = &organization_id {
                orgs.iter().find(|o| o.id == *oid).ok_or("Selected organization not found.".to_string())?
            } else {
                orgs.first().ok_or("No organizations found. Please create one in Supabase dashboard.".to_string())?
            };

            // Generate a secure password (using UUID v4 for now as it's random enough)
            let db_pass = Uuid::new_v4().to_string();
            let region = "us-east-1"; // Default region

            let log = LogEntry::info(
                None,
                LogSource::System,
                format!("Creating remote Supabase project '{}'...", name),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();

            match api.create_project(&name, &org.id, &db_pass, region).await {
                Ok(remote_project) => {
                    let log = LogEntry::success(
                        None,
                        LogSource::System,
                        format!("Created remote project: {}", remote_project.name),
                    );
                    state.add_log(log.clone()).await;
                    app_handle.emit("log", &log).ok();

                    // Supabase API 'id' is the project reference (e.g., abcdefghi)
                    (Some(remote_project.id.clone()), Some(remote_project.id)) 
                },
                Err(e) => {
                    let log = LogEntry::error(
                        None,
                        LogSource::System,
                        format!("Failed to create remote project: {}", e),
                    );
                    state.add_log(log.clone()).await;
                    app_handle.emit("log", &log).ok();
                    return Err(format!("Failed to create remote project: {}", e));
                }
            }
        } else {
            (None, None)
        }
    };

    // Populate .env.local from .env.example if applicable
    if let (Some(_), Some(ref refer)) = (&project_id, &project_ref) {
        // We have a remote project, try to get keys and update .env.local
        let project_path = std::path::Path::new(&local_path);
        let example_path = project_path.join(".env.example");
        let env_path = project_path.join(".env.local");

        // Use async check for file existence
        if tokio::fs::metadata(&example_path).await.is_ok() {
             let log = LogEntry::info(
                None,
                LogSource::System,
                "Creating .env.local from .env.example...".to_string(),
            );
            state.add_log(log.clone()).await;
            app_handle.emit("log", &log).ok();

            if let Ok(api) = state.get_api_client().await {
                match api.ensure_api_keys(refer).await {
                    Ok(publishable_key) => {
                        let supabase_url = format!("https://{}.supabase.co", refer);
                        
                        // Read from .env.example
                        match tokio::fs::read_to_string(&example_path).await {
                            Ok(content) => {
                                let mut new_lines = Vec::new();
                                for line in content.lines() {
                                    if let Some((key, _)) = line.split_once('=') {
                                        let trimmed_key = key.trim();
                                        if trimmed_key.ends_with("SUPABASE_URL") {
                                            new_lines.push(format!("{}={}", trimmed_key, supabase_url));
                                        } else if trimmed_key.ends_with("SUPABASE_PUBLISHABLE_KEY") 
                                               || trimmed_key.ends_with("SUPABASE_ANON_KEY") 
                                               || trimmed_key.ends_with("SUPABASE_PUBLISHABLE_DEFAULT_KEY") {
                                            new_lines.push(format!("{}={}", trimmed_key, publishable_key));
                                        } else {
                                            new_lines.push(line.to_string());
                                        }
                                    } else {
                                        new_lines.push(line.to_string());
                                    }
                                }
                                let new_content = new_lines.join("\n");
                                // Write to .env.local
                                if let Err(e) = tokio::fs::write(&env_path, new_content).await {
                                     let log = LogEntry::error(
                                        None,
                                        LogSource::System,
                                        format!("Failed to write .env.local: {}", e),
                                    );
                                    state.add_log(log.clone()).await;
                                    app_handle.emit("log", &log).ok();
                                } else {
                                    let log = LogEntry::success(
                                        None,
                                        LogSource::System,
                                        "Created .env.local with Supabase keys".to_string(),
                                    );
                                    state.add_log(log.clone()).await;
                                    app_handle.emit("log", &log).ok();
                                }
                            },
                            Err(e) => {
                                let log = LogEntry::error(
                                    None,
                                    LogSource::System,
                                    format!("Failed to read .env.example: {}", e),
                                );
                                state.add_log(log.clone()).await;
                                app_handle.emit("log", &log).ok();
                            }
                        }
                    },
                    Err(e) => {
                        let log = LogEntry::error(
                            None,
                            LogSource::System,
                            format!("Failed to retrieve API keys: {}", e),
                        );
                        state.add_log(log.clone()).await;
                        app_handle.emit("log", &log).ok();
                    }
                }
            }
        }
    }

    let project = if let (Some(pid), Some(pref)) = (project_id, project_ref) {
        Project::with_remote(name, local_path, pid, pref)
    } else {
        Project::new(name, local_path)
    };

    let result = state
        .add_project(project)
        .await
        .map_err(|e| e.to_string())?;

    let log = LogEntry::success(
        Some(result.id),
        LogSource::System,
        format!("Created project: {}", result.name),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    Ok(result)
}

#[tauri::command]
pub async fn get_projects(app_handle: tauri::AppHandle) -> Result<Vec<Project>, String> {
    let state = app_handle.state::<Arc<AppState>>();
    Ok(state.get_projects().await)
}

#[tauri::command]
pub async fn get_project(app_handle: tauri::AppHandle, id: String) -> Result<Project, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.get_project(uuid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_project(
    app_handle: tauri::AppHandle,
    project: Project,
) -> Result<Project, String> {
    let state = app_handle.state::<Arc<AppState>>();
    state
        .update_project(project)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_project(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;

    // Get project name for logging
    let project = state.get_project(uuid).await.ok();

    state.delete_project(uuid).await.map_err(|e| e.to_string())?;

    if let Some(p) = project {
        let log = LogEntry::info(None, LogSource::System, format!("Deleted project: {}", p.name));
        state.add_log(log.clone()).await;
        app_handle.emit("log", &log).ok();
    }

    Ok(())
}



#[tauri::command]
pub async fn link_supabase_project(
    app_handle: tauri::AppHandle,
    project_id: String,
    supabase_project_ref: String,
) -> Result<Project, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    // Verify the remote project exists
    let api = state.get_api_client().await.map_err(|e| e.to_string())?;
    let remote = api
        .get_project(&supabase_project_ref)
        .await
        .map_err(|e| format!("Failed to verify Supabase project: {}", e))?;

    let mut project = state.get_project(uuid).await.map_err(|e| e.to_string())?;

    project.supabase_project_ref = Some(supabase_project_ref.clone());
    project.supabase_project_id = Some(remote.id);
    project.updated_at = chrono::Utc::now();

    let result = state
        .update_project(project)
        .await
        .map_err(|e| e.to_string())?;

    let log = LogEntry::success(
        Some(uuid),
        LogSource::System,
        format!("Linked to Supabase project: {}", remote.name),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    Ok(result)
}

// Watcher commands

#[tauri::command]
pub async fn start_watching(
    app_handle: tauri::AppHandle,
    project_id: String,
) -> Result<(), String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;

    watcher::start_watching(&app_handle, uuid, &project.local_path).await
}

#[tauri::command]
pub async fn stop_watching(app_handle: tauri::AppHandle, project_id: String) -> Result<(), String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    watcher::stop_watching(&app_handle, uuid).await
}

#[tauri::command]
pub async fn is_watching(app_handle: tauri::AppHandle, project_id: String) -> Result<bool, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    Ok(state.is_watching(uuid).await)
}

// Log commands

#[tauri::command]
pub async fn get_logs(
    app_handle: tauri::AppHandle,
    project_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<LogEntry>, String> {
    let state = app_handle.state::<Arc<AppState>>();

    let uuid = match project_id {
        Some(id) => Some(Uuid::parse_str(&id).map_err(|e| e.to_string())?),
        None => None,
    };

    Ok(state.get_logs(uuid, limit.unwrap_or(100)).await)
}

#[tauri::command]
pub async fn clear_logs(
    app_handle: tauri::AppHandle,
    project_id: Option<String>,
) -> Result<(), String> {
    let state = app_handle.state::<Arc<AppState>>();

    let uuid = match project_id {
        Some(id) => Some(Uuid::parse_str(&id).map_err(|e| e.to_string())?),
        None => None,
    };

    state.clear_logs(uuid).await;
    Ok(())
}

#[tauri::command]
pub async fn reveal_in_finder(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to reveal in finder: {}", e))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Not supported on this OS".to_string())
    }
}

// Supabase API commands

#[tauri::command]
pub async fn run_query(
    app_handle: tauri::AppHandle,
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
    app_handle: tauri::AppHandle,
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
    app_handle: tauri::AppHandle,
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

// Supabase Logs API commands

#[tauri::command]
pub async fn query_supabase_logs(
    app_handle: tauri::AppHandle,
    project_id: String,
    sql: Option<String>,
    iso_timestamp_start: Option<String>,
    iso_timestamp_end: Option<String>,
) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
        .ok_or("Project not linked to Supabase")?;

    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    api.query_logs(
        &project_ref,
        sql.as_deref(),
        iso_timestamp_start.as_deref(),
        iso_timestamp_end.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_edge_function_logs(
    app_handle: tauri::AppHandle,
    project_id: String,
    function_name: Option<String>,
    minutes: Option<u32>,
) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
        .ok_or("Project not linked to Supabase")?;

    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    api.get_edge_function_logs(&project_ref, function_name.as_deref(), minutes.unwrap_or(60))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_postgres_logs(
    app_handle: tauri::AppHandle,
    project_id: String,
    minutes: Option<u32>,
) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
        .ok_or("Project not linked to Supabase")?;

    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    api.get_postgres_logs(&project_ref, minutes.unwrap_or(60))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_auth_logs(
    app_handle: tauri::AppHandle,
    project_id: String,
    minutes: Option<u32>,
) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
    let project_ref = project
        .supabase_project_ref
        .ok_or("Project not linked to Supabase")?;

    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    api.get_auth_logs(&project_ref, minutes.unwrap_or(60))
        .await
        .map_err(|e| e.to_string())
}
