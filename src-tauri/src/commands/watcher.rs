use std::sync::Arc;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::models::{LogEntry, LogSource};
use crate::state::AppState;
use crate::watcher;

#[tauri::command]
pub async fn start_watching(
    app_handle: AppHandle,
    project_id: String,
) -> Result<(), String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;

    watcher::start_watching(&app_handle, uuid, &project.local_path).await
}

#[tauri::command]
pub async fn stop_watching(app_handle: AppHandle, project_id: String) -> Result<(), String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    watcher::stop_watching(&app_handle, uuid).await
}

#[tauri::command]
pub async fn is_watching(app_handle: AppHandle, project_id: String) -> Result<bool, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    Ok(state.is_watching(uuid).await)
}
