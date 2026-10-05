#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod audio;
mod capture_history;
mod library;
mod library_media;
mod video_transcript;
mod screen_capture;
mod engine;
mod floating_window;
mod groq;
mod health;
mod incremental;
mod retention;
mod unification;
mod legacy;
mod onboarding;
mod paste;
mod paste_focus;
mod profile;
mod windows_capture_keys;
mod screen;
mod screen_editor;
mod shortcuts;
mod sounds;
mod storage;
mod system_audio;
use storage::{Rule, Settings, Snapshot, Store};
use tauri::{Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[tauri::command]
fn open_groq_console() -> Result<(), String> {
    use windows::{
        core::w,
        Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    };
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("https://console.groq.com/keys"),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        return Err("No se pudo abrir el navegador".into());
    }
    Ok(())
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
    if [
        shortcuts::parse(&video.hotkey)?,
        shortcuts::parse(&video.image_hotkey)?,
    ]
    .contains(&shortcuts::parse(&settings.hotkey)?)
    {
        return Err("El atajo de dictado debe ser distinto de los de video y captura".into());
    }
    shortcuts::update(&app, &old.hotkey, &settings.hotkey)?;
    store.put("settings", &settings)
}
#[tauri::command]
fn save_rules(rules: Vec<Rule>, store: State<Store>) -> Result<(), String> {
    groq::validate_rules(&rules)?;
    store.put("rules", &rules)
}
#[tauri::command]
fn save_api_key(key: String) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() || key.len() > 1024 || key.chars().any(char::is_whitespace) {
        return Err("Clave no valida".into());
    }
    groq::entry()?
        .set_password(key)
        .map_err(|_| "Windows no pudo guardar la credencial".into())
}
#[tauri::command]
async fn transcribe_file(
    path: String,
    app: tauri::AppHandle,
    store: State<'_, Store>,
) -> Result<String, String> {
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
    match groq::transcribe(std::path::Path::new(&path), &settings, &rules).await {
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
fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    let w = app
        .get_webview_window("main")
        .ok_or("Configuracion no disponible")?;
    if let Some(library) = app.get_webview_window("library") { let _ = library.hide(); }
    w.unminimize().map_err(|e| e.to_string())?;
    w.show().map_err(|e| e.to_string())?;
    let window = w.clone();
    w.run_on_main_thread(move || {
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
        .inner_size(650., 680.)
        .min_inner_size(450., 550.)
        .build()
        .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().app_name("Whispera").args(["--autostart"]).build())
        // Launching again must not turn the background dictation tool into a visible window.
        .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
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
                        if shortcuts::parse(&library::hotkey(&app)).map(|key| key == *shortcut).unwrap_or(false) {
                            tauri::async_runtime::spawn(async move { let _ = library::library_toggle(app).await; });
                            return;
                        }
                        if shortcuts::parse("Escape")
                            .map(|key| key == *shortcut)
                            .unwrap_or(false)
                            && app.state::<screen::Screen>().busy()
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
                                    screen::report_error(&app, error);
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
            paste_focus::start();
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let store = profile::open(&dir.join("whispera.sqlite")).map_err(std::io::Error::other)?;
            store
                .event("Whispera 2 iniciada")
                .map_err(std::io::Error::other)?;
            app.manage(store);
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
            app.manage(screen_editor::Editor::default());
            app.manage(engine::Engine::new(dir.clone()).map_err(std::io::Error::other)?);
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
            windows_capture_keys::start(app.handle());
            use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
            let show = MenuItem::with_id(app, "settings", "Configuracion", true, None::<&str>)?;
            let record = MenuItem::with_id(app, "recorder", "Abrir grabadora", true, None::<&str>)?;
            let import =
                MenuItem::with_id(app, "import", "Transcribir archivo...", true, None::<&str>)?;
            let video = MenuItem::with_id(app, "screen", "Iniciar video", true, None::<&str>)?;
            let image =
                MenuItem::with_id(app, "screenshot", "Capturar imagen", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Salir / Quit Whispera", true, None::<&str>)?;
            let library_menu = MenuItem::with_id(app, "library", "Biblioteca / Portapapeles", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu =
                Menu::with_items(app, &[&record, &video, &image, &import, &library_menu, &show, &sep, &quit])?;
            tauri::tray::TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Whispera")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "library" => { let app=app.clone(); tauri::async_runtime::spawn(async move { let _ = library::library_toggle(app).await; }); }
                    "settings" => {
                        if let Err(error) = open_settings(app.clone()) { let _ = app.state::<Store>().event(&error); }
                    }
                    "recorder" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            let _ = show_recorder(app);
                        });
                    }
                    "import" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(error) = open_import(app.clone()).await {
                                let _ = app.state::<Store>().event(&error);
                            }
                        });
                    }
                    "quit" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            let _ = screen::stop(&app);
                            engine::shutdown(&app);
                            app.exit(0);
                        });
                    }
                    "screen" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            if let Err(error) = screen::select(&app) {
                                screen::report_error(&app, error);
                            }
                        });
                    }
                    "screenshot" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            if let Err(error) = screen::select_image(&app) {
                                screen::report_error(&app, error);
                            }
                        });
                    }
                    _ => {}
                })
                .build(app)?;
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
            library::library_state,
            library::library_collect,
            library::library_action,
            library::library_toggle,
            library::library_drag,
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
            screen::screen_cancel_selection,
            screen::screen_start,
            screen::screen_select_image,
            screen::screen_selection_kind,
            screen::screen_selection_image,
            screen::screen_overlay_ready,
            screen_editor::screen_editor_context,
            screen_editor::screen_editor_image,
            screen_editor::screen_editor_sample,
            screen_editor::screen_editor_ready,
            screen_editor::screen_editor_action,
            screen_editor::screen_editor_feedback,
            screen_editor::screen_editor_feedback_get,
            screen_editor::screen_editor_print,
            screen_editor::screen_image_export,
            screen_editor::screen_video_snapshot,
            screen::screen_stop,
            screen::screen_copy,
            screen::screen_reveal,
            onboarding::setup_info,
            open_groq_console,
            shortcuts::save_all_shortcuts,
            library::library_preferences,
            video_transcript::video_transcript_action,
            screen::screen_audio_devices,
            screen::screen_pause,
            screen::screen_cancel,
            screen_editor::screen_overlay_layout,
            screen_editor::screen_tools_panel,
            onboarding::complete_setup,
            onboarding::set_startup,
            onboarding::validate_key,
            windows_capture_keys::windows_capture_shortcuts,
            windows_capture_keys::save_windows_capture_shortcuts,
            snapshot,
            save_settings,
            read_settings,
            read_history,
            save_transcript,
            capture_history::screen_recent,
            capture_history::screen_recent_copy,
            capture_history::screen_recent_reveal,
            save_rules,
            save_api_key,
            transcribe_file,
            import_legacy,
            import_groq_key,
            open_recorder,
            open_settings,
            open_recording_details,
            open_import,
            copy_text,
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
                api.prevent_close();
                let _ = window.hide();
                if window.label().starts_with("screen-select-") {
                    let app = window.app_handle().clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        let _ = screen::cancel_selection(app);
                    });
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("No se pudo iniciar Whispera Preview");
}
