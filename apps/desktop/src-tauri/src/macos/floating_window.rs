use super::HitRect;
use std::{
    ffi::c_void,
    sync::mpsc,
    time::{Duration, Instant},
};

const MAIN_TIMEOUT: Duration = Duration::from_secs(2);

unsafe extern "C" {
    fn whispera_window_apply(window: *mut c_void) -> i32;
    fn whispera_window_regions(window: *mut c_void, rectangles: *const f64, count: usize) -> i32;
    fn whispera_window_menu_bar_mode(enabled: i32) -> i32;
}

fn on_window(
    window: &tauri::WebviewWindow,
    operation: impl FnOnce(*mut c_void) -> i32 + Send + 'static,
) -> Result<(), String> {
    let copy = window.clone();
    let (send, receive) = mpsc::sync_channel(1);
    let deadline = Instant::now() + MAIN_TIMEOUT;
    window
        .run_on_main_thread(move || {
            if Instant::now() >= deadline {
                let _ = send.send(Err("La ventana de macOS no respondio".into()));
                return;
            }
            let result = copy.ns_window().map_err(|e| e.to_string()).and_then(|raw| {
                if operation(raw) == 0 {
                    Ok(())
                } else {
                    Err("macOS no pudo configurar la ventana flotante".into())
                }
            });
            let _ = send.send(result);
        })
        .map_err(|e| e.to_string())?;
    receive
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| "La ventana de macOS no respondio".to_string())?
}

pub fn apply(window: &tauri::WebviewWindow) -> Result<(), String> {
    on_window(window, |raw| unsafe { whispera_window_apply(raw) })
}

pub(super) fn region(window: tauri::WebviewWindow, rects: Vec<HitRect>) -> Result<(), String> {
    // Web CSS pixels are AppKit points. Applying backingScaleFactor again breaks Retina.
    let values: Vec<f64> = rects
        .iter()
        .flat_map(|r| [r.x, r.y, r.width, r.height])
        .collect();
    on_window(&window, move |raw| unsafe {
        whispera_window_regions(raw, values.as_ptr(), rects.len())
    })
}

/// Optional setup hook: the existing Tauri tray remains the status-bar owner.
pub fn set_menu_bar_mode(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let (send, receive) = mpsc::sync_channel(1);
    let deadline = Instant::now() + MAIN_TIMEOUT;
    app.run_on_main_thread(move || {
        if Instant::now() >= deadline {
            let _ = send.send(-1);
            return;
        }
        let _ = send.send(unsafe { whispera_window_menu_bar_mode(i32::from(enabled)) });
    })
    .map_err(|e| e.to_string())?;
    match receive.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(0) => Ok(()),
        _ => Err("macOS no pudo cambiar el modo de barra de menus".into()),
    }
}
