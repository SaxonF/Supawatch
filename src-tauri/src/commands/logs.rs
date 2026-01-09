use std::sync::Arc;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::models::LogEntry;
use crate::state::AppState;

#[tauri::command]
pub async fn get_logs(
    app_handle: AppHandle,
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
    app_handle: AppHandle,
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
pub async fn query_supabase_logs(
    app_handle: AppHandle,
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
    app_handle: AppHandle,
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
    app_handle: AppHandle,
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
    app_handle: AppHandle,
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
