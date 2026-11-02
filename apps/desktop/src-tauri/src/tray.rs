//! Tray menu drawn by Whispera itself. A native Win32 menu cannot be styled
//! (fixed bitmaps, misaligned accelerators), so a small prewarmed window opens
//! next to the cursor and closes when it loses focus.
use crate::storage::{Settings, Store};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};

const LABEL: &str = "tray-menu";
const WIDTH: f64 = 312.;

#[derive(Default)]
pub struct TrayMenu {
    // A click on the tray icon first blurs (and hides) an open menu; the same
    // click must not reopen it, so the icon behaves as a toggle.
    hidden_at: Mutex<Option<Instant>>,
    height: Mutex<f64>,
}

/// "Control+Shift+Space" becomes the keycaps ["Ctrl", "Shift", "Espacio"].
pub fn shortcut_keys(value: &str) -> Vec<String> {
    if value.trim().is_empty() {
        return vec![];
    }
    if value.chars().count() == 1 {
        return vec![value.to_uppercase()];
    }
    value
        .split('+')
        .map(|part| match part {
            "Control" | "Ctrl" | "CommandOrControl" => "Ctrl".to_owned(),
            "Super" | "Meta" | "Cmd" | "Command" => if cfg!(target_os = "macos") { "Cmd" } else { "Win" }.to_owned(),
            "Alt" | "Option" => "Alt".to_owned(),
            "Space" => "Espacio".to_owned(),
            "Escape" => "Esc".to_owned(),
            "ArrowUp" => "↑".to_owned(),
            "ArrowDown" => "↓".to_owned(),
            "ArrowLeft" => "←".to_owned(),
            "ArrowRight" => "→".to_owned(),
            other => other
                .strip_prefix("Key")
                .or_else(|| other.strip_prefix("Digit"))
                .filter(|rest| rest.chars().count() == 1)
                .unwrap_or(other)
                .to_owned(),
        })
        .collect()
}

fn window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        return Ok(window);
    }
    let window = tauri::WebviewWindowBuilder::new(app, LABEL, tauri::WebviewUrl::App("tray-menu.html".into()))
        .title("Whispera")
        .inner_size(WIDTH, 460.)
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    crate::floating_window::apply(&window)?;
    Ok(window)
}

/// Create the hidden menu after startup so the first right-click is instant.
pub fn prewarm(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(4)).await;
        let _ = tauri::async_runtime::spawn_blocking(move || window(&app)).await;
    });
}

