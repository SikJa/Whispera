//! The installed autostart plugin owns LaunchAgent registration on macOS.
use std::sync::{Mutex, OnceLock};
use tauri::Manager;
use tauri_plugin_autostart::AutoLaunchManager;

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static CACHE: Mutex<Option<Result<bool, String>>> = Mutex::new(None);

/// Call from setup, after Tauri has initialized its autostart plugin.
pub fn initialize(app: &tauri::AppHandle) -> Result<(), String> {
    if app.try_state::<AutoLaunchManager>().is_none() {
        return Err("El plugin de inicio automatico no esta inicializado".into());
    }
    let _ = APP.set(app.clone());
    Ok(())
}

fn access(refresh: bool, operation: Option<bool>) -> Result<bool, String> {
    let mut cache = CACHE.lock().map_err(|_| "Inicio automatico ocupado")?;
    if !refresh {
        if let Some(value) = cache.as_ref() {
            return value.clone();
        }
    }
    let result = (|| {
        let app = APP
            .get()
            .ok_or("Inicializa startup::initialize antes de consultar el inicio automatico")?;
        let manager = app
            .try_state::<AutoLaunchManager>()
            .ok_or("Plugin de inicio automatico no disponible")?;
        if let Some(enabled) = operation {
            if enabled {
                manager.enable()
            } else {
                manager.disable()
            }
            .map_err(|e| format!("macOS no pudo cambiar el inicio automatico: {e}"))?;
        }
        let actual = manager
            .is_enabled()
            .map_err(|e| format!("No se pudo consultar el inicio automatico: {e}"))?;
        if operation.is_some_and(|requested| requested != actual) {
            return Err("macOS no confirmo el cambio del inicio automatico".into());
        }
        Ok(actual)
    })();
    *cache = Some(result.clone());
    result
}

pub fn enabled() -> Result<bool, String> {
    access(true, None)
}
pub fn cached_enabled() -> Result<bool, String> {
    access(false, None)
}
pub fn set_enabled(enabled: bool) -> Result<bool, String> {
    access(true, Some(enabled))
}
// Never recreate a LaunchAgent that the user disabled or removed in System Settings.
pub fn reconcile() -> Result<bool, String> {
    access(true, None)
}
