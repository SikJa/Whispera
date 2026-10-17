//! Explicit local text-to-speech. Text is never persisted, logged, or sent to a provider.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{io::{BufRead, BufReader, Write}, process::{Child, ChildStdin, Command, Stdio}, sync::{mpsc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}}, time::Duration};
use tauri::{Manager, Emitter};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Voice { pub id: String, pub name: String, pub language: String }
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Status { pub supported: bool, pub phase: String, #[serde(default)] pub voices: Vec<Voice> }
struct Session { child: Child, stdin: ChildStdin, replies: mpsc::Receiver<String> }
impl Drop for Session { fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); } }
#[derive(Default)]
pub struct Reader(Mutex<Option<Session>>, Mutex<()>, AtomicU64, AtomicBool);
#[derive(Serialize)]
pub struct Preferences { pub hotkey: String, pub voice: String, pub rate: i32 }
pub fn hotkey(app: &tauri::AppHandle) -> String {
    app.state::<crate::storage::Store>().get::<Option<String>>("reader_hotkey").unwrap_or_default().unwrap_or_else(|| "Control+Alt+KeyR".into())
}
#[tauri::command]
pub fn reader_preferences(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<Preferences, String> {
    authorize(&window)?; Ok(preferences(&app))
}
fn preferences(app: &tauri::AppHandle) -> Preferences {
    let store = app.state::<crate::storage::Store>();
    Preferences { hotkey: hotkey(app), voice: store.get("reader_voice").unwrap_or_default(), rate: store.get("reader_rate").unwrap_or_default() }
}
#[tauri::command]
pub fn reader_configure(app: tauri::AppHandle, window: tauri::WebviewWindow, voice: String, rate: i32) -> Result<(), String> {
    authorize(&window)?; validate("preferences", rate)?;
    if voice.len() > 512 { return Err("Voz no válida.".into()); }
    app.state::<crate::storage::Store>().put_many(&[("reader_voice",json!(voice)),("reader_rate",json!(rate))])
}
pub fn enabled(app: &tauri::AppHandle) -> bool { app.state::<Reader>().3.load(Ordering::SeqCst) }
pub fn registered(app: &tauri::AppHandle, value: bool) { app.state::<Reader>().3.store(value, Ordering::SeqCst); }
pub fn matches(app: &tauri::AppHandle, key: &tauri_plugin_global_shortcut::Shortcut) -> bool {
    app.state::<Reader>().3.load(Ordering::SeqCst) && crate::shortcuts::parse(&hotkey(app)).is_ok_and(|k| k == *key)
}
pub fn initialize(app: &tauri::AppHandle) -> Result<(), String> {
    if !cfg!(windows) { return Ok(()); }
    let key = crate::shortcuts::parse(&hotkey(app))?;
    if app.global_shortcut().is_registered(key) { return Err("El atajo de Lectura coincide con otro. Cambialo en Atajos.".into()); }
    app.global_shortcut().register(key).map_err(|_| "No se pudo registrar el atajo de Lectura. Elegí otro en Atajos.".to_string())?;
    registered(app,true); Ok(())
}
fn available(app: &tauri::AppHandle, engine: &crate::engine::Engine) -> Result<(), String> {
    if crate::health::busy(&engine.recorder.snapshot().phase) || crate::screen::busy(app) || crate::replay::preferences(app).enabled {
        return Err("Terminá el dictado o la captura y desactivá Replay antes de leer en voz alta.".into());
    } Ok(())
}
pub fn toggle_selection(app: &tauri::AppHandle, source: isize) -> Result<(), String> {
    let reader = app.state::<Reader>();
    let ticket = reader.2.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
    let Ok(_operation) = reader.1.try_lock() else { stop(app); return Ok(()); };
    let engine = app.state::<crate::engine::Engine>();
    let _gate = engine.gate.lock().map_err(|_| "Motor ocupado.")?;
    let status = request(app,json!({"action":"status"}))?;
    if ["speaking","paused"].contains(&status.phase.as_str()) { stop(app); return Ok(()); }
    available(app,&engine)?;
    let selection = crate::reader_selection::selected_text(source);
    if reader.2.load(Ordering::SeqCst) != ticket { return Ok(()); }
    let text = selection?;
    let prefs = preferences(app); validate(&text,prefs.rate)?;
    let voice = status.voices.iter().find(|v| v.id == prefs.voice)
        .or_else(|| status.voices.iter().find(|v| v.language.starts_with("es"))).or(status.voices.first())
        .ok_or("No hay voces de escritorio instaladas.")?;
    if reader.2.load(Ordering::SeqCst) != ticket { return Ok(()); }
    request_guarded(app,json!({"action":"speak","text":text,"voice":voice.id,"rate":prefs.rate}),Some(ticket))?;
    Ok(())
}
pub fn report_shortcut_error(app: &tauri::AppHandle, error: String) {
    let _ = crate::open_settings(app.clone());
    let _ = app.emit_to("main","reader-shortcut-error",error);
}
fn authorize(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "main" { return Err("Abrí Lectura desde Configuración.".into()); } Ok(())
}
fn validate(text: &str, rate: i32) -> Result<(), String> {
    if text.trim().is_empty() || text.encode_utf16().count() > 50000 || text.contains('\0') { return Err("Usá entre 1 y 50.000 caracteres de texto.".into()); }
    if !(-10..=10).contains(&rate) { return Err("Velocidad no válida.".into()); } Ok(())
}
#[cfg(windows)]
fn launch(app: &tauri::AppHandle) -> Result<Session, String> {
    use std::os::windows::process::CommandExt;
    let binary = if cfg!(debug_assertions) {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/reader/WhisperaReader.exe")
    } else { app.path().resource_dir().map_err(|e| e.to_string())?.join("bin/WhisperaReader.exe") };
    let mut child = Command::new(binary).creation_flags(0x08000000)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
        .map_err(|_| "No se pudo iniciar el lector local de Windows.".to_string())?;
    let stdin = child.stdin.take().ok_or("Lector sin canal de entrada.")?;
    let stdout = child.stdout.take().ok_or("Lector sin canal de salida.")?;
    let (tx, replies) = mpsc::sync_channel(2);
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break; };
            if line.len() > 256000 || tx.send(line).is_err() { break; }
        }
    });
    Ok(Session { child, stdin, replies })
}
#[cfg(not(windows))]
fn launch(_: &tauri::AppHandle) -> Result<Session, String> { Err("La lectura local de esta versión está disponible en Windows.".into()) }
fn request(app: &tauri::AppHandle, value: Value) -> Result<Status, String> { request_guarded(app,value,None) }
fn request_guarded(app: &tauri::AppHandle, value: Value, ticket: Option<u64>) -> Result<Status, String> {
    let reader = app.state::<Reader>();
    let mut slot = reader.0.lock().map_err(|_| "Lector ocupado.")?;
    if ticket.is_some_and(|ticket| reader.2.load(Ordering::SeqCst) != ticket) { return Ok(Status::default()); }
    if slot.is_none() { *slot = Some(launch(app)?); }
    let session = slot.as_mut().unwrap();
    let result = (|| {
        writeln!(session.stdin, "{value}").map_err(|_| "El lector local se desconectó.")?;
        session.stdin.flush().map_err(|_| "El lector local se desconectó.")?;
        let line = session.replies.recv_timeout(Duration::from_secs(8)).map_err(|_| "El lector local no respondió a tiempo.")?;
        let reply: Value = serde_json::from_str(line.trim_start_matches('\u{feff}')).map_err(|_| "Respuesta del lector no válida.")?;
        if reply["ok"] != true { return Err(reply["error"].as_str().unwrap_or("No se pudo completar la lectura.").to_owned()); }
        serde_json::from_value(reply).map_err(|_| "Estado del lector no válido.".into())
    })();
    // A disconnected/timed-out helper must not leave stale audio playing.
    if result.is_err() { *slot = None; }
    result
}
#[tauri::command]
pub async fn reader_status(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<Status, String> {
    authorize(&window)?;
    if !cfg!(windows) { return Ok(Status { supported: false, phase: "unavailable".into(), voices: vec![] }); }
    tauri::async_runtime::spawn_blocking(move || request(&app, json!({"action":"status"}))).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn reader_speak(app: tauri::AppHandle, window: tauri::WebviewWindow, text: String, voice: String, rate: i32) -> Result<Status, String> {
    authorize(&window)?; validate(&text, rate)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _work = crate::updates::work(&app)?;
        let engine = app.state::<crate::engine::Engine>();
        let _gate = engine.gate.lock().map_err(|_| "Motor ocupado.")?;
        available(&app,&engine)?;
        app.state::<Reader>().2.fetch_add(1, Ordering::SeqCst);
        request(&app, json!({"action":"speak","text":text,"voice":voice,"rate":rate}))
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn reader_control(app: tauri::AppHandle, window: tauri::WebviewWindow, action: String) -> Result<Status, String> {
    authorize(&window)?;
    if !["pause", "resume", "stop"].contains(&action.as_str()) { return Err("Acción de lectura no admitida.".into()); }
    tauri::async_runtime::spawn_blocking(move || {
        if action == "stop" { stop(&app); return request(&app,json!({"action":"status"})); }
        request(&app, json!({"action":action}))
    }).await.map_err(|e| e.to_string())?
}
// Killing our own helper stops synthesis immediately, without changing other audio.
pub fn stop(app: &tauri::AppHandle) {
    if let Some(reader) = app.try_state::<Reader>() {
        reader.2.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut session) = reader.0.lock() { *session = None; }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn input_is_literal_and_bounded_in_windows_utf16_units() {
        assert!(validate("<speak>literal</speak> & $(text)", 0).is_ok());
        assert!(validate(" ", 0).is_err()); assert!(validate("nul\0text", 0).is_err());
        assert!(validate(&"a".repeat(50000), -10).is_ok());
        assert!(validate(&"😀".repeat(25001), 0).is_err());
        for rate in [-11, 11] { assert!(validate("Texto", rate).is_err()); }
    }
}
