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

/// One window, two menus: the full one for the tray icon and a short one for the
/// floating folder (clipboard, settings). Each has its own size.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
enum Mode {
    #[default]
    Tray,
    Folder,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Mode::Tray => "tray",
            Mode::Folder => "folder",
        }
    }
    fn parse(name: &str) -> Option<Self> {
        [Mode::Tray, Mode::Folder].into_iter().find(|mode| mode.name() == name)
    }
    fn index(self) -> usize {
        self as usize
    }
    /// Wide enough for three tiles with their shortcuts on one line.
    fn width(self) -> f64 {
        match self {
            Mode::Tray => 344.,
            Mode::Folder => 240.,
        }
    }
    /// Height used until the page reports its own.
    fn min_height(self) -> f64 {
        match self {
            Mode::Tray => 200.,
            Mode::Folder => 150.,
        }
    }
}

#[derive(Default)]
pub struct TrayMenu {
    // A click on the tray icon first blurs (and hides) an open menu; the same
    // click must not reopen it, so the icon behaves as a toggle.
    hidden_at: Mutex<Option<Instant>>,
    // Which menu the window shows, and the natural height of each (by `Mode::index`).
    mode: Mutex<Mode>,
    heights: Mutex<[f64; 2]>,
    // True when the menu opened above the cursor: it keeps its bottom edge when it resizes.
    upward: Mutex<bool>,
}

/// Top-left corner of a `w`×`h` menu next to the cursor, inside the work area
/// `(left, top, right, bottom)`, and whether it opened upward.
/// The tray icon sits on a taskbar, so its menu prefers to open above the cursor;
/// a context menu opens below it and flips only when there is no room.
fn place(cursor: (i32, i32), work: (i32, i32, i32, i32), size: (i32, i32), prefer_up: bool) -> ((i32, i32), bool) {
    let ((cx, cy), (left, top, right, bottom), (w, h)) = (cursor, work, size);
    let x = (if cx + w <= right { cx } else { cx - w }).clamp(left, (right - w).max(left));
    let upward = if prefer_up { cy - h >= top } else { cy + h > bottom && cy - h >= top };
    let y = (if upward { cy - h } else { cy }).clamp(top, (bottom - h).max(top));
    ((x, y), upward)
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
        .inner_size(Mode::Tray.width(), 460.)
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
    if window(app)?.is_visible().unwrap_or(false) {
        return hide(app);
    }
    open(app, cursor, Mode::Tray, true)
}

/// Right-click on the floating folder: the short menu, opened under the cursor.
/// Never a toggle: a second right-click just moves it.
fn show_at_cursor(app: &tauri::AppHandle, cursor: tauri::PhysicalPosition<f64>) -> Result<(), String> {
    open(app, cursor, Mode::Folder, false)
}

fn open(app: &tauri::AppHandle, cursor: tauri::PhysicalPosition<f64>, mode: Mode, prefer_up: bool) -> Result<(), String> {
    let state = app.state::<TrayMenu>();
    let window = window(app)?;
    let height = state.heights.lock().map_err(|_| "Menú ocupado")?[mode.index()].max(mode.min_height());
    let monitor = app
        .monitor_from_point(cursor.x, cursor.y)
        .map_err(|e| e.to_string())?
        .or(app.primary_monitor().map_err(|e| e.to_string())?)
        .ok_or("No hay pantalla disponible")?;
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let (w, h) = ((mode.width() * scale).round() as i32, (height * scale).round() as i32);
    let (left, top) = (work.position.x, work.position.y);
    let (right, bottom) = (left + work.size.width as i32, top + work.size.height as i32);
    // Open toward the screen's interior: above a bottom taskbar, left of a right one.
    let ((x, y), upward) = place((cursor.x.round() as i32, cursor.y.round() as i32), (left, top, right, bottom), (w, h), prefer_up);
    *state.upward.lock().map_err(|_| "Menú ocupado")? = upward;
    *state.mode.lock().map_err(|_| "Menú ocupado")? = mode;
    window.set_size(tauri::PhysicalSize::new(w as u32, h as u32)).map_err(|e| e.to_string())?;
    window.set_position(tauri::PhysicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    let _ = window.emit("tray-menu-open", mode.name());
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

/// Hide the floating folder. A "no microphone" notice goes with it, so it does
/// not come back the next time the folder is shown.
pub fn close_recorder(app: &tauri::AppHandle) {
    app.state::<crate::engine::Engine>().recorder.change(|s| {
        if s.phase == "idle" {
            s.error.clear();
        }
    });
    if let Some(recorder) = app.get_webview_window("recorder") {
        let _ = recorder.hide();
    }
}

/// The folder swallows the webview's own right-click menu and asks for ours.
/// Async so a first-time window build never runs on the main thread.
#[tauri::command]
pub async fn recorder_menu(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "recorder" {
        return Err("Acción no válida".into());
    }
    let cursor = window.cursor_position().map_err(|e| e.to_string())?;
    show_at_cursor(&app, cursor)
}

/// The "X" on the folder's error notice (no microphone, no model...).
#[tauri::command]
pub fn recorder_close(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "recorder" {
        return Err("Acción no válida".into());
    }
    close_recorder(&app);
    Ok(())
}

pub fn hide(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        if window.is_visible().unwrap_or(false) {
            *app.state::<TrayMenu>().hidden_at.lock().map_err(|_| "Menú ocupado")? = Some(Instant::now());
        }
        // The page empties itself first, so the other menu never flashes its old content.
        let _ = window.emit("tray-menu-close", ());
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
        // Lets a page that loaded after the open event still know which menu to draw.
        "mode": app.state::<TrayMenu>().mode.lock().map(|mode| mode.name()).unwrap_or("tray"),
        "phase": phase,
        "videoBusy": crate::screen::busy(&app),
        "update": crate::updates::available_version(&app),
        "theme": settings.app_theme,
        "background": settings.app_background,
        "shortcuts": {
            "recorder": shortcut_keys(&settings.hotkey),
            "video": shortcut_keys(&screen.hotkey),
            "capture": shortcut_keys(&screen.image_hotkey),
            "library": shortcut_keys(&crate::library::hotkey(&app)),
        },
    })
}

