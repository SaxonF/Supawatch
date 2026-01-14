use tauri::AppHandle;

#[tauri::command]
pub fn init(_app_handle: AppHandle) {
    // No-op for regular window mode
    // Previously used for menubar panel setup
}

#[tauri::command]
pub async fn pick_project_folder(app_handle: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = tokio::sync::oneshot::channel();

    app_handle
        .dialog()
        .file()
        .set_title("Select Supabase Project Folder")
        .set_directory(dirs::home_dir().unwrap_or_default())
        .pick_folder(move |path| {
            let _ = tx.send(path);
        });

    rx.await
        .map_err(|e| e.to_string())
        .map(|path| path.map(|p| p.to_string()))
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
