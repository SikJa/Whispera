//! Local region capture. FFmpeg is a separate bundled executable; no cloud upload.
use crate::storage::{Settings, Store};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, State};

#[derive(Clone, Serialize, Deserialize)]
pub struct Preferences {
    pub audio: String,
    pub hotkey: String,
    #[serde(default = "default_image_hotkey")]
    pub image_hotkey: String,
    #[serde(default = "default_frame_color")]
    pub frame_color: String,
    #[serde(default)]
    pub image_auto_copy: bool,
}
fn default_frame_color() -> String {
    "#ffffff".into()
}
fn default_image_hotkey() -> String {
    "Control+Shift+F10".into()
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            audio: "none".into(),
            hotkey: "Control+Shift+F9".into(),
            image_hotkey: default_image_hotkey(),
            frame_color: default_frame_color(),
            image_auto_copy: false,
        }
    }
}
#[derive(Clone, Default, Serialize)]
pub struct Status {
    pub phase: String,
    pub seconds: f64,
    pub path: String,
    pub error: String,
    pub copied: bool,
}
pub struct Screen {
    pub state: Arc<Mutex<Status>>,
    active: Mutex<Option<Active>>,
    pub gate: Mutex<()>,
    pub kind: Mutex<String>,
    selection_ready: AtomicBool,
    selection_started: Mutex<Option<Instant>>,
    snapshots: Mutex<std::collections::HashMap<String, crate::screen_capture::FrozenFrame>>,
}
struct Active {
    stop: mpsc::Sender<CaptureControl>,
    thread: std::thread::JoinHandle<()>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum CaptureControl {
    Stop,
    Pause,
    Resume,
    Cancel,
    Reframe(Region),
}
impl Screen {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(Status {
                phase: "idle".into(),
                ..Default::default()
            })),
            active: Mutex::new(None),
            gate: Mutex::new(()),
            kind: Mutex::new("video".into()),
            selection_ready: AtomicBool::new(false),
            selection_started: Mutex::new(None),
            snapshots: Mutex::new(std::collections::HashMap::new()),
        }
    }
    pub fn busy(&self) -> bool {
        self.state
            .lock()
            .map(|s| {
                [
                    "selecting",
                    "starting",
                    "editing",
                    "recording",
                    "paused",
                    "pausing",
                    "resuming",
                    "reframing",
                    "cancelling",
                    "saving",
                ]
                .contains(&s.phase.as_str())
            })
            .unwrap_or(true)
    }
}
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Region {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
fn region(
    rect: Rect,
    scale: f64,
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
) -> Result<Region, String> {
    if ![rect.x, rect.y, rect.width, rect.height, scale]
        .iter()
        .all(|v| v.is_finite())
        || scale <= 0.
        || rect.x < 0.
        || rect.y < 0.
        || rect.width < 16.
        || rect.height < 16.
    {
        return Err("Selecciona un area de al menos 16 x 16 pixeles".into());
    }
    let x = (rect.x * scale).round() as u32;
    let y = (rect.y * scale).round() as u32;
    let width = (rect.width * scale).round() as u32;
    let height = (rect.height * scale).round() as u32;
    if x.saturating_add(width) > size.width
        || y.saturating_add(height) > size.height
        || width > 8192
        || height > 8192
    {
        return Err("El area debe estar dentro de una pantalla".into());
    }
    Ok(Region {
        x: position.x + x as i32,
        y: position.y + y as i32,
        width,
        height,
    })
}
pub(crate) fn ffmpeg(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let path = if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../node_modules/ffmpeg-static/ffmpeg.exe")
    } else {
        app.path()
            .resource_dir()
            .map_err(|e| e.to_string())?
            .join("bin/ffmpeg.exe")
    };
    if !path.is_file() {
        return Err("Falta el motor de video. Ejecuta npm ci antes de compilar.".into());
    }
    Ok(path)
}
pub(crate) fn command(binary: &Path) -> Command {
    let mut cmd = Command::new(binary);
    cmd.creation_flags(0x08000000).stdout(Stdio::null());
    cmd
}
fn hide_selectors(app: &tauri::AppHandle) {
    if let Ok(mut snapshots) = app.state::<Screen>().snapshots.lock() { snapshots.clear(); }
    release_escape(app);
    crate::screen_editor::hide(app);
    for (label, window) in app.webview_windows() {
        if label.starts_with("screen-select-") {
            let _ = window.hide();
            let _ = window.emit("screen-hide", ());
        }
    }
}
fn release_escape(app: &tauri::AppHandle) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let Ok(key) = crate::shortcuts::parse("Escape") else {
        return;
    };
    let store = app.state::<Store>();
    let voice: Settings = store.get("settings").unwrap_or_default();
    let screen: Preferences = store.get("screen_preferences").unwrap_or_default();
    // Preserve a legacy user shortcut if Escape was explicitly configured before.
    if [voice.hotkey, screen.hotkey, screen.image_hotkey]
        .iter()
        .any(|v| crate::shortcuts::parse(v).ok() == Some(key))
    {
        return;
    }
    if app.global_shortcut().is_registered(key) {
        let _ = app.global_shortcut().unregister(key);
    }
}
pub fn escape(app: &tauri::AppHandle) -> Result<(), String> {
    let screen = app.state::<Screen>();
    let _gate = screen.gate.lock().map_err(|_| "Video ocupado")?;
    stop_locked(app, &screen)?;
    let mut state = screen.state.lock().map_err(|_| "Estado ocupado")?;
    if ["selecting", "editing"].contains(&state.phase.as_str()) {
        state.phase = "idle".into();
    }
    Ok(())
}
#[tauri::command]
pub async fn screen_escape(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || escape(&app))
        .await
        .map_err(|e| e.to_string())?
}
pub(crate) fn protect(window: &tauri::WebviewWindow) -> Result<(), String> {
    window
        .set_content_protected(true)
        .map_err(|e| e.to_string())?;
    // Tao ignores this Win32 call's result. Verify exclusion before displaying
    // the overlay so the border and animation never become part of the video.
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            GetWindowDisplayAffinity, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
        },
    };
    let hwnd = HWND(window.hwnd().map_err(|e| e.to_string())?.0);
    let mut affinity = 0;
    unsafe {
        SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)
            .map_err(|e| format!("No se pudo proteger el indicador de captura: {e}"))?;
        GetWindowDisplayAffinity(hwnd, &mut affinity).map_err(|e| e.to_string())?;
    }
    if affinity != WDA_EXCLUDEFROMCAPTURE.0 {
        return Err("Windows no pudo excluir el indicador del video".into());
    }
    Ok(())
}
fn show_indicator(window: &tauri::WebviewWindow, rect: Rect, kind: &str) -> Result<(), String> {
    window
        .set_ignore_cursor_events(true)
        .map_err(|e| e.to_string())?;
    window.set_focusable(false).map_err(|e| e.to_string())?;
    // Keep the existing transparent document and window on screen. Navigating
    // and hide/show here caused a full-screen flash between selection and capture.
    window
        .emit("screen-stage", serde_json::json!({"rect":rect,"kind":kind}))
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn report_error(app: &tauri::AppHandle, error: String) {
    if let Ok(mut state) = app.state::<Screen>().state.lock() {
        state.error = error.clone();
    }
    let _ = app.state::<Store>().event(&error);
}
#[tauri::command]
pub fn screen_preferences(store: State<Store>) -> Preferences {
    store.get("screen_preferences").unwrap_or_default()
}
#[tauri::command]
pub fn screen_audio_devices() -> serde_json::Value {
    let host = cpal::default_host();
    serde_json::json!({
        "microphone": host.default_input_device().and_then(|device| device.name().ok()),
        "system": host.default_output_device().and_then(|device| device.name().ok()),
    })
}
#[tauri::command]
pub fn screen_appearance(store: State<Store>) -> Result<serde_json::Value, String> {
    let settings: Settings = store.get("settings")?;
    let preferences: Preferences = store.get("screen_preferences").unwrap_or_default();
    Ok(
        serde_json::json!({"color":settings.color,"pattern":settings.pattern,"recorderScale":settings.recorder_scale,"frameColor":preferences.frame_color}),
    )
}
#[tauri::command]
pub fn screen_status(screen: State<Screen>) -> Status {
    screen
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}
#[tauri::command]
pub async fn screen_save_preferences(
    preferences: Preferences,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || save_preferences(preferences, app))
        .await
        .map_err(|e| e.to_string())?
}
fn save_preferences(preferences: Preferences, app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let screen = app.state::<Screen>();
    let _gate = screen.gate.lock().map_err(|_| "Video ocupado")?;
    if screen.busy() {
        return Err("Detene la grabacion antes de cambiar preferencias".into());
    }
    if !["none", "system", "microphone", "both"].contains(&preferences.audio.as_str())
        || preferences.hotkey.is_empty()
        || preferences.hotkey.len() > 80
        || preferences.image_hotkey.is_empty()
        || preferences.image_hotkey.len() > 80
        || !valid_frame_color(&preferences.frame_color)
    {
        return Err("Preferencias de video invalidas".into());
    }
    let key = crate::shortcuts::parse(&preferences.hotkey)?;
    let image_key = crate::shortcuts::parse(&preferences.image_hotkey)?;
    let store = app.state::<Store>();
    let dictation: Settings = store.get("settings")?;
    let voice_key = crate::shortcuts::parse(&dictation.hotkey)?;
    if key == voice_key || image_key == voice_key || key == image_key {
        return Err("Dictado, video y captura necesitan atajos distintos".into());
    }
    let old: Preferences = store.get("screen_preferences").unwrap_or_default();
    let mut added = Vec::new();
    let result = (|| {
        for (value, parsed) in [
            (&preferences.hotkey, key),
            (&preferences.image_hotkey, image_key),
        ] {
            if !app.global_shortcut().is_registered(parsed) {
                crate::shortcuts::register(&app, value)?;
                added.push(parsed);
            }
        }
        store.put("screen_preferences", &preferences)
    })();
    if let Err(error) = result {
        for added_key in added {
            let _ = app.global_shortcut().unregister(added_key);
        }
        return Err(error);
    }
    for old_value in [&old.hotkey, &old.image_hotkey] {
        if let Ok(old_key) = crate::shortcuts::parse(old_value) {
            if old_key != key && old_key != image_key && old_key != voice_key {
                let _ = app.global_shortcut().unregister(old_key);
            }
        }
    }
    Ok(())
}
fn valid_frame_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].bytes().all(|v| v.is_ascii_hexdigit())
}
pub fn select(app: &tauri::AppHandle) -> Result<(), String> {
    let screen = app.state::<Screen>();
    let _gate = screen.gate.lock().map_err(|_| "Video ocupado")?;
    select_locked(app, &screen, "video")
}
pub fn select_image(app: &tauri::AppHandle) -> Result<(), String> {
    let screen = app.state::<Screen>();
    let _gate = screen.gate.lock().map_err(|_| "Captura ocupada")?;
    if screen.state.lock().map_err(|_| "Estado ocupado")?.phase == "editing" {
        hide_selectors(app);
        screen.state.lock().map_err(|_| "Estado ocupado")?.phase = "idle".into();
    }
    select_locked(app, &screen, "image")
}
fn select_locked(app: &tauri::AppHandle, screen: &Screen, kind: &str) -> Result<(), String> {
    if screen.busy() {
        return Err("Ya hay una grabacion o seleccion de pantalla".into());
    }
    *screen.selection_started.lock().map_err(|_| "Captura ocupada")? = Some(Instant::now());
    if kind != "image" { ffmpeg(app)?; }
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    if monitors.is_empty() {
        return Err("No hay pantallas disponibles".into());
    }
    // Freeze before activation can dismiss a menu in the target app.
    let mut snapshots = std::collections::HashMap::new();
    if kind == "image" {
        let _exclude = app.get_webview_window("screen-ink")
            .map(|w| w.hwnd().map_err(|e| e.to_string()).and_then(|h| crate::screen_capture::InkExclusion::new(windows::Win32::Foundation::HWND(h.0))))
            .transpose()?;
        for (index, monitor) in monitors.iter().enumerate() {
            let region = Region { x: monitor.position().x, y: monitor.position().y, width: monitor.size().width, height: monitor.size().height };
            snapshots.insert(format!("screen-select-{index}"), crate::screen_capture::freeze(region)?);
        }
    }
    hide_selectors(app);
    *screen.snapshots.lock().map_err(|_| "Captura ocupada")? = snapshots;
    crate::shortcuts::register(app, "Escape")?;
    screen.selection_ready.store(false, Ordering::SeqCst);
    {
        let mut status = screen.state.lock().map_err(|_| "Estado ocupado")?;
        status.phase = "selecting".into();
        status.error.clear();
    }
    *screen.kind.lock().map_err(|_| "Estado ocupado")? = kind.into();
    let result = (|| {
        for (index, monitor) in monitors.iter().enumerate() {
            let label = format!("screen-select-{index}");
            let window = selector_window(app, &label)?;
            set_frame_region(&window, None)?;
            window.set_focusable(true).map_err(|e| e.to_string())?;
            window
                .set_ignore_cursor_events(false)
                .map_err(|e| e.to_string())?;
            protect(&window)?;
            window
                .set_position(*monitor.position())
                .map_err(|e| e.to_string())?;
            window
                .set_size(*monitor.size())
                .map_err(|e| e.to_string())?;
            window
                .emit("screen-reset", kind)
                .map_err(|e| e.to_string())?;
        }
        screen.selection_ready.store(true, Ordering::SeqCst);
        Ok(())
    })();
    if result.is_err() {
        hide_selectors(app);
        screen.state.lock().map_err(|_| "Estado ocupado")?.phase = "idle".into();
    }
    result
}
fn selector_window(app: &tauri::AppHandle, label: &str) -> Result<tauri::WebviewWindow, String> {
    Ok(match app.get_webview_window(label) {
                Some(w) => w,
                None => tauri::WebviewWindowBuilder::new(
                    app,
                    label,
                    tauri::WebviewUrl::App("overlay.html?view=screen-select".into()),
                )
                .title("Seleccionar area · Whispera")
                .transparent(true)
                .background_color(tauri::window::Color(0, 0, 0, 0))
                .content_protected(true)
                .decorations(false)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .visible(false)
                .build()
                .map_err(|e| e.to_string())?,
    })
}
/// Prepare hidden selectors once, outside the first shortcut's critical path.
pub fn warm_selectors(app: &tauri::AppHandle) -> Result<(), String> {
    let screen = app.state::<Screen>();
    let Ok(_gate) = screen.gate.try_lock() else { return Ok(()); };
    if screen.busy() { return Ok(()); }
    for (index, _) in app.available_monitors().map_err(|e| e.to_string())?.iter().enumerate() {
        selector_window(app, &format!("screen-select-{index}"))?;
    }
    Ok(())
}
#[tauri::command]
pub async fn screen_select(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || select(&app))
        .await
        .map_err(|e| e.to_string())?
}
pub(crate) fn set_frame_region(window: &tauri::WebviewWindow, rect: Option<Rect>) -> Result<(), String> {
    let clone=window.clone();
    window.run_on_main_thread(move || unsafe {
        use windows::Win32::Graphics::Gdi::{CreateRectRgn,CreateRoundRectRgn,CombineRgn,SetWindowRgn,DeleteObject,RGN_DIFF};
        let Ok(hwnd)=clone.hwnd() else {return;};
        let hwnd=windows::Win32::Foundation::HWND(hwnd.0);
        let Some(rect)=rect else {SetWindowRgn(hwnd,None,true);return;};
        let dpi=clone.scale_factor().unwrap_or(1.);
        let edge=8.;
        let outer=CreateRoundRectRgn(((rect.x-edge)*dpi).floor() as i32,((rect.y-edge)*dpi).floor() as i32,
            ((rect.x+rect.width+edge)*dpi).ceil() as i32,((rect.y+rect.height+edge)*dpi).ceil() as i32,(28.*dpi) as i32,(28.*dpi) as i32);
        let inner=CreateRectRgn(((rect.x+edge)*dpi).ceil() as i32,((rect.y+edge)*dpi).ceil() as i32,
            ((rect.x+rect.width-edge)*dpi).floor() as i32,((rect.y+rect.height-edge)*dpi).floor() as i32);
        if outer.0.is_null()||inner.0.is_null() {
            if !outer.0.is_null(){let _=DeleteObject(outer.into());}
            if !inner.0.is_null(){let _=DeleteObject(inner.into());}return;
        }
        let _=CombineRgn(Some(outer),Some(outer),Some(inner),RGN_DIFF);
        let _=DeleteObject(inner.into());
        if SetWindowRgn(hwnd,Some(outer),true)==0 {let _=DeleteObject(outer.into());}
    }).map_err(|e|e.to_string())
}
#[tauri::command]
pub fn screen_frame_drag(app: tauri::AppHandle, window: tauri::WebviewWindow, active: bool) -> Result<(), String> {
    if !window.label().starts_with("screen-select-") {return Err("Vista incorrecta".into());}
    let ctx=crate::screen_editor::screen_editor_context(app.state::<crate::screen_editor::Editor>()).ok_or("Captura no disponible")?;
    if ctx.source_label!=window.label(){return Err("Vista incorrecta".into());}
    set_frame_region(&window,if active {None}else{Some(ctx.rect)})?;
    crate::screen_editor::raise_controls(&app);
    Ok(())
}
#[tauri::command]
pub async fn screen_resize_region(app: tauri::AppHandle, window: tauri::WebviewWindow, id:String, rect:Rect) -> Result<(),String> {
    tauri::async_runtime::spawn_blocking(move|| {
        let screen=app.state::<Screen>();let _gate=screen.gate.lock().map_err(|_|"Captura ocupada")?;
        let ctx=crate::screen_editor::screen_editor_context(app.state::<crate::screen_editor::Editor>()).ok_or("Captura no disponible")?;
        if ctx.id!=id||ctx.source_label!=window.label(){return Err("La captura ya termino".into());}
        let next=region(rect,ctx.scale,window.inner_position().map_err(|e|e.to_string())?,window.inner_size().map_err(|e|e.to_string())?)?;
        let phase=screen.state.lock().map_err(|_|"Estado ocupado")?.phase.clone();
        let bytes=if ctx.kind=="image" {
            if phase!="editing"{return Err("La captura ya termino".into());}
            Some(screen.snapshots.lock().map_err(|_|"Captura ocupada")?.get(window.label()).ok_or("Imagen no disponible")?.crop_png(next)?)
        }else{
            if !["recording","paused"].contains(&phase.as_str()){return Err("Espera a que termine el ajuste del video".into());}
            let active=screen.active.lock().map_err(|_|"Video ocupado")?;
            active.as_ref().ok_or("Video no disponible")?.stop.send(CaptureControl::Reframe(next)).map_err(|e|e.to_string())?;
            if phase=="recording"{screen.state.lock().map_err(|_|"Estado ocupado")?.phase="reframing".into();}
            None
        };
        crate::screen_editor::resize(&app,&window,&id,rect,next,bytes)
    }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn screen_select_image(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || select_image(&app))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn screen_selection_kind(screen: State<Screen>) -> String {
    screen
        .kind
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}
#[tauri::command]
pub fn screen_selection_image(screen: State<Screen>, window: tauri::WebviewWindow) -> Result<tauri::ipc::Response, String> {
    let snapshots = screen.snapshots.lock().map_err(|_| "Captura ocupada")?;
    let bytes = snapshots.get(window.label()).map(|frame| frame.preview_bmp()).unwrap_or_default();
    Ok(tauri::ipc::Response::new(bytes))
}
#[tauri::command]
pub fn screen_overlay_ready(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<bool, String> {
    if !app.state::<Screen>().selection_ready.load(Ordering::SeqCst) {
        return Ok(false);
    }
    if window.label().starts_with("screen-select-")
        && app
            .state::<Screen>()
            .state
            .lock()
            .map_err(|_| "Estado ocupado")?
            .phase
            == "selecting"
    {
        window.show().map_err(|e| e.to_string())?;
        // Reassert z-order after showing a reused selector, including over our own settings.
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SWP_NOACTIVATE};
            SetWindowPos(windows::Win32::Foundation::HWND(window.hwnd().map_err(|e| e.to_string())?.0), Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)
                .map_err(|e| e.to_string())?;
        }
        if window.label() == "screen-select-0" {
            window.set_focus().map_err(|e| e.to_string())?;
            if let Some(started) = app.state::<Screen>().selection_started.lock().map_err(|_| "Captura ocupada")?.take() {
                let _ = app.state::<Store>().event(&format!("Selector de captura listo en {} ms", started.elapsed().as_millis()));
            }
        }
    }
    Ok(true)
}
#[tauri::command]
pub async fn screen_cancel_selection(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || cancel_selection(app))
        .await
        .map_err(|e| e.to_string())?
}
pub fn cancel_selection(app: tauri::AppHandle) -> Result<(), String> {
    let screen = app.state::<Screen>();
    let _gate = screen.gate.lock().map_err(|_| "Video ocupado")?;
    cancel_selection_locked(&app, &screen)
}
fn cancel_selection_locked(app: &tauri::AppHandle, screen: &Screen) -> Result<(), String> {
    if ["selecting", "editing"].contains(
        &screen
            .state
            .lock()
            .map_err(|_| "Estado ocupado")?
            .phase
            .as_str(),
    ) {
        hide_selectors(app);
        screen.state.lock().map_err(|_| "Estado ocupado")?.phase = "idle".into();
    }
    Ok(())
}
#[tauri::command]
pub async fn screen_start(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    rect: Rect,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !window.label().starts_with("screen-select-") {
            return Err("Seleccion no disponible".into());
        }
        let screen = app.state::<Screen>();
        let _gate = screen.gate.lock().map_err(|_| "Video ocupado")?;
        if screen.state.lock().map_err(|_| "Estado ocupado")?.phase != "selecting" {
            return Err("La seleccion ya termino".into());
        }
        let region = region(
            rect,
            window.scale_factor().map_err(|e| e.to_string())?,
            window.inner_position().map_err(|e| e.to_string())?,
            window.inner_size().map_err(|e| e.to_string())?,
        )?;
        let kind = screen.kind.lock().map_err(|_| "Estado ocupado")?.clone();
        for (label, other) in app.webview_windows() {
            if label.starts_with("screen-select-") && label != window.label() {
                let _ = other.hide();
            }
        }
        show_indicator(&window, rect, &kind)?;
        if kind == "image" {
            let bytes = {
                let mut snapshots = screen.snapshots.lock().map_err(|_| "Captura ocupada")?;
                let frame = snapshots.get(window.label()).ok_or("La imagen de seleccion ya no esta disponible")?;
                let cropped = frame.crop_png(region)?;
                // Keep this monitor's original pixels so its crop remains adjustable.
                snapshots.retain(|label, _| label == window.label());
                cropped
            };
            screen.state.lock().map_err(|_| "Estado ocupado")?.phase = "editing".into();
            let preferences: Preferences = app
                .state::<Store>()
                .get("screen_preferences")
                .unwrap_or_default();
            if preferences.image_auto_copy {
                match crate::screen_editor::copy_immediately(&app, &window, rect, region, bytes) {
                    Ok(true) => {
                        hide_selectors(&app);
                        *screen.state.lock().map_err(|_| "Estado ocupado")? = Status {
                            phase: "idle".into(),
                            copied: true,
                            ..Default::default()
                        };
                    }
                    Ok(false) => {} // Clipboard failure: keep the original image open for retry.
                    Err(error) => {
                        hide_selectors(&app);
                        screen.state.lock().map_err(|_| "Estado ocupado")?.phase = "error".into();
                        report_error(&app, error.clone());
                        return Err(error);
                    }
                }
                return Ok(());
            }
            if let Err(error) = crate::screen_editor::open_with_image(&app, &window, rect, region, "image", Some(bytes)) {
                hide_selectors(&app);
                screen.state.lock().map_err(|_| "Estado ocupado")?.phase = "error".into();
                report_error(&app, error.clone());
                return Err(error);
            }
            return Ok(());
        }
        let prefs: Preferences = app
            .state::<Store>()
            .get("screen_preferences")
            .unwrap_or_default();
        let binary = ffmpeg(&app)?;
        // Each session has a private directory. Files remain available for pasting/recovery.
        let dir = app
            .path()
            .app_cache_dir()
            .map_err(|e| e.to_string())?
            .join("screen-recordings")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        if let Some(previous) = screen.active.lock().map_err(|_| "Video ocupado")?.take() {
            let _ = previous.stop.send(CaptureControl::Stop);
            let _ = previous.thread.join();
        }
        *screen.state.lock().map_err(|_| "Estado ocupado")? = Status {
            phase: "starting".into(),
            ..Default::default()
        };
        let (stop, stop_rx) = mpsc::channel();
        let (ready, ready_rx) = mpsc::channel();
        let state = screen.state.clone();
        let history_app = app.clone();
        let transcribe_microphone = crate::video_transcript::recording_enabled(&app.state::<Store>());
        let thread = std::thread::spawn(move || {
            capture(region, prefs, binary, dir, state.clone(), stop_rx, ready, transcribe_microphone);
            let path = state
                .lock()
                .ok()
                .map(|s| s.path.clone())
                .unwrap_or_default();
            if !path.is_empty() {
                if transcribe_microphone {
                    if let Err(error) = crate::video_transcript::enqueue(&history_app.state::<Store>(), Path::new(&path)) {
                        let _ = history_app.state::<Store>().event(&format!("Transcripcion de video: {error}"));
                    }
                }
                if let Err(error) = crate::capture_history::remember(
                    &history_app.state::<Store>(),
                    "video",
                    Path::new(&path),
                ) {
                    let _ = history_app.state::<Store>().event(&error);
                }
            }
            if let Ok(status) = state.lock() {
                if status.phase == "error" {
                    let _ = history_app.state::<Store>().event(&status.error);
                    hide_selectors(&history_app);
                }
            }
        });
        match ready_rx.recv() {
            Ok(Ok(())) => {
                *screen.active.lock().map_err(|_| "Video ocupado")? = Some(Active { stop, thread });
                if let Err(error) = crate::screen_editor::open(&app, &window, rect, region, "video")
                {
                    if let Some(active) = screen.active.lock().map_err(|_| "Video ocupado")?.take()
                    {
                        let _ = active.stop.send(CaptureControl::Cancel);
                        let _ = active.thread.join();
                    }
                    hide_selectors(&app);
                    report_error(&app, error.clone());
                    return Err(error);
                }
                Ok(())
            }
            result => {
                let _ = thread.join();
                let error = result
                    .unwrap_or_else(|_| Err("El motor de video se cerro".into()))
                    .unwrap_err();
                report_error(&app, error.clone());
                Err(error)
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
pub fn stop(app: &tauri::AppHandle) -> Result<(), String> {
    let screen = app.state::<Screen>();
    let _gate = screen.gate.lock().map_err(|_| "Video ocupado")?;
    stop_locked(app, &screen)
}
fn stop_locked(app: &tauri::AppHandle, screen: &Screen) -> Result<(), String> {
    let active = screen.active.lock().map_err(|_| "Video ocupado")?.take();
    if let Some(active) = active {
        let _ = active.stop.send(CaptureControl::Stop);
        // The worker finalizes the MP4 without keeping the capture UI visible.
        hide_selectors(app);
        active
            .thread
            .join()
            .map_err(|_| "El motor de video se cerro inesperadamente")?;
    }
    hide_selectors(app);
    Ok(())
}
#[tauri::command]
pub async fn screen_stop(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || stop(&app))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn screen_pause(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let screen = app.state::<Screen>();
        let _gate = screen.gate.lock().map_err(|_| "Video ocupado")?;
        let active = screen.active.lock().map_err(|_| "Video ocupado")?;
        let active = active.as_ref().ok_or("No hay una grabacion activa")?;
        let mut state = screen.state.lock().map_err(|_| "Estado ocupado")?;
        let (action, phase) = match state.phase.as_str() {
            "recording" => (CaptureControl::Pause, "pausing"),
            "paused" => (CaptureControl::Resume, "resuming"),
            _ => return Ok(()),
        };
        active
            .stop
            .send(action)
            .map_err(|_| "El motor de video se cerro")?;
        state.phase = phase.into();
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn screen_cancel(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let screen = app.state::<Screen>();
        let _gate = screen.gate.lock().map_err(|_| "Video ocupado")?;
        if let Some(active) = screen.active.lock().map_err(|_| "Video ocupado")?.take() {
            let _ = active.stop.send(CaptureControl::Cancel);
            active
                .thread
                .join()
                .map_err(|_| "El motor de video se cerro")?;
        }
        hide_selectors(&app);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[derive(Debug, PartialEq)]
enum ToggleAction {
    Select,
    Cancel,
    Stop,
    Wait,
}
fn gated_action(screen: &Screen) -> Result<(MutexGuard<'_, ()>, ToggleAction), String> {
    let gate = screen.gate.lock().map_err(|_| "Video ocupado")?;
    // Starting capture holds the gate. A second shortcut must see the state
    // after startup, otherwise it attempts to cancel an already-finished selection.
    let action = match screen
        .state
        .lock()
        .map_err(|_| "Estado ocupado")?
        .phase
        .as_str()
    {
        "recording" | "paused" | "pausing" | "resuming" | "reframing" => ToggleAction::Stop,
        "selecting" => ToggleAction::Cancel,
        "editing" => ToggleAction::Cancel,
        "starting" => ToggleAction::Wait,
        "saving" | "cancelling" => ToggleAction::Wait,
        _ => ToggleAction::Select,
    };
    Ok((gate, action))
}
pub fn toggle(app: &tauri::AppHandle) -> Result<(), String> {
    let screen = app.state::<Screen>();
    let (_gate, action) = gated_action(&screen)?;
    match action {
        ToggleAction::Stop => stop_locked(app, &screen),
        ToggleAction::Cancel => cancel_selection_locked(app, &screen),
        ToggleAction::Wait => Ok(()),
        ToggleAction::Select => select_locked(app, &screen, "video"),
    }
}
pub(crate) fn copy_file(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err("El video temporal ya no existe".into());
    }
    let _clipboard = clipboard_win::Clipboard::new_attempts(20)
        .map_err(|e| format!("No se pudo abrir el portapapeles: {e}"))?;
    use clipboard_win::Setter;
    clipboard_win::formats::FileList
        .write_clipboard(&[path.to_string_lossy().as_ref()])
        .map_err(|e| format!("Video conservado, pero no se pudo copiar: {e}"))
}
#[tauri::command]
pub fn screen_copy(screen: State<Screen>, app: tauri::AppHandle) -> Result<(), String> {
    let mut state = screen.state.lock().map_err(|_| "Estado ocupado")?;
    if state.path.is_empty() {
        return Err("Todavia no hay un video".into());
    }
    crate::video_transcript::copy_files(&app, &[PathBuf::from(&state.path)])?;
    state.copied = true;
    Ok(())
}
#[tauri::command]
pub fn screen_reveal(screen: State<Screen>) -> Result<(), String> {
    let path = screen
        .state
        .lock()
        .map_err(|_| "Estado ocupado")?
        .path
        .clone();
    if path.is_empty() || !Path::new(&path).is_file() {
        return Err("El video temporal no esta disponible".into());
    }
    Command::new("explorer.exe")
        .arg(format!("/select,{path}"))
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

struct AudioTrack {
    _stream: cpal::Stream,
    rx: mpsc::Receiver<(Instant, Vec<i16>)>,
    file: File,
    path: PathBuf,
    rate: u32,
    channels: u16,
    written: u64,
    fault: Arc<Mutex<String>>,
    scratch: Vec<u8>,
}
fn samples<T: Copy>(
    data: &[T],
    convert: impl Fn(T) -> f32,
    tx: &mpsc::SyncSender<(Instant, Vec<i16>)>,
    fault: &Mutex<String>,
) {
    let values = data
        .iter()
        .map(|s| (convert(*s).clamp(-1., 1.) * 32767.).round() as i16)
        .collect();
    if tx.try_send((Instant::now(), values)).is_err() {
        if let Ok(mut err) = fault.lock() {
            *err = "El audio no pudo escribirse a tiempo".into();
        }
    }
}
impl AudioTrack {
    fn new(system: bool, dir: &Path) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = if system {
            host.default_output_device()
        } else {
            host.default_input_device()
        }
        .ok_or(if system {
            "No hay salida de audio"
        } else {
            "No hay microfono"
        })?;
        let config = if system {
            device.default_output_config()
        } else {
            device.default_input_config()
        }
        .map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::sync_channel(256);
        let fault = Arc::new(Mutex::new(String::new()));
        let errors = fault.clone();
        let callback_fault = fault.clone();
        let on_error = move |e: cpal::StreamError| {
            if let Ok(mut err) = errors.lock() {
                *err = e.to_string();
            }
        };
        let cfg: cpal::StreamConfig = config.clone().into();
        // CPAL's WASAPI backend enables loopback when an output device is opened as input.
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &cfg,
                move |d: &[f32], _| samples(d, |v| v, &tx, &callback_fault),
                on_error,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                &cfg,
                move |d: &[i16], _| samples(d, |v| v as f32 / 32768., &tx, &callback_fault),
                on_error,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_input_stream(
                &cfg,
                move |d: &[u16], _| {
                    samples(d, |v| (v as f32 - 32768.) / 32768., &tx, &callback_fault)
                },
                on_error,
                None,
            ),
            other => return Err(format!("Formato de audio no soportado: {other:?}")),
        }
        .map_err(|e| format!("No se pudo iniciar el audio: {e}"))?;
        let path = dir.join(if system {
            "system.pcm"
        } else {
            "microphone.pcm"
        });
        let file = File::create(&path).map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self {
            _stream: stream,
            rx,
            file,
            path,
            rate: cfg.sample_rate.0,
            channels: cfg.channels,
            written: 0,
            fault,
            scratch: Vec::with_capacity(8192),
        })
    }
    fn silence(&mut self, samples: u64) -> Result<(), String> {
        let zeros = [0u8; 8192];
        let mut bytes = samples * 2;
        while bytes > 0 {
            let n = bytes.min(zeros.len() as u64) as usize;
            self.file
                .write_all(&zeros[..n])
                .map_err(|e| e.to_string())?;
            bytes -= n as u64;
        }
        self.written += samples;
        Ok(())
    }
    fn drain(&mut self, start: Instant) -> Result<(), String> {
        let error = self.fault.lock().map_err(|_| "Audio ocupado")?.clone();
        if !error.is_empty() {
            return Err(error);
        }
        while let Ok((time, values)) = self.rx.try_recv() {
            let end_frame =
                (time.saturating_duration_since(start).as_secs_f64() * self.rate as f64) as u64;
            let begin = end_frame.saturating_sub(values.len() as u64 / self.channels as u64)
                * self.channels as u64;
            if begin > self.written {
                self.silence(begin - self.written)?;
            }
            pcm_bytes(&values,&mut self.scratch);
            self.file.write_all(&self.scratch).map_err(|e| e.to_string())?;
            self.written += values.len() as u64;
        }
        Ok(())
    }
    fn finish(&mut self, start: Instant, seconds: f64) -> Result<(), String> {
        self._stream.pause().map_err(|e| e.to_string())?;
        self.drain(start)?;
        let target = (seconds * self.rate as f64).round() as u64 * self.channels as u64;
        if target > self.written {
            self.silence(target - self.written)?;
        }
        self.file.set_len(target * 2).map_err(|e| e.to_string())?;
        self.file.sync_all().map_err(|e| e.to_string())
    }
}
fn finish_process(child: &mut Child) -> Result<(), String> {
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"q\n");
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err("El motor de video fallo; los archivos de recuperacion se conservaron".into())
            };
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(
                "El motor de video no respondio al detener; los originales se conservaron".into(),
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
fn pcm_bytes(values:&[i16],bytes:&mut Vec<u8>) {
    bytes.clear();
    for value in values {bytes.extend_from_slice(&value.to_le_bytes());}
}
fn capture(
    mut region: Region,
    prefs: Preferences,
    binary: PathBuf,
    dir: PathBuf,
    state: Arc<Mutex<Status>>,
    stop: mpsc::Receiver<CaptureControl>,
    ready: mpsc::Sender<Result<(), String>>,
    transcribe_microphone: bool,
) {
    let result = (|| -> Result<Option<PathBuf>, String> {
        let output_size=((region.width+1)/2*2,(region.height+1)/2*2);
        let mut segments = Vec::new();
        let mut elapsed = 0.;
        let mut cancelled = false;
        'capture: loop {
            let part = dir.join(format!("part-{:04}", segments.len()));
            fs::create_dir_all(&part).map_err(|e| e.to_string())?;
            let (path, action, seconds) = record(
                region,
                output_size,
                prefs.clone(),
                binary.clone(),
                &part,
                &state,
                &stop,
                &ready,
                elapsed,
                transcribe_microphone,
            )?;
            elapsed += seconds;
            if let Some(path) = path {
                segments.push(path);
            }
            match action {
                CaptureControl::Cancel => {
                    cancelled = true;
                    break;
                }
                CaptureControl::Pause => {
                    state.lock().map_err(|_| "Estado ocupado")?.phase = "paused".into();
                    loop {
                        match stop.recv().unwrap_or(CaptureControl::Stop) {
                            CaptureControl::Resume => break,
                            CaptureControl::Cancel => {
                                cancelled = true;
                                break 'capture;
                            }
                            CaptureControl::Stop => break 'capture,
                            CaptureControl::Pause => {}
                            CaptureControl::Reframe(next) => {region=next;}
                        }
                    }
                }
                CaptureControl::Reframe(next) => {region=next;}
                _ => break,
            }
        }
        if cancelled {
            // This UUID directory was created for this session; never touch previous captures.
            let resolved = dir.canonicalize().map_err(|e| e.to_string())?;
            let root = dir
                .parent()
                .ok_or("Directorio de captura invalido")?
                .canonicalize()
                .map_err(|e| e.to_string())?;
            if resolved.parent() != Some(root.as_path())
                || root.file_name().and_then(|name| name.to_str()) != Some("screen-recordings")
                || dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| uuid::Uuid::parse_str(name).ok())
                    .is_none()
            {
                return Err("Directorio de captura invalido".into());
            }
            fs::remove_dir_all(&resolved).map_err(|e| e.to_string())?;
            return Ok(None);
        }
        state.lock().map_err(|_| "Estado ocupado")?.phase = "saving".into();
        let output = dir.join(format!(
            "Whispera-{}.mp4",
            chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
        ));
        if segments.len() == 1 {
            fs::rename(&segments[0], &output).map_err(|e| e.to_string())?;
        } else {
            let manifest = segments
                .iter()
                .map(|path| {
                    format!(
                        "file '{}'\n",
                        path.strip_prefix(&dir)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/")
                    )
                })
                .collect::<String>();
            let list = dir.join("segments.txt");
            fs::write(&list, manifest).map_err(|e| e.to_string())?;
            let mut child = command(&binary)
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "concat",
                    "-safe",
                    "1",
                    "-i",
                ])
                .arg(&list)
                .args(["-c", "copy", "-movflags", "+faststart"])
                .arg(&output)
                .stdin(Stdio::null())
                .stderr(Stdio::from(
                    File::create(dir.join("join.log")).map_err(|e| e.to_string())?,
                ))
                .spawn()
                .map_err(|e| e.to_string())?;
            wait_encoder(&mut child)?;
            if fs::metadata(&output).map_err(|e| e.to_string())?.len() == 0 {
                return Err("El video quedo vacio".into());
            }
            for path in segments {
                let _ = fs::remove_file(path);
            }
        }
        state.lock().map_err(|_| "Estado ocupado")?.seconds = elapsed;
        Ok(Some(output))
    })();
    match result {
        Ok(Some(path)) => {
            let copied = copy_file(&path);
            if let Ok(mut status) = state.lock() {
                status.phase = "idle".into();
                status.path = path.to_string_lossy().into();
                status.copied = copied.is_ok();
                status.error = copied.err().unwrap_or_default();
            }
        }
        Ok(None) => {
            if let Ok(mut status) = state.lock() {
                *status = Status {
                    phase: "idle".into(),
                    ..Default::default()
                };
            }
        }
        Err(e) => {
            let _ = ready.send(Err(e.clone()));
            if let Ok(mut status) = state.lock() {
                status.phase = "error".into();
                status.error = format!("{e}. Recuperacion: {}", dir.display());
            }
        }
    }
}

