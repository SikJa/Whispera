#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod replay;
mod reader;
mod reader_language;
mod reader_export;
mod ai_usage;
mod reader_selection;
#[cfg(all(test, windows))]
mod audit_tests;
mod platform;
mod permissions;
mod replay_audio;
mod encoder_job;
mod video_trim;
mod updates;
mod upstream_updates;
mod update_transport;
mod audio;
mod capture_history;
mod capture_finalize;
mod capture_session;
mod library;
mod clipboard_events;
mod native_drag;
mod library_media;
mod video_transcript;
mod screen_capture;
mod engine;
mod floating_window;
mod groq;
mod local_asr;
mod video_timing;
mod lossless_upload;
mod image_clipboard;
#[cfg(test)]
mod timestamp_bench;
mod health;
mod lifecycle;
mod incremental;
mod upload_tail;
mod retention;
mod unification;
mod legacy;
mod onboarding;
mod startup;
mod paste;
mod paste_focus;
mod video_audio_clock;
mod profile;
mod windows_capture_keys;
mod screen;
mod screen_editor;
mod capture_docks;
mod shortcuts;
mod sounds;
mod storage;
mod system_audio;
mod tray;
use storage::{Rule, Settings, Snapshot, Store};
use tauri::{Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[tauri::command]
fn open_groq_console() -> Result<(), String> {
    platform::open_url("https://console.groq.com/keys")
}

#[tauri::command]
fn snapshot(store: State<Store>) -> Result<Snapshot, String> {
    Ok(Snapshot {
        settings: store.get("settings")?,
        rules: store.get("rules")?,
        history: store.history()?,
        key_configured: groq::entry()
            .and_then(|e| e.get_password().map_err(|e| e.to_string()))
            .is_ok(),
        logs: store.logs()?,
        native: true,
    })
}
#[tauri::command]
async fn audio_input_devices() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(audio::input_devices).await.unwrap_or_default()
}
#[tauri::command]
fn local_models() -> local_asr::Status {
    local_asr::status()
}
#[tauri::command]
async fn local_model_install(id: String) -> Result<(), String> {
    local_asr::install(id).await
}
#[tauri::command]
fn local_model_cancel() {
    local_asr::cancel()
}
#[tauri::command]
fn local_model_remove(id: String) -> Result<(), String> {
    local_asr::remove(&id)
}
#[tauri::command]
fn read_settings(store: State<Store>) -> Result<Settings, String> {
    store.get("settings")
}
#[tauri::command]
fn read_history(store: State<Store>) -> Result<Vec<storage::Transcript>, String> {
    store.history()
}
#[tauri::command]
fn save_transcript(
    id: String,
    text: String,
    store: State<Store>,
    engine: State<engine::Engine>,
) -> Result<(), String> {
    store.update_transcript(&id, &text)?;
    engine.recorder.change(|state| {
        if state.session == id && state.phase == "done" {
            state.text = text;
        }
    });
    Ok(())
}
#[tauri::command]
fn save_settings(
    settings: Settings,
    store: State<Store>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    settings.validate()?;
    let old: Settings = store.get("settings")?;
    let video: screen::Preferences = store.get("screen_preferences").unwrap_or_default();
    // An unparseable video/capture key cannot collide; it must not block saving.
    let dictation = shortcuts::parse(&settings.hotkey)?;
    if [&video.hotkey, &video.image_hotkey]
        .into_iter()
        .any(|value| shortcuts::parse(value).is_ok_and(|key| key == dictation))
    {
        return Err("El atajo de dictado debe ser distinto de los de video y captura".into());
    }
    shortcuts::update(&app, &old.hotkey, &settings.hotkey)?;
    store.put("settings", &settings)?;
    local_asr::preload(&settings);
    Ok(())
}
#[tauri::command]
fn read_rules(store: State<Store>) -> Result<Vec<Rule>, String> {
    store.get("rules")
}
#[tauri::command]
fn save_rules(rules: Vec<Rule>, expected_rules: Vec<Rule>, store: State<Store>) -> Result<(), String> {
    groq::validate_rules(&rules)?;
    store.replace_rules(&rules, &expected_rules)
}
#[tauri::command]
fn save_api_key(key: String) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() || key.len() > 1024 || key.chars().any(char::is_whitespace) {
        return Err("Clave no valida".into());
    }
    groq::entry()?
        .set_password(key)
        .map_err(|_| "El sistema no pudo guardar la credencial de forma segura".into())
}
#[tauri::command]
async fn transcribe_file(
    path: String,
    app: tauri::AppHandle,
    store: State<'_, Store>,
) -> Result<String, String> {
    let _updating = updates::work(&app)?;
    let engine = app.state::<engine::Engine>();
    {
        let _lock = engine.gate.lock().map_err(|_| "Motor ocupado")?;
        if health::busy(&engine.recorder.snapshot().phase)
            || engine
                .processing
                .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return Err("Termina la grabacion o transcripcion actual antes de importar".into());
        }
    }
    struct Permit<'a>(&'a std::sync::atomic::AtomicBool);
    impl Drop for Permit<'_> {
        fn drop(&mut self) {
            self.0.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _permit = Permit(&engine.processing);
    let settings: Settings = store.get("settings")?;
    let rules: Vec<Rule> = store.get("rules")?;
    store.event("Transcripcion de archivo iniciada")?;
    let source = std::path::PathBuf::from(&path);
    let binary = screen::ffmpeg(&app).ok();
    let local=settings.transcription_engine=="local";
    let compact = tauri::async_runtime::spawn_blocking(move || if local {local_import(binary.as_deref(), &source)} else {compact_audio(binary.as_deref(), &source)})
        .await
        .map_err(|e| e.to_string())??;
    let upload = compact.as_ref().map(|c| c.0.join(if local {"audio.wav"} else {"audio.ogg"})).unwrap_or_else(|| path.clone().into());
    match groq::transcribe(&upload, &settings, &rules).await {
        Ok(text) => {
            store.append(&text)?;
            store.event("Transcripcion guardada en historial")?;
            if settings.auto_copy && app.clipboard().write_text(&text).is_err() {
                store.event("No se pudo copiar. El texto permanece en Historial.")?;
            }
            Ok(text)
        }
        Err(e) => {
            let _ = store.event(&e);
            Err(e)
        }
    }
}
struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}
fn local_import(binary: Option<&std::path::Path>, source: &std::path::Path) -> Result<Option<Scratch>, String> {
    if source.extension().is_some_and(|e|e.eq_ignore_ascii_case("wav")) {return Ok(None);}
    let binary=binary.ok_or("Falta el motor para convertir el audio local")?;
    let scratch=Scratch(std::env::temp_dir().join(format!("whispera-local-import-{}",uuid::Uuid::new_v4())));
    std::fs::create_dir(&scratch.0).map_err(|e|e.to_string())?;
    let output=scratch.0.join("audio.wav");
    let status=screen::command(binary).args(["-hide_banner","-loglevel","error","-y","-i"]).arg(source)
        .args(["-vn","-ac","1","-ar","16000","-c:a","pcm_s16le"]).arg(&output)
        .stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map_err(|e|e.to_string())?;
    if !status.success()||!output.is_file(){return Err("No se pudo convertir el archivo para leerlo con el motor local. No se envió a Groq.".into());}
    Ok(Some(scratch))
}
// Groq accepts 25 MB and a few containers. Long recordings or video files are
// converted to 16 kHz mono Opus (~100 minutes per 24 MB); the original is untouched.
fn compact_audio(binary: Option<&std::path::Path>, source: &std::path::Path) -> Result<Option<Scratch>, String> {
    let extension = source.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let size = std::fs::metadata(source).map_err(|_| "No se pudo abrir el audio")?.len();
    let supported = ["wav", "mp3", "m4a", "ogg", "webm", "flac", "mp4", "mpeg", "mpga"].contains(&extension.as_str());
    if supported && size <= 24_000_000 { return Ok(None); }
    let binary = binary.ok_or("Falta el motor de audio para convertir este archivo")?;
    let scratch = Scratch(std::env::temp_dir().join(format!("whispera-import-{}", uuid::Uuid::new_v4())));
    std::fs::create_dir(&scratch.0).map_err(|e| e.to_string())?;
    let output = scratch.0.join("audio.ogg");
    let status = screen::command(binary)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"]).arg(source)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "libopus", "-b:a", "24k", "-application", "voip"])
        .arg(&output)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() || !output.is_file() {
        return Err("No se pudo leer el audio de ese archivo".into());
    }
    if std::fs::metadata(&output).map_err(|e| e.to_string())?.len() > 24_000_000 {
        return Err("El audio dura demasiado para un solo envío (más de ~2 horas). Dividilo en partes.".into());
    }
    Ok(Some(scratch))
}
#[tauri::command]
fn import_legacy(path: String, store: State<Store>) -> Result<String, String> {
    legacy::import(&store, std::path::Path::new(&path))
}