/// The page reports its natural height so the window hugs the card.
#[tauri::command]
pub fn tray_menu_fit(app: tauri::AppHandle, window: tauri::WebviewWindow, height: f64, mode: String) -> Result<(), String> {
    let Some(mode) = Mode::parse(&mode) else { return Err("Menú no válido".into()) };
    if window.label() != LABEL || !height.is_finite() || !(120. ..=900.).contains(&height) {
        return Err("Tamaño de menú no válido".into());
    }
    let state = app.state::<TrayMenu>();
    state.heights.lock().map_err(|_| "Menú ocupado")?[mode.index()] = height;
    // A page still drawing the previous menu must not resize the one on screen.
    if *state.mode.lock().map_err(|_| "Menú ocupado")? != mode {
        return Ok(());
    }
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let target = (height * scale).round() as u32;
    if size.height != target && window.is_visible().unwrap_or(false) {
        // Keep the edge nearest the cursor fixed: the bottom when opened upward, the top otherwise.
        let upward = *app.state::<TrayMenu>().upward.lock().map_err(|_| "Menú ocupado")?;
        let position = window.outer_position().map_err(|e| e.to_string())?;
        window.set_size(tauri::PhysicalSize::new(size.width, target)).map_err(|e| e.to_string())?;
        let y = if upward { position.y + size.height as i32 - target as i32 } else { position.y };
        window.set_position(tauri::PhysicalPosition::new(position.x, y)).map_err(|e| e.to_string())?;
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
                app.exit(0);
            });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // 1920×1040 work area (taskbar at the bottom), 312×460 menu.
    const WORK: (i32, i32, i32, i32) = (0, 0, 1920, 1040);
    const MENU: (i32, i32) = (312, 460);

    #[test]
    fn tray_menu_opens_above_the_cursor() {
        assert_eq!(place((1800, 1040), WORK, MENU, true), ((1488, 580), true));
        // No room above (taskbar on top): it drops below.
        assert_eq!(place((100, 20), WORK, MENU, true), ((100, 20), false));
    }

    #[test]
    fn context_menu_opens_below_the_cursor() {
        assert_eq!(place((300, 200), WORK, MENU, false), ((300, 200), false));
        // Near the right edge it flips to the left of the cursor.
        assert_eq!(place((1800, 200), WORK, MENU, false), ((1488, 200), false));
    }

    #[test]
    fn context_menu_flips_up_only_when_below_does_not_fit() {
        assert_eq!(place((300, 900), WORK, MENU, false), ((300, 440), true));
        // Fits neither way: it stays inside the screen instead of leaving it.
        assert_eq!(place((300, 300), WORK, (312, 1200), false), ((300, 0), false));
    }

    #[test]
    fn menu_modes_round_trip_and_differ_in_size() {
        for mode in [Mode::Tray, Mode::Folder] {
            assert_eq!(Mode::parse(mode.name()), Some(mode));
        }
        assert_eq!(Mode::parse("other"), None);
        assert!(Mode::Folder.width() < Mode::Tray.width());
        assert_ne!(Mode::Tray.index(), Mode::Folder.index());
    }

    #[test]
    fn shortcuts_become_keycaps() {
        assert_eq!(shortcut_keys("Control+Shift+Space"), ["Ctrl", "Shift", "Espacio"]);
        assert_eq!(shortcut_keys("Alt+KeyZ"), ["Alt", "Z"]);
        assert_eq!(shortcut_keys("Super+Digit4"), [if cfg!(target_os = "macos") { "Cmd" } else { "Win" }, "4"]);
        assert_eq!(shortcut_keys("|"), ["|"]);
        assert!(shortcut_keys("").is_empty());
    }
}
