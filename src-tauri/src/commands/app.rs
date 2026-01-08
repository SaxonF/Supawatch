use std::sync::Once;
use tauri::{AppHandle, Emitter, Manager};

use crate::fns::{
    setup_menubar_panel_listeners, swizzle_to_menubar_panel, update_menubar_appearance,
    IS_DIALOG_OPEN,
};

static INIT: Once = Once::new();

#[tauri::command]
pub fn init(app_handle: AppHandle) {
    INIT.call_once(|| {
        swizzle_to_menubar_panel(&app_handle);

        update_menubar_appearance(&app_handle);

        setup_menubar_panel_listeners(&app_handle);
    });
}

#[tauri::command]
pub fn show_menubar_panel(app_handle: AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub async fn pick_project_folder(app_handle: AppHandle) -> Result<Option<String>, String> {
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
