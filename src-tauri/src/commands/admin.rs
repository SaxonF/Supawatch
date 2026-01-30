use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::state::AppState;
use crate::sync;

/// Default sidebar spec with only "tables" and "scripts" groups.
/// This is used when no admin.json file exists in the project.
pub const DEFAULT_SIDEBAR_SPEC: &str = r#"{
  "groups": [
    {
      "id": "tables",
      "name": "Tables",
      "icon": "table",
      "itemsSource": {
        "type": "sql",
        "value": "SELECT schemaname AS schema, tablename AS name FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename"
      },
      "itemTemplate": {
        "id": ":schema.:name",
        "icon": "table",
        "name": ":name",
        "visible": true,
        "autoRun": true,
        "queries": [
          {
            "source": {
              "type": "sql",
              "value": "SELECT * FROM \":schema\".\":name\" LIMIT 100"
            },
            "results": "table"
          }
        ]
      }
    },
    {
      "id": "scripts",
      "name": "Scripts",
      "icon": "file-text",
      "itemsFromState": "tabs",
      "userCreatable": true,
      "itemTemplate": {
        "id": ":id",
        "name": "Untitled",
        "icon": "file-text",
        "visible": true,
        "queries": [
          {
            "source": {
              "type": "sql",
              "value": ""
            },
            "results": "table"
          }
        ]
      }
    }
  ]
}"#;

/// Check if admin.json exists for a project.
#[tauri::command]
pub async fn has_admin_config(
    app_handle: AppHandle,
    project_id: String,
) -> Result<bool, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let project = state
        .get_project(project_id)
        .await
        .map_err(|e| e.to_string())?;

    let local_path = Path::new(&project.local_path);
    Ok(sync::find_admin_config_path(local_path).is_some())
}

/// Get the sidebar spec for a project.
/// Returns the content of admin.json if it exists, otherwise returns the default spec.
#[tauri::command]
pub async fn get_sidebar_spec(
    app_handle: AppHandle,
    project_id: String,
) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<Arc<AppState>>();
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    // Get the project to find its local path
    let project = state
        .get_project(project_id)
        .await
        .map_err(|e| e.to_string())?;

    let local_path = Path::new(&project.local_path);

    // Try to find and read admin.json
    if let Some(config_path) = sync::find_admin_config_path(local_path) {
        let content = tokio::fs::read_to_string(&config_path)
            .await
            .map_err(|e| format!("Failed to read admin.json: {}", e))?;

        let spec: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse admin.json: {}", e))?;

        return Ok(spec);
    }

    // Return default spec if no admin.json exists
    let default_spec: serde_json::Value = serde_json::from_str(DEFAULT_SIDEBAR_SPEC)
        .map_err(|e| format!("Failed to parse default spec: {}", e))?;

    Ok(default_spec)
}

/// Write the full sidebar spec to admin.json.
#[tauri::command]
pub async fn write_sidebar_spec(
    app_handle: AppHandle,
    project_id: String,
    spec: serde_json::Value,
) -> Result<(), String> {
    let state = app_handle.state::<Arc<AppState>>();
    let project_id = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    #[derive(serde::Serialize, Clone)]
    struct AdminConfigChangedPayload {
        project_id: Uuid,
    }

    // Get the project to find its local path
    let project = state
        .get_project(project_id)
        .await
        .map_err(|e| e.to_string())?;

    let local_path = Path::new(&project.local_path);
    let config_path = sync::get_admin_config_write_path(local_path);

    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // Write the spec with pretty formatting
    let content = serde_json::to_string_pretty(&spec)
        .map_err(|e| format!("Failed to serialize spec: {}", e))?;

    tokio::fs::write(&config_path, content)
        .await
        .map_err(|e| format!("Failed to write admin.json: {}", e))?;

    app_handle
        .emit(
            "admin_config_changed",
            AdminConfigChangedPayload { project_id },
        )
        .ok();

    Ok(())
}

/// Add an item to a group in the sidebar spec.
/// If the group doesn't exist, returns an error.
/// If no admin.json exists, creates one with the default spec plus the new item.
#[tauri::command]
pub async fn add_sidebar_item(
    app_handle: AppHandle,
    project_id: String,
    group_id: String,
    item: serde_json::Value,
) -> Result<(), String> {
    // Get current spec
    let mut spec = get_sidebar_spec(app_handle.clone(), project_id.clone()).await?;

    // Find the group and add the item
    let groups = spec
        .get_mut("groups")
        .and_then(|g| g.as_array_mut())
        .ok_or_else(|| "Invalid spec: missing groups array".to_string())?;

    let group = groups
        .iter_mut()
        .find(|g| g.get("id").and_then(|id| id.as_str()) == Some(&group_id))
        .ok_or_else(|| format!("Group '{}' not found", group_id))?;

    // Get or create the items array
    let items = group
        .as_object_mut()
        .ok_or_else(|| "Invalid group structure".to_string())?
        .entry("items")
        .or_insert_with(|| serde_json::Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| "Invalid items structure".to_string())?;

    items.push(item);

    // Write the updated spec
    write_sidebar_spec(app_handle, project_id, spec).await
}

/// Add a new group to the sidebar spec.
/// If no admin.json exists, creates one with the default spec plus the new group.
#[tauri::command]
pub async fn add_sidebar_group(
    app_handle: AppHandle,
    project_id: String,
    group: serde_json::Value,
) -> Result<(), String> {
    // Get current spec
    let mut spec = get_sidebar_spec(app_handle.clone(), project_id.clone()).await?;

    // Add the group
    let groups = spec
        .get_mut("groups")
        .and_then(|g| g.as_array_mut())
        .ok_or_else(|| "Invalid spec: missing groups array".to_string())?;

    groups.push(group);

    // Write the updated spec
    write_sidebar_spec(app_handle, project_id, spec).await
}
