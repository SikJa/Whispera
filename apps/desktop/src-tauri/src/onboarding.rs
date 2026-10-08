use cpal::traits::{DeviceTrait, HostTrait};
use tauri::State;
use crate::{groq, storage::Store};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupInfo {
    complete: bool,
    startup: bool,
    microphone: Option<String>,
}

#[tauri::command]
pub async fn setup_info(store: State<'_, Store>) -> Result<SetupInfo, String> {
    let complete=store.get("setup_complete")?;
    tauri::async_runtime::spawn_blocking(move || Ok(SetupInfo {
        complete,
        startup: crate::startup::enabled()?,
        microphone: cpal::default_host().default_input_device().and_then(|d| d.name().ok()),
    })).await.map_err(|e|e.to_string())?
}

#[tauri::command]
pub async fn startup_enabled(window: tauri::WebviewWindow) -> Result<bool, String> {
    if window.label() != "main" { return Err("Abrí Configuración para cambiar el inicio automático".into()); }
    tauri::async_runtime::spawn_blocking(crate::startup::enabled).await.map_err(|e|e.to_string())?
}

#[tauri::command]
pub async fn set_startup(window: tauri::WebviewWindow, enabled: bool) -> Result<bool, String> {
    if window.label() != "main" { return Err("Abrí Configuración para cambiar el inicio automático".into()); }
    tauri::async_runtime::spawn_blocking(move || crate::startup::set_enabled(enabled)).await.map_err(|e|e.to_string())?
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
