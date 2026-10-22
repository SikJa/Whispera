//! Explicit local text-to-speech. Text is never persisted, logged, or sent to a provider.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{io::{BufRead, BufReader, Write}, process::{Child, ChildStdin, Command, Stdio}, sync::{mpsc, Mutex, atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering}}, time::Duration};
use tauri::{Manager, Emitter};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

pub use crate::reader_language::Voice;
use crate::reader_language::{self, Analysis, Favorites, AUTO};
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Status { pub supported: bool, pub phase: String, #[serde(default)] pub voices: Vec<Voice>, #[serde(default)] pub exporting: bool, #[serde(default)] pub has_reading: bool }
#[derive(Clone, Deserialize, Serialize)]
pub struct Reading { pub text: String, pub voice: String, pub rate: i32 }
struct Session { child: Child, stdin: ChildStdin, replies: mpsc::Receiver<String> }
impl Drop for Session { fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); } }
#[derive(Default)]
pub struct Reader(Mutex<Option<Session>>, Mutex<()>, AtomicU64, AtomicBool, Mutex<Option<Reading>>);
pub struct TrayMenu {
    pub pause: tauri::menu::MenuItem<tauri::Wry>,
    pub stop: tauri::menu::MenuItem<tauri::Wry>,
    pub export: tauri::menu::MenuItem<tauri::Wry>,
    pub cancel_export: tauri::menu::MenuItem<tauri::Wry>,
    phase: AtomicU8,
}
impl TrayMenu {
    pub fn new(app: &tauri::AppHandle) -> tauri::Result<Self> {
        Ok(Self {
            pause: tauri::menu::MenuItem::with_id(app,"reader-pause","Pausar lectura",false,None::<&str>)?,
            stop: tauri::menu::MenuItem::with_id(app,"reader-stop","Detener lectura",false,None::<&str>)?,
            export: tauri::menu::MenuItem::with_id(app,"reader-export","Guardar lectura como MP3…",false,None::<&str>)?,
            cancel_export: tauri::menu::MenuItem::with_id(app,"reader-export-cancel","Cancelar guardado MP3",false,None::<&str>)?,
            phase: AtomicU8::new(0),
        })
    }
}
fn sync_tray(app: &tauri::AppHandle, phase: &str) {
    let Some(menu) = app.try_state::<TrayMenu>() else { return; };
    let phase = match phase { "speaking" => 1, "paused" => 2, _ => 0 };
    if menu.phase.swap(phase,Ordering::SeqCst) == phase { return; }
    let app_copy = app.clone();
    // Never wait for the UI thread while holding the helper lock: closing the
    // settings window can stop the reader on that same UI thread.
    let _ = app.run_on_main_thread(move || {
        let menu = app_copy.state::<TrayMenu>();
        if menu.phase.load(Ordering::SeqCst) != phase { return; }
        let _ = menu.pause.set_text(if phase == 2 { "Continuar lectura" } else { "Pausar lectura" });
        let _ = menu.pause.set_enabled(phase != 0);
        let _ = menu.stop.set_enabled(phase != 0);
    });
}
pub fn start_tray_monitor(app: &tauri::AppHandle) {
    if !cfg!(windows) { return; }
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));
        let reader = app.state::<Reader>();
        let Ok(mut slot) = reader.0.try_lock() else { continue; };
        // Observe completion even with Settings hidden; never start a helper
        // just to refresh the tray, or recreate one after Stop.
        let Some(session) = slot.as_mut() else { continue; };
        match exchange(session,&json!({"action":"status"})) {
            Ok(status) => sync_tray(&app,&status.phase),
            Err(_) => { *slot = None; sync_tray(&app,"idle"); }
        }
    });
}
pub fn control_from_tray(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    if id == "reader-stop" { stop(app); return Ok(()); }
    let reader = app.state::<Reader>();
    let mut slot = reader.0.lock().map_err(|_| "Lector ocupado.")?;
    let Some(session) = slot.as_mut() else { sync_tray(app,"idle"); return Ok(()); };
    let result: Result<Status,String> = (|| {
        let status = exchange(session,&json!({"action":"status"}))?;
        let action = match status.phase.as_str() { "speaking" => "pause", "paused" => "resume", _ => return Ok(status) };
        exchange(session,&json!({"action":action}))
    })();
    match result {
        Ok(status) => { sync_tray(app,&status.phase); Ok(()) }
        Err(error) => { *slot = None; sync_tray(app,"idle"); Err(error) }
    }
}
#[derive(Serialize)]
pub struct Preferences { pub hotkey: String, pub voice: String, pub rate: i32, pub favorites: Favorites }
pub fn hotkey(app: &tauri::AppHandle) -> String {
    app.state::<crate::storage::Store>().get::<Option<String>>("reader_hotkey").unwrap_or_default().unwrap_or_else(|| "Control+Alt+KeyR".into())
}
#[tauri::command]
pub fn reader_preferences(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<Preferences, String> {
    authorize(&window)?; Ok(preferences(&app))
}
fn preferences(app: &tauri::AppHandle) -> Preferences {
    let store = app.state::<crate::storage::Store>();
    let voice: String = store.get("reader_voice").unwrap_or_default();
    Preferences { hotkey: hotkey(app), voice: if voice.is_empty(){AUTO.into()}else{voice}, rate: store.get("reader_rate").unwrap_or_default(), favorites: store.get("reader_language_voices").unwrap_or_default() }
}
#[tauri::command]
pub fn reader_configure(app: tauri::AppHandle, window: tauri::WebviewWindow, voice: String, rate: i32, favorites: Option<Favorites>) -> Result<(), String> {
    authorize(&window)?; validate("preferences", rate)?;
    if voice.len() > 512 { return Err("Voz no válida.".into()); }
    let mut changes = vec![("reader_voice",json!(voice)),("reader_rate",json!(rate))];
    if let Some(favorites) = favorites { reader_language::validate_favorites(&favorites)?;changes.push(("reader_language_voices",json!(favorites))); }
    app.state::<crate::storage::Store>().put_many(&changes)
}
#[tauri::command]
pub async fn reader_analyze(app: tauri::AppHandle, window: tauri::WebviewWindow, text: String) -> Result<Analysis, String> {
    authorize(&window)?;validate(&text,0)?;
    tauri::async_runtime::spawn_blocking(move || {
        let voices = request(&app,json!({"action":"status"}))?.voices;
        Ok(reader_language::analyze(&text,AUTO,&voices,&preferences(&app).favorites))
    }).await.map_err(|_|"No se pudo comprobar el idioma.".to_string())?
}
pub(crate) fn resolve_reading(app: &tauri::AppHandle, mut reading: Reading) -> Result<Reading, String> {
    validate(&reading.text,reading.rate)?;
    let voices = request(app,json!({"action":"status"}))?.voices;
    reading.voice = reader_language::analyze(&reading.text,&reading.voice,&voices,&preferences(app).favorites).require_voice()?.id;
    Ok(reading)
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
    let voice = reader_language::analyze(&text,&prefs.voice,&status.voices,&prefs.favorites).require_voice()?;
    if reader.2.load(Ordering::SeqCst) != ticket { return Ok(()); }
    request_guarded(app,json!({"action":"speak","text":text,"voice":voice.id,"rate":prefs.rate}),Some(ticket))?;
    Ok(())
}
pub fn report_shortcut_error(app: &tauri::AppHandle, error: String) {
    let _ = crate::open_settings(app.clone());
    let _ = app.emit_to("main","reader-shortcut-error",error);
}
pub(crate) fn authorize(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "main" { return Err("Abrí Lectura desde Configuración.".into()); } Ok(())
}
pub(crate) fn validate(text: &str, rate: i32) -> Result<(), String> {
    if text.trim().is_empty() || text.encode_utf16().count() > 50000 || text.contains('\0') { return Err("Usá entre 1 y 50.000 caracteres de texto.".into()); }
    if !(-10..=10).contains(&rate) { return Err("Velocidad no válida.".into()); } Ok(())
}
#[cfg(windows)]
pub(crate) fn binary(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(if cfg!(debug_assertions) {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/reader/WhisperaReader.exe")
    } else { app.path().resource_dir().map_err(|e| e.to_string())?.join("bin/WhisperaReader.exe") })
}
#[cfg(windows)]
fn launch(app: &tauri::AppHandle) -> Result<Session, String> {
    use std::os::windows::process::CommandExt;
    let mut child = Command::new(binary(app)?).creation_flags(0x08000000)
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
    let mut result = exchange(slot.as_mut().unwrap(),&value);
    // A disconnected/timed-out helper must not leave stale audio playing.
    if result.is_err() { *slot = None; }
    if let Ok(status) = &mut result {
        if value["action"] == "speak" {
            if let Ok(reading) = serde_json::from_value::<Reading>(value) {
                *reader.4.lock().map_err(|_|"Lectura ocupada.")? = Some(reading);
                crate::reader_export::sync_tray(app);
            }
        }
        status.has_reading = current(app).is_some();
        status.exporting = crate::reader_export::busy(app);
    }
    sync_tray(app,result.as_ref().map(|s|s.phase.as_str()).unwrap_or("idle"));
    result
}
fn exchange(session: &mut Session, value: &Value) -> Result<Status, String> {
        writeln!(session.stdin, "{value}").map_err(|_| "El lector local se desconectó.")?;
        session.stdin.flush().map_err(|_| "El lector local se desconectó.")?;
        let line = session.replies.recv_timeout(Duration::from_secs(8)).map_err(|_| "El lector local no respondió a tiempo.")?;
        let reply: Value = serde_json::from_str(line.trim_start_matches('\u{feff}')).map_err(|_| "Respuesta del lector no válida.")?;
        if reply["ok"] != true { return Err(reply["error"].as_str().unwrap_or("No se pudo completar la lectura.").to_owned()); }
        let mut status: Status = serde_json::from_value(reply).map_err(|_| "Estado del lector no válido.".to_string())?;
        for voice in &mut status.voices {voice.code=reader_language::canonical(&voice.code).to_owned();}
        Ok(status)
}
#[tauri::command]
pub async fn reader_status(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<Status, String> {
    authorize(&window)?;
    if !cfg!(windows) { return Ok(Status { supported: false, phase: "unavailable".into(), ..Status::default() }); }
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
        let reading = resolve_reading(&app,Reading{text,voice,rate})?;
        app.state::<Reader>().2.fetch_add(1, Ordering::SeqCst);
        request(&app, json!({"action":"speak","text":reading.text,"voice":reading.voice,"rate":reading.rate}))
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
        if let Ok(mut session) = reader.0.lock() { *session = None; sync_tray(app,"idle"); }
    }
}
// Only the latest successful reading is retained in memory, including shortcut selections.
pub(crate) fn current(app: &tauri::AppHandle) -> Option<Reading> {
    app.state::<Reader>().4.lock().ok().and_then(|value|value.clone())
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
