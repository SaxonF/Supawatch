use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::models::{LogEntry, LogSource, Project, RemoteProject, ProjectKeys};
use crate::state::AppState;
use crate::supabase_api::Organization;
use crate::sync;


#[tauri::command]
pub async fn list_remote_projects(
    app_handle: AppHandle,
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

#[tauri::command]
pub async fn list_organizations(app_handle: AppHandle) -> Result<Vec<Organization>, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let api = state.get_api_client().await.map_err(|e| e.to_string())?;
    api.list_organizations()
        .await
        .map_err(|e| format!("Failed to list organizations: {}", e))
}

#[tauri::command]
pub async fn create_project(
    app_handle: AppHandle,
    name: String,
    local_path: String,
    supabase_project_id: Option<String>,
    supabase_project_ref: Option<String>,
    organization_id: Option<String>,
    generate_typescript: Option<bool>,
    typescript_output_path: Option<String>,
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

                            // Generate TypeScript types if enabled
                            if generate_typescript.unwrap_or(true) {
                                let project_path = std::path::Path::new(&local_path);
                                let output_path = sync::get_typescript_output_path(
                                    project_path,
                                    typescript_output_path.as_deref(),
                                );

                                if let Err(e) = sync::generate_typescript_types(&schema_path, &output_path).await {
                                    let log = LogEntry::error(
                                        None,
                                        LogSource::System,
                                        format!("Failed to generate TypeScript types: {}", e),
                                    );
                                    state.add_log(log.clone()).await;
                                    app_handle.emit("log", &log).ok();
                                } else {
                                    let relative_output = output_path
                                        .strip_prefix(project_path)
                                        .unwrap_or(&output_path)
                                        .to_string_lossy();
                                    let log = LogEntry::success(
                                        None,
                                        LogSource::System,
                                        format!("TypeScript types generated: {}", relative_output),
                                    );
                                    state.add_log(log.clone()).await;
                                    app_handle.emit("log", &log).ok();
                                }
                            }
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

    let mut project = if let (Some(pid), Some(pref)) = (project_id, project_ref) {
        Project::with_remote(name, local_path, pid, pref)
    } else {
        Project::new(name, local_path)
    };

    // Apply TypeScript settings if provided
    if let Some(enabled) = generate_typescript {
        project.generate_typescript = enabled;
    }
    if let Some(path) = typescript_output_path {
        project.typescript_output_path = Some(path);
    }

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
pub async fn get_projects(app_handle: AppHandle) -> Result<Vec<Project>, String> {
    let state = app_handle.state::<Arc<AppState>>();
    Ok(state.get_projects().await)
}

#[tauri::command]
pub async fn get_project(app_handle: AppHandle, id: String) -> Result<Project, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.get_project(uuid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_project(
    app_handle: AppHandle,
    project: Project,
) -> Result<Project, String> {
    let state = app_handle.state::<Arc<AppState>>();
    state
        .update_project(project)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_project(app_handle: AppHandle, id: String) -> Result<(), String> {
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
    app_handle: AppHandle,
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

#[tauri::command]
pub async fn get_project_keys(app_handle: AppHandle, project_id: String) -> Result<ProjectKeys, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;

    // Must be linked to a remote project
    let project_ref = project.supabase_project_ref
        .ok_or("Project is not linked to a Supabase project".to_string())?;

    let api = state.get_api_client().await.map_err(|e| e.to_string())?;
    
    // Use cached keys if possible? No, for now fetch fresh to ensure validity
    // or maybe we should cache them in project struct?
    // Requirement says "retrieve API keys via management api".
    // Let's fetch them fresh.
    
    let keys = api.get_api_keys(&project_ref).await.map_err(|e| e.to_string())?;

    let anon_key = keys.iter()
        .find(|k| k.key_type == "publishable" || k.name == "anon")
        .map(|k| k.api_key.clone())
        .ok_or("No anon/publishable key found".to_string())?;

    let service_role_key = keys.iter()
        .find(|k| k.key_type == "secret" || k.name == "service_role")
        .map(|k| k.api_key.clone())
        .ok_or("No service_role key found".to_string())?;

    Ok(ProjectKeys {
        anon_key,
        service_role_key,
    })
}
