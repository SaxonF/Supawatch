use std::sync::Mutex;
use std::time::Duration;
use tauri::{
    async_runtime::JoinHandle,
    image::Image,
    tray::{MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Runtime,
};
use tauri_nspanel::ManagerExt;
use once_cell::sync::Lazy;

use crate::fns::position_menubar_panel;

static ROTATION_TASK: Lazy<Mutex<Option<JoinHandle<()>>>> = Lazy::new(|| Mutex::new(None));

pub fn create(app_handle: &AppHandle) -> tauri::Result<TrayIcon> {
    let icon = Image::from_bytes(include_bytes!("../icons/tray.png"))?;

    TrayIconBuilder::with_id("tray")
        .icon(icon)
        .icon_as_template(true)
        .on_tray_icon_event(|tray, event| {
            let app_handle = tray.app_handle();

            if let TrayIconEvent::Click { button_state, .. } = event {
                if button_state == MouseButtonState::Up {
                    let panel = app_handle.get_webview_panel("main").unwrap();

                    if panel.is_visible() {
                        panel.order_out(None);
                        return;
                    }

                    position_menubar_panel(app_handle, 30.0);

                    panel.show();
                }
            }
        })
        .build(app_handle)
}

pub fn update_icon<R: Runtime>(app_handle: &AppHandle<R>, is_syncing: bool) {
    if is_syncing {
        start_rotation(app_handle);
    } else {
        stop_rotation(app_handle);
    }
}

fn start_rotation<R: Runtime>(app_handle: &AppHandle<R>) {
    let mut task_guard = ROTATION_TASK.lock().unwrap();
    if task_guard.is_some() {
        return; // Already rotating
    }

    let app_handle = app_handle.clone();
    
    let handle = tauri::async_runtime::spawn(async move {
        // Load the base image
        let tray_syncing_bytes = include_bytes!("../icons/tray-syncing.png");
        let mut angle = 0;
        
        loop {
            // Load fresh every time to avoid accumulation of artifacts/complexity, 
            // though rotating the same buffer would be efficient, loading from memory is fast enough.
            // Actually, let's keep one DynamicImage and rotate it.
            if let Ok(mut dyn_image) = image::load_from_memory(tray_syncing_bytes) {
                // Apply rotation
                 match angle {
                    90 => dyn_image = dyn_image.rotate90(),
                    180 => dyn_image = dyn_image.rotate180(),
                    270 => dyn_image = dyn_image.rotate270(),
                    _ => {} // 0, do nothing
                }

                // Convert to RGBA for Tauri
                let rgba = dyn_image.to_rgba8();
                let width = rgba.width();
                let height = rgba.height();
                let rgba_bytes = rgba.into_raw();

                if let Some(tray) = app_handle.tray_by_id("tray") {
                     let icon = Image::new(&rgba_bytes, width, height);
                     let _ = tray.set_icon(Some(icon));
                }
            }

            // Update angle for next frame
            angle = (angle + 90) % 360;

            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    });

    *task_guard = Some(handle);
}

fn stop_rotation<R: Runtime>(app_handle: &AppHandle<R>) {
    let mut task_guard = ROTATION_TASK.lock().unwrap();
    if let Some(handle) = task_guard.take() {
        handle.abort();
    }

    // Reset to static icon
    if let Some(tray) = app_handle.tray_by_id("tray") {
        let icon_bytes = include_bytes!("../icons/tray.png");
        if let Ok(icon) = Image::from_bytes(icon_bytes) {
            let _ = tray.set_icon(Some(icon));
        }
    }
}
