use std::str::FromStr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

#[derive(Default)]
pub struct Capture {
    pub active: AtomicBool,
    gate: Mutex<()>,
}

pub fn capture(app: &tauri::AppHandle, active: bool) -> Result<(), String> {
    let state = app.state::<Capture>();
    let _gate = state.gate.lock().map_err(|_| "Atajos ocupados")?;
    if active == state.active.load(Ordering::SeqCst) {
        return Ok(());
    }
    if active
        && !app
            .get_webview_window("main")
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false)
    {
        return Err("Enfoca el campo del atajo".into());
    }
    if active
        && (app.state::<crate::screen::Screen>().busy()
            || crate::health::busy(
                &app.state::<crate::engine::Engine>()
                    .recorder
                    .snapshot()
                    .phase,
            ))
    {
        return Err("Termina la grabacion antes de cambiar el atajo".into());
    }
    let store = app.state::<crate::storage::Store>();
    let settings: crate::storage::Settings = store.get("settings")?;
    let screen: crate::screen::Preferences = store.get("screen_preferences").unwrap_or_default();
    let values = [settings.hotkey, screen.hotkey, screen.image_hotkey];
    state.active.store(true, Ordering::SeqCst);
    if active {
        // Windows consumes registered shortcuts before a focused input sees them.
        // Temporarily release only Whispera's keys, including the current combo.
        for value in &values {
            let result = parse(value).and_then(|key| {
                if app.global_shortcut().is_registered(key) {
                    app.global_shortcut()
                        .unregister(key)
                        .map_err(|e| e.to_string())
                } else {
                    Ok(())
                }
            });
            if let Err(error) = result {
                for value in &values {
                    let _ = register(app, value);
                }
                state.active.store(false, Ordering::SeqCst);
                return Err(error);
            }
        }
    } else {
        let mut failure = None;
        for value in &values {
            if let Err(error) = register(app, value) {
                failure = Some(error);
            }
        }
        state.active.store(false, Ordering::SeqCst);
        if let Some(error) = failure {
            return Err(error);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn shortcut_capture(
    active: bool,
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Configura el atajo desde Configuracion".into());
    }
    if active && !window.is_focused().map_err(|e| e.to_string())? {
        return Err("Enfoca el campo del atajo".into());
    }
    tauri::async_runtime::spawn_blocking(move || capture(&app, active))
        .await
        .map_err(|e| e.to_string())?
}
pub fn parse(value: &str) -> Result<Shortcut, String> {
    if value.chars().count() != 1 {
        return Shortcut::from_str(value)
            .map_err(|_| "Atajo invalido. Ejemplo: Control+Shift+Space".into());
    }
    let character = value.chars().next().unwrap();
    let code = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::VkKeyScanW(character as u16) };
    if code == -1 {
        return Err("Esa tecla no existe en la distribucion de Windows".into());
    }
    let key = code as u16 & 255;
    let name = match key {
        65..=90 => format!("Key{}", char::from_u32(key as u32).unwrap()),
        48..=57 => format!("Digit{}", char::from_u32(key as u32).unwrap()),
        32 => "Space".into(),
        186 => "Semicolon".into(),
        187 => "Equal".into(),
        188 => "Comma".into(),
        189 => "Minus".into(),
        190 => "Period".into(),
        191 => "Slash".into(),
        192 => "Backquote".into(),
        219 => "BracketLeft".into(),
        220 => "Backslash".into(),
        221 => "BracketRight".into(),
        222 => "Quote".into(),
        226 => "IntlBackslash".into(),
        _ => return Err("Usa un atajo como Control+Shift+Space".into()),
    };
    let flags = code as u16 >> 8;
    let mut parts = vec![];
    if flags & 2 != 0 {
        parts.push("Control");
    }
    if flags & 4 != 0 {
        parts.push("Alt");
    }
    if flags & 1 != 0 {
        parts.push("Shift");
    }
    parts.push(&name);
    Shortcut::from_str(&parts.join("+")).map_err(|_| "No se pudo registrar esa tecla".into())
}
pub fn register(app: &tauri::AppHandle, value: &str) -> Result<(), String> {
    let key = parse(value)?;
    if app.global_shortcut().is_registered(key) {
        return Ok(());
    }
    app.global_shortcut().register(key).map_err(|_| {
        "Otra aplicacion usa ese atajo. Cierra la Whispera anterior o elige otro.".to_string()
    })?;
    Ok(())
}
pub fn update(app: &tauri::AppHandle, old: &str, new: &str) -> Result<(), String> {
    register(app, new)?;
    if let (Ok(a), Ok(b)) = (parse(old), parse(new)) {
        if a != b {
            let _ = app.global_shortcut().unregister(a);
        }
    }
    app.state::<crate::storage::Store>()
        .event("Atajo global registrado")?;
    Ok(())
}
