
use tauri::Manager;
#[tauri::command]
pub async fn is_folder_empty(path: String) -> Result<bool, String> {
    let path = std::path::Path::new(&path);
    if !path.exists() {
        return Ok(true);
    }
    
    let mut entries = tokio::fs::read_dir(path).await.map_err(|e| e.to_string())?;
    // If there is at least one entry, it's not empty
    if let Some(_) = entries.next_entry().await.map_err(|e| e.to_string())? {
        Ok(false)
    } else {
        Ok(true)
    }
}

#[tauri::command]
pub async fn get_templates(app_handle: tauri::AppHandle) -> Result<Vec<String>, String> {
    let resource_path = if cfg!(debug_assertions) {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates")
    } else {
        app_handle
            .path()
            .resolve("templates", tauri::path::BaseDirectory::Resource)
            .map_err(|e| e.to_string())?
    };
    
    if !resource_path.exists() {
        return Ok(vec![]);
    }

    let mut templates = Vec::new();
    let mut entries = tokio::fs::read_dir(resource_path).await.map_err(|e| e.to_string())?;
    
    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
       if entry.file_type().await.map_err(|e| e.to_string())?.is_dir() {
           templates.push(entry.file_name().to_string_lossy().to_string());
       }
    }
    
    Ok(templates)
}

#[tauri::command]
pub async fn copy_template(
    app_handle: tauri::AppHandle, 
    template_name: String, 
    target_path: String
) -> Result<(), String> {
    let resource_path = if cfg!(debug_assertions) {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates")
    } else {
        app_handle
            .path()
            .resolve("templates", tauri::path::BaseDirectory::Resource)
            .map_err(|e| e.to_string())?
    };
    let template_path = resource_path.join(&template_name);
    let target_dir = std::path::Path::new(&target_path);

    if !template_path.exists() {
        return Err(format!("Template '{}' not found", template_name));
    }

    if !target_dir.exists() {
         tokio::fs::create_dir_all(target_dir).await.map_err(|e| e.to_string())?;
    }

    // Interactive recursive copy not easily available in fs, so we implement a simple recursive copy or use a crate if available.
    // Since we don't have extra crates, we'll write a simple recursive copy helper.
    // Actually, we can just use `cp -r` for simplicity on mac if we want, but Rust approach is better for portability (though user is on Mac).
    // Let's implement a simple copy_dir_all logic using a stack or recursion.
    
    // Helper function for recursive copy
    fn copy_dir_all(src: impl AsRef<std::path::Path>, dst: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::create_dir_all(&dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_dir() {
                copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
            } else {
                std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
            }
        }
        Ok(())
    }

    // Since the inner helper is synchronous (std::fs), we should wrap it in spawn_blocking
    // or just use tokio recursively. Given specific constraints, spawn_blocking is safer for potentially deep recursion or large IO.
    
    let t_path = template_path.clone();
    let tgt_path = target_dir.to_path_buf();
    
    tauri::async_runtime::spawn_blocking(move || {
        copy_dir_all(t_path, tgt_path)
    }).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;

    Ok(())
}