#[tauri::command]
fn import_groq_key(path: String) -> Result<(), String> {
    let path = std::path::Path::new(&path);
    let mut key = String::new();
    if let Ok(values) = dotenvy::from_path_iter(path.join(".env")) {
        for (name, value) in values.flatten() {
            if name == "GROQ_API_KEY" {
                key = value;
                break;
            }
        }
    }
    if key.is_empty() {
        if let Ok(bytes) = std::fs::read(path.join("whispera.settings.json")) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                key = value
                    .get("groq_api_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into();
            }
        }
    }
    if key.is_empty() {
        return Err("No se encontro una clave Groq en esa carpeta".into());
    }
    save_api_key(key)
}
#[tauri::command]
fn copy_text(text: String, app: tauri::AppHandle) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}
#[tauri::command]
async fn open_recorder(app: tauri::AppHandle, visible: Option<bool>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || prepare_recorder(app, visible.unwrap_or(true)))
        .await
        .map_err(|e| e.to_string())?
}
fn show_recorder(app: tauri::AppHandle) -> Result<(), String> {
    prepare_recorder(app, true)
}
#[tauri::command]
async fn toggle_recorder(window: tauri::WebviewWindow, app: tauri::AppHandle) -> Result<(), String> {
    if window.label() != "main" { return Err("Control disponible únicamente en Configuración".into()); }
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(recorder) = app.get_webview_window("recorder") {
            if recorder.is_visible().map_err(|e| e.to_string())? {
                return recorder.hide().map_err(|e| e.to_string());
            }
        }
        prepare_recorder(app, true)
    }).await.map_err(|e| e.to_string())?
}
fn prepare_recorder(app: tauri::AppHandle, visible: bool) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("recorder") {
        if visible {
            w.show().map_err(|e| e.to_string())?;
        }
        floating_window::apply(&w)?;
        return Ok(());
    }
    let window = tauri::WebviewWindowBuilder::new(
        &app,
        "recorder",
        tauri::WebviewUrl::App("index.html?view=record".into()),
    )
    .title("Whispera · Grabadora")
    .inner_size(520., 520.)
    .transparent(true)
    .decorations(false)
    .shadow(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .resizable(false)
    .focused(false)
    .visible(false)
    .build()
    .map_err(|e| e.to_string())?;
    floating_window::apply(&window)?;
    if visible {
        window.show().map_err(|e| e.to_string())?;
    }
    floating_window::apply(&window)?;
    Ok(())
}