pub fn show(app: &tauri::AppHandle, cursor: tauri::PhysicalPosition<f64>) -> Result<(), String> {
    let state = app.state::<TrayMenu>();
    if state.hidden_at.lock().map_err(|_| "Menú ocupado")?.is_some_and(|at| at.elapsed() < Duration::from_millis(250)) {
        return Ok(());
    }
    let window = window(app)?;
    if window.is_visible().unwrap_or(false) {
        return hide(app);
    }
    let height = (*state.height.lock().map_err(|_| "Menú ocupado")?).max(200.);
    let monitor = app
        .monitor_from_point(cursor.x, cursor.y)
        .map_err(|e| e.to_string())?
        .or(app.primary_monitor().map_err(|e| e.to_string())?)
        .ok_or("No hay pantalla disponible")?;
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let (w, h) = ((WIDTH * scale).round() as i32, ((height * scale).round() as i32).min(work.size.height as i32));
    let (left, top) = (work.position.x, work.position.y);
    let (right, bottom) = (left + work.size.width as i32, top + work.size.height as i32);
    let (cx, cy) = (cursor.x.round() as i32, cursor.y.round() as i32);
    // Open toward the screen's interior: above a bottom taskbar, left of a right one.
    let x = if cx + w <= right { cx } else { cx - w }.clamp(left, (right - w).max(left));
    let y = if cy - h >= top { cy - h } else { cy }.clamp(top, (bottom - h).max(top));
    window.set_size(tauri::PhysicalSize::new(w as u32, h as u32)).map_err(|e| e.to_string())?;
    window.set_position(tauri::PhysicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    let _ = window.emit("tray-menu-open", ());
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

pub fn hide(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        if window.is_visible().unwrap_or(false) {
            *app.state::<TrayMenu>().hidden_at.lock().map_err(|_| "Menú ocupado")? = Some(Instant::now());
        }
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn tray_menu_state(app: tauri::AppHandle) -> serde_json::Value {
    let store = app.state::<Store>();
    let settings: Settings = store.get("settings").unwrap_or_default();
    let screen: crate::screen::Preferences = store.get("screen_preferences").unwrap_or_default();
    let phase = app.state::<crate::engine::Engine>().recorder.snapshot().phase;
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "phase": phase,
        "videoBusy": crate::screen::busy(&app),
        "update": crate::updates::available_version(&app),
        "theme": settings.app_theme,
        "background": settings.app_background,
        "reader": crate::reader::tray_status(&app),
        "shortcuts": {
            "recorder": shortcut_keys(&settings.hotkey),
            "video": shortcut_keys(&screen.hotkey),
            "capture": shortcut_keys(&screen.image_hotkey),
            "library": shortcut_keys(&crate::library::hotkey(&app)),
            "reading": shortcut_keys(&crate::reader::hotkey(&app)),
        },
    })
}

/// The page reports its natural height so the window hugs the card.
#[tauri::command]
pub fn tray_menu_fit(app: tauri::AppHandle, window: tauri::WebviewWindow, height: f64) -> Result<(), String> {
    if window.label() != LABEL || !height.is_finite() || !(120. ..=900.).contains(&height) {
        return Err("Tamaño de menú no válido".into());
    }
    *app.state::<TrayMenu>().height.lock().map_err(|_| "Menú ocupado")? = height;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let available=window.current_monitor().ok().flatten().map(|m|m.work_area().size.height).unwrap_or(u32::MAX);
    let target = ((height * scale).round() as u32).min(available);
    if size.height != target && window.is_visible().unwrap_or(false) {
        // Keep the edge nearest the cursor fixed (the bottom when opened upward).
        let position = window.outer_position().map_err(|e| e.to_string())?;
        window.set_size(tauri::PhysicalSize::new(size.width, target)).map_err(|e| e.to_string())?;
        window
            .set_position(tauri::PhysicalPosition::new(position.x, position.y + size.height as i32 - target as i32))
            .map_err(|e| e.to_string())?;
    } else if size.height != target {
        window.set_size(tauri::PhysicalSize::new(size.width, target)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn tray_menu_action(app: tauri::AppHandle, window: tauri::WebviewWindow, action: String) -> Result<(), String> {
    if window.label() != LABEL {
        return Err("Acción no válida".into());
    }
    hide(&app)?;
    run(&app, &action);
    Ok(())
}

pub fn run(app: &tauri::AppHandle, action: &str) {
    use crate::{engine, library, lifecycle, replay, screen};
    let app = app.clone();
    match action {
        "ai-usage" | "reading" => {
            let event=if action=="reading" {"reader-open"} else {"ai-usage-open"};
            if crate::open_settings(app.clone()).is_ok() {let _=app.emit_to("main",event,());}
        }
        "reader-pause" | "reader-stop" => {
            let action=action.to_owned();
            tauri::async_runtime::spawn_blocking(move || {if let Err(error)=crate::reader::control_from_tray(&app,&action) {crate::reader::report_shortcut_error(&app,error);}});
        }
        "reader-export-cancel" => crate::reader_export::cancel(&app),
        "reader-export" => {tauri::async_runtime::spawn_blocking(move || {if let Err(error)=crate::reader_export::save(&app,None) {crate::reader::report_shortcut_error(&app,error);}});}

        "library" => { tauri::async_runtime::spawn(async move { let _ = library::library_toggle(app).await; }); }
        "settings" => { if let Err(error) = crate::open_settings(app.clone()) { let _ = app.state::<Store>().event(&error); } }
        "updates" => { let _ = crate::open_updates(app); }
        "recorder" => { tauri::async_runtime::spawn_blocking(move || { let _ = crate::show_recorder(app); }); }
        "import" => {
            tauri::async_runtime::spawn(async move {
                if let Err(error) = crate::open_import(app.clone()).await { let _ = app.state::<Store>().event(&error); }
            });
        }
        "screen" => {
            tauri::async_runtime::spawn_blocking(move || { if let Err(error) = screen::select(&app) { screen::report_error(&app, error); } });
        }
        "screenshot" => {
            tauri::async_runtime::spawn_blocking(move || { if let Err(error) = screen::select_image(&app) { screen::report_image_error(&app, error); } });
        }
        "quit" => {
            lifecycle::record("Quit requested from tray");
            tauri::async_runtime::spawn_blocking(move || {
                let _ = screen::stop(&app);
                engine::shutdown(&app);
                replay::stop(&app);
                crate::reader::stop(&app);
                app.exit(0);
            });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcuts_become_keycaps() {
        assert_eq!(shortcut_keys("Control+Shift+Space"), ["Ctrl", "Shift", "Espacio"]);
        assert_eq!(shortcut_keys("Alt+KeyZ"), ["Alt", "Z"]);
        assert_eq!(shortcut_keys("Super+Digit4"), [if cfg!(target_os = "macos") { "Cmd" } else { "Win" }, "4"]);
        assert_eq!(shortcut_keys("|"), ["|"]);
        assert!(shortcut_keys("").is_empty());
    }
}
