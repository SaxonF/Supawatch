//! Shared synchronization logic for schema and edge function operations.
//!
//! This module consolidates common code used by both manual commands and
//! the file watcher for syncing with Supabase.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::models::{LogEntry, LogSource};
use crate::state::AppState;
use crate::supabase_api::SupabaseApi;

// ============================================================================
// Edge Function File Operations
// ============================================================================

/// Struct representing a changed edge function
#[derive(Debug, Clone, serde::Serialize)]
pub struct EdgeFunctionDiff {
    pub slug: String,
    pub name: String,
    pub path: String, // Relative path from project root
}

/// Collect all source files in a function directory recursively.
/// Returns a list of (relative_path, content) pairs.
pub async fn collect_function_files(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut files = Vec::new();
    collect_files_recursive(dir, dir, &mut files).await?;
    Ok(files)
}

/// Compute the diff of edge functions (local vs deployed state).
/// Returns a list of functions that have changed or are new.
/// Note: This relies on local state (.harbor_hash files), not remote API state.
pub async fn compute_edge_functions_diff(
    project_local_path: &Path,
) -> Result<Vec<EdgeFunctionDiff>, String> {
    let functions_dir = project_local_path.join("supabase").join("functions");
    if !functions_dir.exists() {
        return Ok(Vec::new());
    }

    let mut changed_functions = Vec::new();
    let mut entries = tokio::fs::read_dir(&functions_dir)
        .await
        .map_err(|e| e.to_string())?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let function_slug = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        // Skip shared folders (starting with _) like _shared
        if function_slug.starts_with('_') {
            continue;
        }

        // Collect all files
        let files = match collect_function_files(&path).await {
            Ok(f) => f,
            Err(_) => continue, // Skip unreadable
        };

        if files.is_empty() {
            continue;
        }

        // Compute local hash
        let local_hash = compute_files_hash(&files);

        // Check stored hash
        let hash_file = path.join(".harbor_hash");
        let is_changed = match tokio::fs::read_to_string(&hash_file).await {
            Ok(stored_hash) => stored_hash.trim() != local_hash,
            Err(_) => true, // No hash = new or not deployed
        };

        if is_changed {
            changed_functions.push(EdgeFunctionDiff {
                slug: function_slug.clone(),
                name: function_slug.clone(), // Name is usually slug
                path: format!("supabase/functions/{}", function_slug),
            });
        }
    }

    // Sort by name for deterministic output
    changed_functions.sort_by(|a, b| a.slug.cmp(&b.slug));

    Ok(changed_functions)
}

