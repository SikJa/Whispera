use cpal::traits::{DeviceTrait, HostTrait};
use tauri::State;
use tauri_plugin_autostart::ManagerExt;
use crate::{groq, storage::Store};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupInfo {
    complete: bool,
    startup: bool,
    microphone: Option<String>,
}

#[tauri::command]
pub fn setup_info(app: tauri::AppHandle, store: State<Store>) -> Result<SetupInfo, String> {
    Ok(SetupInfo {
        complete: store.get("setup_complete")?,
        startup: app.autolaunch().is_enabled().map_err(|e| e.to_string())?,
        microphone: cpal::default_host().default_input_device().and_then(|d| d.name().ok()),
    })
}

#[tauri::command]
pub fn set_startup(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if enabled { app.autolaunch().enable() } else { app.autolaunch().disable() }
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn complete_setup(store: State<Store>) -> Result<(), String> {
    groq::entry()?.get_password().map_err(|_| "Configure Groq first / Configura Groq primero")?;
    store.put("setup_complete", &true)
}

#[tauri::command]
pub async fn validate_key(key: String) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() || key.len() > 1024 || key.chars().any(char::is_whitespace) {
        return Err("Invalid key / Clave no valida".into());
    }
    let response = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15))
        .build().map_err(|_| "Connection unavailable")?
        .get("https://api.groq.com/openai/v1/models").bearer_auth(key)
        .send().await.map_err(|_| "Cannot reach Groq / No se pudo conectar con Groq")?;
    if !response.status().is_success() {
        return Err(format!("Groq HTTP {}. Check key or retry / Revisa la clave o reintenta", response.status().as_u16()));
    }
    groq::entry()?.set_password(key).map_err(|_| "Cannot save credential / No se pudo guardar la clave".into())
}
