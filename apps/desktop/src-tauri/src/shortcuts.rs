use std::str::FromStr;
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
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