#[async_recursion::async_recursion]
async fn collect_files_recursive(
    base: &Path,
    current: &Path,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), String> {
    let mut entries = tokio::fs::read_dir(current)
        .await
        .map_err(|e| e.to_string())?;

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
                let relative = path
                    .strip_prefix(base)
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

/// Compute a hash of all files for change detection.
pub fn compute_files_hash(files: &[(String, Vec<u8>)]) -> String {
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

/// Determine the entrypoint file for an edge function.
pub fn determine_entrypoint(files: &[(String, Vec<u8>)]) -> String {
    if files.iter().any(|(p, _)| p == "index.ts") {
        "index.ts".to_string()
    } else if files.iter().any(|(p, _)| p == "index.js") {
        "index.js".to_string()
    } else {
        files
            .first()
            .map(|(p, _)| p.clone())
            .unwrap_or_else(|| "index.ts".to_string())
    }
}

// ============================================================================
// Edge Function Download
// ============================================================================

/// Download and save edge function files from Supabase.
/// This handles the different formats (multipart, text, eszip) that Supabase returns.
pub async fn download_edge_function(
    api: &SupabaseApi,
    project_ref: &str,
    func_slug: &str,
    func_dir: &Path,
) -> Result<bool, String> {
    let body = api.get_function_body(project_ref, func_slug).await.map_err(|e| e.to_string())?;
    let mut saved_files = false;

    // First: try to use multipart files if available (best option)
    if !body.files.is_empty() {
        println!(
            "[DEBUG] Got {} multipart files for {}",
            body.files.len(),
            func_slug
        );
        for file in &body.files {
            // Strip leading "source/" or "src/" prefix if present
            let file_name = file
                .name
                .strip_prefix("source/")
                .or_else(|| file.name.strip_prefix("src/"))
                .unwrap_or(&file.name);
            let file_path = func_dir.join(file_name);
            // Create any subdirectories if needed (for nested files)
            if let Some(parent) = file_path.parent() {
                if parent != func_dir {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
            }
            let _ = tokio::fs::write(&file_path, &file.content).await;
            println!("[DEBUG] Wrote {} for {}", file_name, func_slug);
        }
        // Clean up old eszip if we got real files
        let _ = tokio::fs::remove_file(func_dir.join("function.eszip")).await;
        // Clean up old source folder if it exists
        let _ = tokio::fs::remove_dir_all(func_dir.join("source")).await;
        saved_files = true;
    }

    // Second: if no multipart files, check if it's plain text TypeScript
    if !saved_files && (body.content_type.contains("text/") || body.content_type.contains("typescript")) {
        let _ = tokio::fs::write(func_dir.join("index.ts"), &body.data).await;
        let _ = tokio::fs::remove_file(func_dir.join("function.eszip")).await;
        saved_files = true;
    }

    // Third: try eszip unpacking as last resort
    if !saved_files
        && (body.content_type == "application/vnd.denoland.eszip"
            || body.content_type == "application/octet-stream")
    {
        let reader = futures::io::Cursor::new(body.data.clone());
        let buf_reader = futures::io::BufReader::new(reader);

        if let Ok((eszip, loader)) = eszip::EszipV2::parse(buf_reader).await {
            let loader_handle = tokio::spawn(async move { loader.await });
            let specifiers = eszip.specifiers();
            println!(
                "[DEBUG] Found {} specifiers in eszip for {}",
                specifiers.len(),
                func_slug
            );

            // Try file:/// specifiers first (older format)
            for specifier in &specifiers {
                if specifier.starts_with("file:///") {
                    let path_str = specifier.trim_start_matches("file:///");
                    if !path_str.contains("node_modules") && !path_str.contains("deno_dir") {
                        if let Some(module) = eszip.get_module(specifier) {
                            if let Some(source) = module.source().await {
                                let out_name = std::path::Path::new(path_str)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("index.ts");
                                let _ = tokio::fs::write(func_dir.join(out_name), source).await;
                                saved_files = true;
                            }
                        }
                    }
                }
            }

            loader_handle.abort();

            if saved_files {
                let _ = tokio::fs::remove_file(func_dir.join("function.eszip")).await;
            }
        }
    }

    // Fallback: save the raw data
    if !saved_files {
        let _ = tokio::fs::remove_file(func_dir.join("index.ts")).await;
        let _ = tokio::fs::write(func_dir.join("function.eszip"), &body.data).await;
        let notice = format!(
            "// The source code for function '{}' could not be unpacked.\n// The deployed bundle has been downloaded as 'function.eszip'.",
            func_slug
        );
        let _ = tokio::fs::write(func_dir.join("index.ts"), notice).await;
    }

    Ok(true)
}

/// Pull all edge functions from a Supabase project.
pub async fn pull_edge_functions(
    api: &SupabaseApi,
    project_ref: &str,
    project_id: Option<Uuid>,
    project_local_path: &Path,
    state: &AppState,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let log = LogEntry::info(
        project_id,
        LogSource::System,
        "Syncing edge functions...".to_string(),
    );
    state.add_log(log.clone()).await;
    app_handle.emit("log", &log).ok();

    match api.list_functions(project_ref).await {
        Ok(funcs) => {
            let supabase_dir = project_local_path.join("supabase");
            let functions_dir = supabase_dir.join("functions");
            if !functions_dir.exists() {
                tokio::fs::create_dir_all(&functions_dir)
                    .await
                    .map_err(|e| e.to_string())?;
            }

            let mut func_count = 0;
            for func in funcs {
                let func_dir = functions_dir.join(&func.slug);
                if !func_dir.exists() {
                    tokio::fs::create_dir_all(&func_dir)
                        .await
                        .map_err(|e| e.to_string())?;
                }

                match download_edge_function(api, project_ref, &func.slug, &func_dir).await {
                    Ok(_) => {
                        func_count += 1;
                    }
                    Err(e) => {
                        let log = LogEntry::error(
                            project_id,
                            LogSource::System,
                            format!("Failed to download function {}: {}", func.slug, e),
                        );
                        state.add_log(log).await;
                    }
                }
            }

            if func_count > 0 {
                let log = LogEntry::success(
                    project_id,
                    LogSource::System,
                    format!("Synced {} edge functions", func_count),
                );
                state.add_log(log.clone()).await;
                app_handle.emit("log", &log).ok();
            }
        }
        Err(e) => {
            let log = LogEntry::error(
                project_id,
                LogSource::System,
                format!("Failed to list functions: {}", e),
            );
            state.add_log(log).await;
        }
    }
    Ok(())
}

// ============================================================================
// Schema Path Resolution
// ============================================================================

/// Represents where the schema is stored — either a single file or a directory of split files.
#[derive(Debug, Clone)]
pub enum SchemaSource {
    SingleFile(PathBuf),
    Directory(PathBuf),
}

/// Find the schema source, preferring the split directory layout over a single file.
///
/// Checks for:
/// 1. `supabase/schemas/` directory containing `.sql` files (split layout)
/// 2. `supabase/schemas/schema.sql` (single file in schemas dir)
/// 3. `supabase/schema.sql` (legacy single file location)
pub fn find_schema_source(project_local_path: &Path) -> Option<SchemaSource> {
    let schemas_dir = project_local_path.join("supabase/schemas");

    // Check for split directory layout first
    if schemas_dir.exists() && schemas_dir.is_dir() {
        // Check if directory contains split files (numbered .sql files, not just schema.sql)
        if let Ok(entries) = std::fs::read_dir(&schemas_dir) {
            let has_split_files = entries
                .filter_map(|e| e.ok())
                .any(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.ends_with(".sql") && name != "schema.sql"
                });

            if has_split_files {
                return Some(SchemaSource::Directory(schemas_dir));
            }
        }
    }

    // Fall back to single file
    find_schema_path(project_local_path).map(SchemaSource::SingleFile)
}

/// Find the schema file path, checking multiple standard locations.
/// This is a backwards-compatible wrapper; prefer `find_schema_source()` for new code.
pub fn find_schema_path(project_local_path: &Path) -> Option<PathBuf> {
    let schema_paths = [
        project_local_path.join("supabase/schemas/schema.sql"),
        project_local_path.join("supabase/schema.sql"),
    ];

    schema_paths.into_iter().find(|p| p.exists())
}

/// Read all `.sql` files from a schema directory, sorted alphabetically.
/// Returns a list of (filename, content) tuples.
pub async fn read_schema_dir(dir: &Path) -> Result<Vec<(String, String)>, String> {
    let mut entries: Vec<PathBuf> = Vec::new();

    let mut read_dir = tokio::fs::read_dir(dir)
        .await
        .map_err(|e| format!("Failed to read schema directory: {}", e))?;

    while let Some(entry) = read_dir.next_entry().await.map_err(|e| e.to_string())? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("sql") {
            entries.push(path);
        }
    }

    // Sort alphabetically by filename (numeric prefixes give dependency order)
    entries.sort_by(|a, b| {
        a.file_name()
            .unwrap_or_default()
            .cmp(&b.file_name().unwrap_or_default())
    });

    let mut files = Vec::new();
    for path in entries {
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        files.push((filename, content));
    }

    Ok(files)
}

/// Read the schema SQL from a `SchemaSource` — either a single file or directory.
/// Returns a list of (filename, content) tuples.
pub async fn read_schema_source(source: &SchemaSource) -> Result<Vec<(String, String)>, String> {
    match source {
        SchemaSource::SingleFile(path) => {
            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let content = tokio::fs::read_to_string(path)
                .await
                .map_err(|e| format!("Failed to read schema file: {}", e))?;
            Ok(vec![(filename, content)])
        }
        SchemaSource::Directory(dir) => read_schema_dir(dir).await,
    }
}

/// Find the admin config file path, checking multiple standard locations.
/// Returns the path to admin.json if it exists.
pub fn find_admin_config_path(project_local_path: &Path) -> Option<std::path::PathBuf> {
    let config_paths = [
        project_local_path.join("supabase/admin.json"),
        project_local_path.join("admin.json"),
    ];

    config_paths.into_iter().find(|p| p.exists())
}

/// Get the default path where admin.json should be written.
/// Prefers supabase/admin.json if the supabase folder exists.
pub fn get_admin_config_write_path(project_local_path: &Path) -> std::path::PathBuf {
    let supabase_dir = project_local_path.join("supabase");
    if supabase_dir.exists() {
        supabase_dir.join("admin.json")
    } else {
        project_local_path.join("admin.json")
    }
}

// ============================================================================
// Schema Operations
// ============================================================================

/// Result of computing a schema diff.
pub struct SchemaDiffResult {
    pub diff: crate::diff::SchemaDiff,
    pub local_schema: crate::schema::DbSchema,
    pub migration_sql: String,
}

/// Compute the diff between remote and local schemas.
/// Accepts a `SchemaSource` to support both single file and split directory layouts.
/// Compute the diff between remote and local schemas.
/// Accepts a `SchemaSource` to support both single file and split directory layouts.
pub async fn compute_schema_diff(
    api: &SupabaseApi,
    project_ref: &str,
    source: &SchemaSource,
) -> Result<SchemaDiffResult, String> {
    // 1. Introspect Remote
    let introspector = crate::introspection::Introspector::new(api, project_ref.to_string());
    let remote_schema = introspector.introspect().await?;

    // 2. Parse Local (read from single file or stitch from directory)
    // Now returns Vec<(filename, content)>
    let local_files = read_schema_source(source).await?;
    let local_schema = crate::parsing::parse_schema_sql(&local_files)?;

    // 3. Diff (Remote -> Local)
    let diff = crate::diff::compute_diff(&remote_schema, &local_schema);

    // 4. Generate Migration SQL
    let migration_sql = crate::generator::generate_sql(&diff, &local_schema);

    Ok(SchemaDiffResult {
        diff,
        local_schema,
        migration_sql,
    })
}

// ============================================================================
// TypeScript Generation
// ============================================================================

/// Generate TypeScript types from a schema file and save to the output path.
pub async fn generate_typescript_types(
    schema_path: &Path,
    output_path: &Path,
) -> Result<(), String> {
    // 1. Read and parse the schema
    let local_sql = tokio::fs::read_to_string(schema_path)
        .await
        .map_err(|e| format!("Failed to read schema file: {}", e))?;
    
    let filename = schema_path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let files = vec![(filename, local_sql)];
    
    let schema = crate::parsing::parse_schema_sql(&files)?;

    // 2. Generate TypeScript
    let config = crate::generator::typescript::TypeScriptConfig::default();
    let typescript_content = crate::generator::typescript::generate_typescript(&schema, &config);

    // 3. Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    // 4. Write the TypeScript file
    tokio::fs::write(output_path, typescript_content)
        .await
        .map_err(|e| format!("Failed to write TypeScript file: {}", e))?;

    Ok(())
}

/// Generate TypeScript types from a list of schema files.
pub async fn generate_typescript_types_from_sql(
    files: &[(String, String)],
    output_path: &Path,
) -> Result<(), String> {
    let schema = crate::parsing::parse_schema_sql(files)?;

    let config = crate::generator::typescript::TypeScriptConfig::default();
    let typescript_content = crate::generator::typescript::generate_typescript(&schema, &config);

    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create output directory: {}", e))?;
    }

    tokio::fs::write(output_path, typescript_content)
        .await
        .map_err(|e| format!("Failed to write TypeScript file: {}", e))?;

    Ok(())
}

/// Find the TypeScript output path based on project settings.
/// Uses custom path if provided, otherwise defaults to `<project_path>/src/types/database.ts`
pub fn get_typescript_output_path(
    project_local_path: &Path,
    custom_path: Option<&str>,
) -> std::path::PathBuf {
    match custom_path {
        Some(path) => project_local_path.join(path),
        None => project_local_path
            .join("src")
            .join("types")
            .join("database.ts"),
    }
}
