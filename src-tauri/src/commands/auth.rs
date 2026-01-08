use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

use crate::models::{LogEntry, LogSource};
use crate::state::AppState;

#[tauri::command]
pub async fn set_access_token(
    app_handle: AppHandle,
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
pub async fn has_access_token(app_handle: AppHandle) -> Result<bool, String> {
    let state = app_handle.state::<Arc<AppState>>();
    Ok(state.has_access_token().await)
}

#[tauri::command]
pub async fn clear_access_token(app_handle: AppHandle) -> Result<(), String> {
    let state = app_handle.state::<Arc<AppState>>();
    state.clear_access_token().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn validate_access_token(app_handle: AppHandle) -> Result<bool, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let api = state.get_api_client().await.map_err(|e| e.to_string())?;

    match api.list_projects().await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}