fn wait_encoder(child: &mut Child) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(exit) = child.try_wait().map_err(|e| e.to_string())? {
            return if exit.success() {
                Ok(())
            } else {
                Err("No se pudo preparar el MP4; los originales se conservaron".into())
            };
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("La preparacion del MP4 tardo demasiado; originales conservados".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn record(
    region: Region,
    output_size:(u32,u32),
    prefs: Preferences,
    binary: PathBuf,
    dir: &Path,
    state: &Arc<Mutex<Status>>,
    stop: &mpsc::Receiver<CaptureControl>,
    ready: &mpsc::Sender<Result<(), String>>,
    elapsed: f64,
    transcribe_microphone: bool,
) -> Result<(Option<PathBuf>, CaptureControl, f64), String> {
    let mut tracks = Vec::new();
    if ["system", "both"].contains(&prefs.audio.as_str()) {
        tracks.push(AudioTrack::new(true, &dir)?);
    }
    if ["microphone", "both"].contains(&prefs.audio.as_str()) {
        tracks.push(AudioTrack::new(false, &dir)?);
    }
    let video = dir.join("video.mkv");
    let log = File::create(dir.join("capture.log")).map_err(|e| e.to_string())?;
    let mut child = command(&binary)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "gdigrab",
            "-framerate",
            "30",
            "-probesize",
            "32",
            "-analyzeduration",
            "0",
            "-draw_mouse",
            "1",
            "-offset_x",
            &region.x.to_string(),
            "-offset_y",
            &region.y.to_string(),
            "-video_size",
            &format!("{}x{}", region.width, region.height),
            "-i",
            "desktop",
            "-an",
            "-vf",
            &format!("pad=ceil(iw/2)*2:ceil(ih/2)*2,scale={}:{}:force_original_aspect_ratio=decrease:force_divisible_by=2,pad={}:{}:(ow-iw)/2:(oh-ih)/2,setsar=1",output_size.0,output_size.1,output_size.0,output_size.1),
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-tune",
            "zerolatency",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            "-flush_packets",
            "1",
        ])
        .arg(&video)
        .stdin(Stdio::piped())
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    // Wait for FFmpeg to actually create output, rather than reporting a successful spawn.
    let startup = (|| -> Result<(), String> {
        loop {
            if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Err(format!(
                    "No se pudo capturar la pantalla. Revisa {}",
                    dir.join("capture.log").display()
                ));
            }
            if fs::metadata(&video).map(|m| m.len() > 0).unwrap_or(false) {
                return Ok(());
            }
            for track in &mut tracks {
                track.drain(started)?;
            }
            if started.elapsed() > Duration::from_secs(10) {
                return Err("La captura de pantalla no inicio a tiempo".into());
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    })();
    if let Err(e) = startup {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e);
    }
    *state.lock().map_err(|_| "Estado ocupado")? = Status {
        phase: "recording".into(),
        seconds: elapsed,
        ..Default::default()
    };
    let _ = ready.send(Ok(()));
    let mut error = None;
    let mut reason = CaptureControl::Stop;
    let mut last_status=Instant::now();
    loop {
        match stop.recv_timeout(Duration::from_millis(25)) {
            Ok(CaptureControl::Resume) => continue,
            Ok(action) => {
                reason = action;
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            error = Some("La captura se interrumpio. Se conservaron los originales.".to_string());
            break;
        }
        for track in &mut tracks {
            if let Err(e) = track.drain(started) {
                error = Some(e);
                break;
            }
        }
        if error.is_some() {
            break;
        }
        if last_status.elapsed() >= Duration::from_millis(250) {
            state.lock().map_err(|_| "Estado ocupado")?.seconds = elapsed + started.elapsed().as_secs_f64();
            last_status=Instant::now();
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    {
        let mut status = state.lock().map_err(|_| "Estado ocupado")?;
        status.seconds = elapsed + seconds;
        status.phase = if reason == CaptureControl::Pause {
            "pausing"
        } else if matches!(reason,CaptureControl::Reframe(_)) {
            "reframing"
        } else {
            "saving"
        }
        .into();
    }
    if reason == CaptureControl::Cancel {
        let _ = child.kill();
        let _ = child.wait();
        drop(tracks);
        return Ok((None, reason, seconds));
    }
    let finalized = finish_process(&mut child);
    for track in &mut tracks {
        track.finish(started, seconds)?;
    }
    finalized?;
    if let Some(e) = error {
        return Err(e);
    }
    let output = dir.join(format!(
        "Whispera-{}.mp4",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
    ));
    let mut mux = command(&binary);
    mux.args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&video);
    for track in &tracks {
        mux.args([
            "-f",
            "s16le",
            "-ar",
            &track.rate.to_string(),
            "-ac",
            &track.channels.to_string(),
            "-i",
        ])
        .arg(&track.path);
    }
    mux.args(["-map", "0:v:0", "-c:v", "copy"]);
    if tracks.len() == 2 {
        mux.args([
            "-filter_complex",
            "[1:a][2:a]amix=inputs=2:duration=longest:normalize=0,alimiter=limit=0.95[a]",
            "-map",
            "[a]",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
        ]);
    } else if tracks.len() == 1 {
        mux.args(["-map", "1:a:0", "-c:a", "aac", "-b:a", "192k"]);
    }
    // Resume can reopen a different Windows default device. Keep the encoded
    // audio format stable so all segments concatenate without resampling video.
    if !tracks.is_empty() {
        mux.args(["-ar", "48000", "-ac", "2"]);
    }
    mux.args(["-movflags", "+faststart"])
        .arg(&output)
        .stdin(Stdio::null())
        .stderr(Stdio::from(
            File::create(dir.join("encode.log")).map_err(|e| e.to_string())?,
        ));
    let mut mux_child = mux.spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(exit) = mux_child.try_wait().map_err(|e| e.to_string())? {
            if !exit.success() {
                return Err(format!(
                    "No se pudo preparar el MP4. Originales en {}",
                    dir.display()
                ));
            }
            break;
        }
        if Instant::now() > deadline {
            let _ = mux_child.kill();
            let _ = mux_child.wait();
            return Err("La preparacion del MP4 tardo demasiado; originales conservados".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut preserve_microphone = false;
    if transcribe_microphone {
        if let Some(track) = tracks.iter().find(|track| track.path.file_name().is_some_and(|name| name == "microphone.pcm")) {
            if let Err(error) = crate::video_transcript::prepare(&binary, &track.path, track.rate, track.channels) {
                preserve_microphone = true;
                let _ = fs::write(dir.join("speech-error.txt"), error);
            }
        }
    }
    drop(tracks);
    // Only remove intermediate files after a complete, nonempty MP4 exists.
    if fs::metadata(&output).map_err(|e| e.to_string())?.len() == 0 {
        return Err("El video quedo vacio".into());
    }
    for file in ["video.mkv", "system.pcm", "microphone.pcm"] {
        if file == "microphone.pcm" && preserve_microphone { continue; }
        let _ = fs::remove_file(dir.join(file));
    }
    Ok((Some(output), reason, seconds))
}

#[cfg(test)]
mod tests {
    #[test]
    fn video_remains_busy_until_finalization() {
        let screen = Screen::new();
        for phase in ["recording", "paused", "reframing", "saving"] {
            screen.state.lock().unwrap().phase = phase.into();
            assert!(screen.busy());
        }
        screen.state.lock().unwrap().phase = "idle".into();
        assert!(!screen.busy());
    }
    use super::*;
    #[test]
    fn pcm_scratch_reuses_allocation_without_changing_samples() {
        let mut bytes=Vec::with_capacity(8192);
        let pointer=bytes.as_ptr();
        let values=[i16::MIN,-1,0,1,i16::MAX];
        pcm_bytes(&values,&mut bytes);
        assert_eq!(bytes,values.iter().flat_map(|value|value.to_le_bytes()).collect::<Vec<_>>());
        pcm_bytes(&[123,-456],&mut bytes);
        assert_eq!(bytes,[123i16,-456].iter().flat_map(|value|value.to_le_bytes()).collect::<Vec<_>>());
        assert_eq!(bytes.as_ptr(),pointer);
        pcm_bytes(&[],&mut bytes);assert!(bytes.is_empty());
    }
    #[test]
    fn existing_video_preferences_keep_their_shortcut() {
        let p: Preferences = serde_json::from_str(r#"{"audio":"none","hotkey":"alt+x"}"#).unwrap();
        assert_eq!(p.hotkey, "alt+x");
        assert_eq!(p.image_hotkey, "Control+Shift+F10");
        assert_eq!(p.frame_color, "#ffffff");
        assert!(!p.image_auto_copy);
    }
    #[test]
    fn border_color_is_validated() {
        assert!(valid_frame_color("#ffffff"));
        assert!(valid_frame_color("#12Ab34"));
        for invalid in ["white", "#fff", "#12345z", "#1234567", ""] {
            assert!(!valid_frame_color(invalid));
        }
    }
    #[test]
    fn shortcut_queued_during_startup_stops_the_recording() {
        let screen = Arc::new(Screen::new());
        let gate = screen.gate.lock().unwrap();
        screen.state.lock().unwrap().phase = "selecting".into();
        let queued = screen.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            entered_tx.send(()).unwrap();
            let (_gate, action) = gated_action(&queued).unwrap();
            action
        });
        entered_rx.recv().unwrap();
        screen.state.lock().unwrap().phase = "recording".into();
        drop(gate);
        assert_eq!(worker.join().unwrap(), ToggleAction::Stop);
    }
    #[test]
    fn shortcut_finishes_paused_video_without_starting_another_selection() {
        let screen = Screen::new();
        for phase in ["paused", "pausing", "resuming", "reframing"] {
            screen.state.lock().unwrap().phase = phase.into();
            assert!(screen.busy());
            let (_guard, action) = gated_action(&screen).unwrap();
            assert_eq!(action, ToggleAction::Stop);
        }
    }
    #[test]
    #[ignore = "Records two tiny desktop regions without audio; run explicitly on Windows"]
    fn native_reframe_keeps_resolution_and_decodes_joined_video() {
        let binary=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../node_modules/ffmpeg-static/ffmpeg.exe");
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/screen-smoke").join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        let regions=[Region{x:0,y:0,width:129,height:131},Region{x:20,y:20,width:160,height:90}];
        let mut elapsed=0.;
        for (index,area) in regions.into_iter().enumerate() {
            let dir=root.join(format!("part-{index:04}"));fs::create_dir_all(&dir).unwrap();
            let (tx,rx)=mpsc::channel();let (ready_tx,ready_rx)=mpsc::channel();
            let binary_clone=binary.clone();let dir_clone=dir.clone();let start=elapsed;
            let worker=std::thread::spawn(move||record(area,(130,132),Preferences{audio:"none".into(),..Default::default()},binary_clone,&dir_clone,&Arc::new(Mutex::new(Status::default())),&rx,&ready_tx,start,false));
            ready_rx.recv_timeout(Duration::from_secs(15)).unwrap().unwrap();
            std::thread::sleep(Duration::from_millis(400));
            let action=if index==0 {CaptureControl::Reframe(regions[1])}else{CaptureControl::Stop};
            tx.send(action).unwrap();
            let (path,reason,seconds)=worker.join().unwrap().unwrap();assert_eq!(reason,action);assert!(path.is_some());elapsed+=seconds;
        }
        // Use actual segment filenames; they are timestamped by the recorder.
        let mut list=String::new();
        for index in 0..2 {
            let path=fs::read_dir(root.join(format!("part-{index:04}"))).unwrap().filter_map(Result::ok).map(|e|e.path()).find(|p|p.extension().is_some_and(|e|e=="mp4")).unwrap();
            list.push_str(&format!("file '{}'\n",path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\',"/")));
        }
        fs::write(root.join("segments.txt"),list).unwrap();
        let joined=root.join("joined.mp4");
        let join=command(&binary).args(["-hide_banner","-loglevel","error","-y","-f","concat","-safe","1","-i"]).arg(root.join("segments.txt")).args(["-c","copy","-movflags","+faststart"]).arg(&joined).output().unwrap();
        assert!(join.status.success(),"{}",String::from_utf8_lossy(&join.stderr));
        let decode=command(&binary).args(["-hide_banner","-i"]).arg(&joined).args(["-f","null","-"]).output().unwrap();
        let info=String::from_utf8_lossy(&decode.stderr);
        assert!(decode.status.success()&&info.contains("130x132")&&info.contains("Video: h264"),"{info}");
        assert!(!info.contains("Audio:"));assert!(elapsed>0.);
        let resolved=root.canonicalize().unwrap();let allowed=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/screen-smoke").canonicalize().unwrap();assert!(resolved.starts_with(allowed));fs::remove_dir_all(resolved).unwrap();
    }
    #[test]
    #[ignore = "Records a tiny desktop region and optional default audio devices; run explicitly on Windows"]
    fn native_capture_smoke_all_audio_modes() {
        let binary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../node_modules/ffmpeg-static/ffmpeg.exe");
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/screen-smoke")
            .join(uuid::Uuid::new_v4().to_string());
        for audio in ["none", "system", "microphone", "both"] {
            let dir = root.join(audio);
            fs::create_dir_all(&dir).unwrap();
            let (stop_tx, stop_rx) = mpsc::channel();
            let (ready_tx, ready_rx) = mpsc::channel();
            let state = Arc::new(Mutex::new(Status::default()));
            let binary_clone = binary.clone();
            let dir_clone = dir.clone();
            let prefs = Preferences {
                audio: audio.into(),
                ..Default::default()
            };
            let thread = std::thread::spawn(move || {
                let result = record(
                    Region {
                        x: 0,
                        y: 0,
                        width: 129,
                        height: 131,
                    },
                    (130,132),
                    prefs,
                    binary_clone,
                    &dir_clone,
                    &state,
                    &stop_rx,
                    &ready_tx,
                    0.,
                    false,
                );
                if let Err(error) = &result {
                    let _ = ready_tx.send(Err(error.clone()));
                }
                result
            });
            ready_rx
                .recv_timeout(Duration::from_secs(15))
                .expect("capture startup")
                .expect("capture ready");
            std::thread::sleep(Duration::from_millis(400));
            stop_tx.send(CaptureControl::Stop).unwrap();
            let (path, reason, _) = thread.join().unwrap().expect("MP4 finalization");
            assert_eq!(reason, CaptureControl::Stop);
            let path = path.expect("saved recording");
            assert!(fs::metadata(&path).unwrap().len() > 1000);
            let info = command(&binary)
                .args(["-hide_banner", "-i"])
                .arg(&path)
                .args(["-f", "null", "-"])
                .stderr(Stdio::piped())
                .output()
                .unwrap();
            assert!(
                info.status.success(),
                "{}",
                String::from_utf8_lossy(&info.stderr)
            );
            let metadata = String::from_utf8_lossy(&info.stderr);
            assert!(
                metadata.contains("Video: h264") && metadata.contains("130x132"),
                "{metadata}"
            );
            assert_eq!(
                metadata.contains("Audio: aac"),
                audio != "none",
                "{metadata}"
            );
        }
        // This smoke test never writes to the clipboard and removes its recordings.
        let resolved = root.canonicalize().unwrap();
        let allowed = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/screen-smoke")
            .canonicalize()
            .unwrap();
        assert!(resolved.starts_with(allowed));
        fs::remove_dir_all(resolved).unwrap();
    }
    #[test]
    fn region_respects_dpi_and_negative_monitor_origin() {
        let r = region(
            Rect {
                x: 10.,
                y: 20.,
                width: 101.,
                height: 80.,
            },
            1.5,
            tauri::PhysicalPosition::new(-1920, 0),
            tauri::PhysicalSize::new(1920, 1080),
        )
        .unwrap();
        assert_eq!(
            r,
            Region {
                x: -1905,
                y: 30,
                width: 152,
                height: 120
            }
        );
    }
    #[test]
    fn rejects_invalid_or_out_of_bounds_selection() {
        for x in [f64::NAN, -1., 1900.] {
            assert!(region(
                Rect {
                    x,
                    y: 0.,
                    width: 100.,
                    height: 100.
                },
                1.,
                tauri::PhysicalPosition::new(0, 0),
                tauri::PhysicalSize::new(1920, 1080)
            )
            .is_err());
        }
    }
}
