use serde::Serialize;

#[derive(Serialize)]
pub struct Permissions {
    platform: &'static str,
    accessibility: &'static str,
    screen: &'static str,
    microphone: &'static str,
}

#[cfg(target_os = "macos")]
extern "C" {
    fn whispera_permission_status(kind: i32) -> i32;
    fn whispera_permission_request(kind: i32) -> i32;
}

#[cfg(target_os = "macos")]
fn label(status: i32) -> &'static str {
    match status { 1 => "granted", 2 => "denied", 0 => "required", _ => "unknown" }
}

#[tauri::command]
pub fn platform_permissions() -> Permissions {
    #[cfg(target_os = "macos")]
    { Permissions { platform: "macos", accessibility: label(unsafe { whispera_permission_status(0) }),
        screen: label(unsafe { whispera_permission_status(1) }),
        microphone: label(unsafe { whispera_permission_status(2) }) } }
    #[cfg(not(target_os = "macos"))]
    { Permissions { platform: "windows", accessibility: "not-applicable", screen: "not-applicable", microphone: "not-applicable" } }
}

pub fn ensure_microphone() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if unsafe { whispera_permission_status(2) } != 1 {
        return Err("Permiti el microfono en Configuracion > Aplicacion > Permisos de macOS antes de grabar.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn platform_request_permission(window: tauri::WebviewWindow, kind: String, settings: bool) -> Result<Permissions, String> {
    if window.label() != "main" { return Err("Abri Configuracion para administrar permisos".into()); }
    #[cfg(target_os = "macos")]
    {
        let (id, pane) = match kind.as_str() {
            "accessibility" => (0, "Privacy_Accessibility"),
            "screen" => (1, "Privacy_ScreenCapture"),
            "microphone" => (2, "Privacy_Microphone"),
            _ => return Err("Permiso desconocido".into()),
        };
        if settings {
            crate::platform::open_url(&format!("x-apple.systempreferences:com.apple.preference.security?{pane}"))?;
        } else {
            tauri::async_runtime::spawn_blocking(move || unsafe { whispera_permission_request(id) }).await.map_err(|e| e.to_string())?;
        }
    }
    #[cfg(not(target_os = "macos"))]
    { let _ = (kind, settings); }
    Ok(platform_permissions())
}