#[tauri::command]
fn settings_window_action(window:tauri::WebviewWindow,action:String)->Result<bool,String>{
    if window.label()!="main"{return Err("Control disponible únicamente en Configuración".into());}
    match action.as_str(){
        "state"=>{},
        "minimize"=>window.minimize().map_err(|e|e.to_string())?,
        "maximize"=>{if window.is_maximized().map_err(|e|e.to_string())?{window.unmaximize().map_err(|e|e.to_string())?;}else{window.maximize().map_err(|e|e.to_string())?;}},
        "close"=>window.close().map_err(|e|e.to_string())?,
        _=>return Err("Acción de ventana no válida".into()),
    }
    window.is_maximized().map_err(|e|e.to_string())
}
#[tauri::command]
fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    let w = app
        .get_webview_window("main")
        .ok_or("Configuracion no disponible")?;
    if let Some(library) = app.get_webview_window("library") { let _ = library.hide(); }
    w.unminimize().map_err(|e| e.to_string())?;
    w.show().map_err(|e| e.to_string())?;
    let window = w.clone();
    w.run_on_main_thread(move || {
        #[cfg(windows)]
        if let Ok(handle) = window.hwnd() {
            // Restore even if an external hide left the toolkit visibility flag stale.
            unsafe {
                use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::{ShowWindow, SetForegroundWindow, SW_RESTORE}};
                let _ = ShowWindow(HWND(handle.0), SW_RESTORE);
                let _ = SetForegroundWindow(HWND(handle.0));
            }
        }
        let _ = window.set_focus();
    }).map_err(|e| e.to_string())
}
#[tauri::command]
fn open_updates(app: tauri::AppHandle) -> Result<(), String> {
    open_settings(app.clone())?;
    tauri::Emitter::emit_to(&app, "main", "updater-open", ()).map_err(|e| e.to_string())
}
#[tauri::command]
async fn open_recording_details(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(w) = app.get_webview_window("details") {
            w.show().map_err(|e| e.to_string())?;
            return w.set_focus().map_err(|e| e.to_string());
        }
        tauri::WebviewWindowBuilder::new(
            &app,
            "details",
            tauri::WebviewUrl::App("index.html?view=details".into()),
        )
        .title("Whispera - Audios y transcripcion")
        .inner_size(700., 640.)
        .min_inner_size(540., 480.)
        .build()
        .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn open_import(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(w) = app.get_webview_window("import") {
            w.show().map_err(|e| e.to_string())?;
            return w.set_focus().map_err(|e| e.to_string());
        }
        tauri::WebviewWindowBuilder::new(
            &app,
            "import",
            tauri::WebviewUrl::App("index.html?view=import".into()),
        )
        .title("Whispera - Transcribir audio")
        .inner_size(480., 430.)
        .min_inner_size(420., 360.)
        .build()
        .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

static APP_STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
fn main() {
    let _ = APP_STARTED.set(std::time::Instant::now());
    // A drag helper never opens the profile, registers shortcuts, or starts audio.
    if let Some(code) = native_drag::helper_entry() { std::process::exit(code); }
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().app_name("Whispera").args(["--autostart"]).build())
        // A second autostart stays in the tray, but opening Whispera from Start or the
        // desktop while it already runs must show something instead of doing nothing.
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            // At sign-in Windows can start Whispera twice (autostart plus an old
            // Startup shortcut); that duplicate must not open a window.
            let early = APP_STARTED.get().is_none_or(|at| at.elapsed() < std::time::Duration::from_secs(30));
            if !early && !args.iter().any(|arg| arg == "--autostart") {
                if let Err(error) = open_settings(app.clone()) { let _ = app.state::<Store>().event(&error); }
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(reader::Reader::default())
        .manage(reader::TrayMenu::default())
        .manage(reader_export::Exports::default())
        .manage(ai_usage::Usage::default())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if app
                        .state::<shortcuts::Capture>()
                        .active
                        .load(std::sync::atomic::Ordering::SeqCst)
                    {
                        return;
                    }
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        let app = app.clone();
                        if reader::matches(&app, shortcut) {
                            let source = reader_selection::foreground();
                            tauri::async_runtime::spawn_blocking(move || {
                                if let Err(error) = reader::toggle_selection(&app, source) { reader::report_shortcut_error(&app,error); }
                            });
                            return;
                        }
                        if shortcuts::parse(&library::hotkey(&app)).map(|key| key == *shortcut).unwrap_or(false) {
                            tauri::async_runtime::spawn(async move { let _ = library::library_toggle(app).await; });
                            return;
                        }
                        if shortcuts::parse(&replay::hotkey(&app)).map(|key| key == *shortcut).unwrap_or(false) {
                            tauri::async_runtime::spawn_blocking(move || { if let Err(error)=replay::save(&app){ replay::report_error(&app,error); } });
                            return;
                        }
                        if shortcuts::parse("Escape")
                            .map(|key| key == *shortcut)
                            .unwrap_or(false)
                            && (app.state::<screen::Screen>().busy() || app.state::<capture_session::ImageCapture>().screen.busy())
                        {
                            tauri::async_runtime::spawn_blocking(move || {
                                if let Err(error) = screen::escape(&app) {
                                    screen::report_error(&app, error);
                                }
                            });
                            return;
                        }
                        let preferences: screen::Preferences = app
                            .state::<Store>()
                            .get("screen_preferences")
                            .unwrap_or_default();
                        if shortcuts::parse(&preferences.image_hotkey)
                            .map(|key| key == *shortcut)
                            .unwrap_or(false)
                        {
                            tauri::async_runtime::spawn_blocking(move || {
                                if let Err(error) = screen::select_image(&app) {
                                    screen::report_image_error(&app, error);
                                }
                            });
                            return;
                        }
                        if shortcuts::parse(&preferences.hotkey)
                            .map(|key| key == *shortcut)
                            .unwrap_or(false)
                        {
                            tauri::async_runtime::spawn_blocking(move || {
                                if let Err(error) = screen::toggle(&app) {
                                    screen::report_error(&app, error);
                                }
                            });
                            return;
                        }
                        let settings: Settings = match app.state::<Store>().get("settings") {
                            Ok(value) => value,
                            Err(_) => return,
                        };
                        if shortcuts::parse(&settings.hotkey)
                            .map(|key| key != *shortcut)
                            .unwrap_or(true)
                        {
                            return;
                        }
                        tauri::async_runtime::spawn_blocking(move || {
                            let phase = app.state::<engine::Engine>().recorder.snapshot().phase;
                            if phase == "processing" {
                                return;
                            }
                            let action = if ["recording", "paused"].contains(&phase.as_str()) {
                                "stop"
                            } else {
                                "start"
                            };
                            if let Err(e) = engine::control(&app, action) {
                                app.state::<engine::Engine>()
                                    .recorder
                                    .change(|s| s.error = e);
                            }
                            // Stop keeps the current processing view; only Start opens the pill.
                            // Reopening after Stop can race with a fast completed transcription.
                            if action == "start" && phase != "processing" {
                                let _ = show_recorder(app);
                            }
                        });
                    }
                })
                .build(),
        )
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            #[cfg(target_os = "macos")]
            startup::initialize(app.handle()).map_err(std::io::Error::other)?;
            // Set the live window and tray icons explicitly as well as the EXE resource.
            // Windows can otherwise keep the previous icon when the app is upgraded in place.
            if let Some(window) = app.get_webview_window("main") {
                window.set_icon(tauri::image::Image::from_bytes(include_bytes!("../icons/cristal/64x64.png"))?)?;
            }
            paste_focus::start();
            let dir = app.path().app_data_dir()?;
            lifecycle::init(app.path().app_log_dir()?);
            // A redirected data drive may mount after Windows starts tray apps.
            // Do not create a replacement profile while its junction is unavailable.
            // The settings database lives in the profile folder, so wait for that too.
            if std::env::args().any(|arg| arg == "--autostart") {
                let profile = profile::database_path().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
                let pending = |path: &std::path::Path| std::fs::symlink_metadata(path).is_ok() && !path.is_dir();
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
                while (pending(&dir) || profile.as_deref().is_some_and(pending))
                    && std::time::Instant::now() < deadline
                {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            }
            std::fs::create_dir_all(&dir)?;
            let store = profile::open(&dir.join("whispera.sqlite")).map_err(std::io::Error::other)?;
            store
                .event("Whispera 2 iniciada")
                .map_err(std::io::Error::other)?;
            if let Ok(settings) = store.get::<Settings>("settings") { local_asr::preload(&settings); }
            tauri::async_runtime::spawn(local_asr::load_installed_speech_detector());
            app.manage(store);
            if let Ok(binary)=screen::ffmpeg(app.handle()) {screen::warm_encoder(&binary);lossless_upload::configure(binary);}
            let startup_app=app.handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error)=startup::reconcile() {
                    let _=startup_app.state::<Store>().event(&format!("Inicio automático: {error}"));
                }
            });
            app.manage(library::Library::default());
            library::start(app.handle());
            video_transcript::start(app.handle());
            if let Err(error) = shortcuts::register(app.handle(), &library::hotkey(app.handle())) {
                let _ = app.state::<Store>().event(&format!("Biblioteca: {error}"));
            }
            if let Err(error) = unification::import_screen_settings(&app.state::<Store>()) {
                let _ = app.state::<Store>().event(&format!("Importacion de capturas pendiente: {error}"));
            }
            app.manage(shortcuts::Capture::default());
            app.manage(screen::Screen::new());
            app.manage(capture_session::ImageCapture::default());
            app.manage(screen_editor::Editor::default());
            app.manage(engine::Engine::new(dir.clone()).map_err(std::io::Error::other)?);
            app.manage(replay::Replay::default());
            app.manage(video_trim::Editors::default());
            app.manage(updates::Updates::default());
            // Fork preview: upstream updates could replace the local reader build.
            // updates::start is intentionally disabled until this fork signs its releases.
            health::start(app.handle());
            retention::start(app.handle());
            let settings: Settings = app
                .state::<Store>()
                .get("settings")
                .map_err(std::io::Error::other)?;
            if let Err(e) = shortcuts::register(app.handle(), &settings.hotkey) {
                let _ = app.state::<Store>().event(&e);
            }
            let video: screen::Preferences = app
                .state::<Store>()
                .get("screen_preferences")
                .unwrap_or_default();
            if let Err(e) = shortcuts::register(app.handle(), &video.hotkey) {
                let _ = app.state::<Store>().event(&e);
            }
            if let Err(e) = shortcuts::register(app.handle(), &video.image_hotkey) {
                let _ = app.state::<Store>().event(&e);
            }
            replay::initialize(app.handle());
            if let Err(error) = reader::initialize(app.handle()) { let _ = app.state::<Store>().event(&error); }
            windows_capture_keys::start(app.handle());
            // Creating the dictation window takes about a second (a new WebView2);
            // do it hidden after startup instead of on the first shortcut press.
            let recorder_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let _ = tauri::async_runtime::spawn_blocking(move || prepare_recorder(recorder_app, false)).await;
            });
            let capture_app = app.handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = screen::warm_selectors(&capture_app) {
                    let _ = capture_app.state::<Store>().event(&format!("Preparacion de capturas: {error}"));
                }
            });
            app.manage(tray::TrayMenu::default());
            tray::prewarm(app.handle());
            tauri::tray::TrayIconBuilder::new()
                .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/cristal/32x32.png"))?)
                .tooltip("Whispera")
                // Whispera draws its own menu next to the cursor on either click.
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { button_state: tauri::tray::MouseButtonState::Up, position, .. } = event {
                        if let Err(error) = tray::show(tray.app_handle(), position) {
                            let _ = tray.app_handle().state::<Store>().event(&format!("Menú de la bandeja: {error}"));
                        }
                    }
                })
                .build(app)?;
            reader::start_tray_monitor(app.handle());
            if std::env::args().any(|arg| arg == "--settings")
                || !app
                    .state::<Store>()
                    .get::<bool>("setup_complete")
                    .unwrap_or(false)
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.show()?;
                    window.set_focus()?;
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ai_usage::ai_usage_snapshot, ai_usage::ai_usage_connect_claude, ai_usage::ai_usage_open_provider,
            reader::reader_status, reader::reader_speak, reader::reader_control, reader::reader_preferences, reader::reader_configure, reader::reader_analyze,
            reader_export::reader_export_mp3, reader_export::reader_export_cancel,
            permissions::platform_permissions, permissions::platform_request_permission,
            replay::replay_preferences, replay::replay_status, replay::replay_create_folder, replay::replay_save_preferences, replay::replay_save,
            video_trim::video_trim_open, video_trim::video_trim_context, video_trim::video_trim_close, video_trim::video_trim_save,
            tray::tray_menu_state, tray::tray_menu_fit, tray::tray_menu_action,
            updates::updater_status, updates::updater_check, updates::updater_install, updates::updater_open_upstream,
            library::library_state,
            library::library_collect,
            library::library_action,
            library::library_toggle,
            library::library_drag,
            library::library_prepare_drag,
            library::library_pick,
            library::library_window,
            library::library_cursor,
            library_media::library_media_info,
            library_media::library_media_open,
            library_media::library_media_window,
            library_media::library_media_export,
            library_media::library_media_save,
            shortcuts::shortcut_capture,
            screen::screen_preferences,
            screen::screen_escape,
            screen::screen_appearance,
            screen::screen_save_preferences,
            screen::screen_status,
            screen::screen_select,
            screen::screen_monitors,
            screen::screen_start_monitor,
            screen::screen_cancel_selection,
            screen::screen_start,
            screen::screen_select_image,
            screen::screen_selection_kind,
            screen::screen_selection_image,
            screen::screen_editor_background,
            screen::screen_frozen_state,
            screen::screen_frozen_ready,
            screen::screen_overlay_ready,
            screen::screen_frame_drag,
            screen::screen_frame_preview,
            screen::screen_resize_region,
            screen_editor::screen_editor_context,
            screen_editor::screen_editor_image,
            screen_editor::screen_editor_sample,
            screen_editor::screen_editor_ready,
            screen_editor::screen_editor_action,
            screen_editor::screen_editor_feedback,
            screen_editor::screen_editor_feedback_get,
            screen_editor::screen_editor_print,
            screen_editor::screen_image_export,
            screen_editor::screen_image_export_binary,
            screen_editor::screen_video_snapshot,
            screen::screen_stop,
            screen::screen_copy,
            screen::screen_reveal,
            screen::screen_pending,
            screen::screen_pending_reveal,
            onboarding::setup_info,
            open_groq_console,
            shortcuts::save_all_shortcuts,
            library::library_preferences,
            video_transcript::video_transcript_action,
            screen::screen_audio_devices,
            screen::screen_pause,
            screen::screen_cancel,
            screen_editor::screen_overlay_layout,
            screen_editor::screen_overlay_customize,
            screen_editor::screen_tools_panel,
            onboarding::complete_setup,
            onboarding::set_startup,
            onboarding::startup_enabled,
            onboarding::validate_key,
            windows_capture_keys::windows_capture_shortcuts,
            windows_capture_keys::save_windows_capture_shortcuts,
            snapshot,
            audio_input_devices,
            local_models,
            local_model_install,
            local_model_cancel,
            local_model_remove,
            save_settings,
            read_settings,
            read_history,
            save_transcript,
            capture_history::screen_recent,
            capture_history::screen_recent_copy,
            capture_history::screen_recent_reveal,
            save_rules,
            read_rules,
            save_api_key,
            transcribe_file,
            import_legacy,
            import_groq_key,
            open_recorder,
            open_settings,
            open_updates,
            settings_window_action,
            open_recording_details,
            open_import,
            copy_text,
            toggle_recorder,
            floating_window::recorder_region,
            floating_window::recorder_size,
            health::ui_heartbeat,
            health::restart_app,
            engine::recording_state,
            engine::recording_action,
            engine::pending_recordings,
            engine::retry_recording,
            engine::copy_recording,
            engine::reveal_recording
        ])
        .on_window_event(|window, event| {
            if window.label() == "tray-menu" {
                if let tauri::WindowEvent::Focused(false) = event { let _ = tray::hide(window.app_handle()); }
                return;
            }
            if window.label().starts_with("video-trim-") {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    if window.app_handle().state::<video_trim::Editors>().busy.load(std::sync::atomic::Ordering::SeqCst) { api.prevent_close(); }
                    else { video_trim::destroyed(window.app_handle(),window.label()); }
                }
                if matches!(event, tauri::WindowEvent::Destroyed) { video_trim::destroyed(window.app_handle(),window.label()); }
                return;
            }
            if window.label() == "main"
                && matches!(
                    event,
                    tauri::WindowEvent::Focused(false)
                        | tauri::WindowEvent::CloseRequested { .. }
                        | tauri::WindowEvent::Destroyed
                )
            {
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn_blocking(move || {
                    if let Err(error) = shortcuts::capture(&app, false) {
                        let _ = app.state::<Store>().event(&error);
                    }
                });
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" { reader::stop(window.app_handle()); }
                if !lifecycle::keep_on_close(window.label()) { return; }
                api.prevent_close();
                let _ = window.hide();
                if capture_session::selector(window.label()) {
                    let app = window.app_handle().clone();
                    let scope=window.label().to_string();
                    tauri::async_runtime::spawn_blocking(move || {
                        let _ = screen::cancel_selection_for(app,&scope);
                    });
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("No se pudo iniciar Whispera Preview")
        .run(|app, event| match event {
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Ready if std::env::var("GITHUB_ACTIONS").as_deref()==Ok("true")
                && std::env::var("WHISPERA_CI_SMOKE").as_deref()==Ok("1")
                && std::env::args().any(|arg|arg=="--ci-smoke") => {
                let app=app.clone();
                std::thread::spawn(move||{
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    println!("WHISPERA_CI_READY");
                    app.exit(0);
                });
            }
            tauri::RunEvent::ExitRequested { code, api, .. } => {
                if lifecycle::prevent_implicit_exit(code) {
                    api.prevent_exit();
                    lifecycle::record("Implicit exit prevented; tray app remains active");
                } else { lifecycle::record(&format!("Explicit exit requested: {code:?}")); }
            }
            tauri::RunEvent::Exit => lifecycle::clean_exit(),
            _ => {}
        });
}

#[cfg(all(test,windows))]
mod local_import_tests {
    use super::*;
    #[test]
    fn imported_mp3_is_decoded_locally_and_original_is_preserved() {
        let root=Scratch(std::env::temp_dir().join(format!("whispera-local-import-test-{}",uuid::Uuid::new_v4())));
        std::fs::create_dir(&root.0).unwrap();
        let wav=root.0.join("original.wav");
        let mut writer=hound::WavWriter::create(&wav,hound::WavSpec{channels:1,sample_rate:16000,bits_per_sample:16,sample_format:hound::SampleFormat::Int}).unwrap();
        for _ in 0..1600 {writer.write_sample(0i16).unwrap();}writer.finalize().unwrap();
        let binary=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../node_modules/ffmpeg-static/ffmpeg.exe");
        let source=root.0.join("original.mp3");
        assert!(screen::command(&binary).args(["-hide_banner","-loglevel","error","-y","-i"]).arg(&wav).arg(&source).status().unwrap().success());
        let original=std::fs::read(&source).unwrap();
        let converted=local_import(Some(&binary),&source).unwrap().unwrap();
        let reader=hound::WavReader::open(converted.0.join("audio.wav")).unwrap();
        assert_eq!(reader.spec().sample_rate,16000);assert_eq!(reader.spec().channels,1);assert!(reader.duration()>0);
        assert_eq!(std::fs::read(&source).unwrap(),original);
        assert!(local_import(None,&wav).unwrap().is_none());
        assert!(local_import(None,&source).is_err());
    }
}
