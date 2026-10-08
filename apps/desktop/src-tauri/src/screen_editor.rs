//! Separate ink and tool windows: annotations enter the recording; controls do not.
use crate::screen::{self, Rect, Region};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;

#[derive(Clone, Serialize)]
pub struct Context {
    pub id: String,
    pub kind: String,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    pub frame_color: String,
    pub hud_scale: f64,
    pub hud_above: bool,
}
#[derive(Default)]
pub struct Editor {
    configured: AtomicBool,
    context: Mutex<Option<Context>>,
    region: Mutex<Option<Region>>,
    monitor: Mutex<Option<Region>>,
    image: Mutex<Vec<u8>>,
    feedback: Mutex<serde_json::Value>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Action {
    pub action: String,
    pub value: Option<String>,
}
fn context(app: &tauri::AppHandle, id: &str) -> Result<Context, String> {
    app.state::<Editor>()
        .context
        .lock()
        .map_err(|_| "Editor ocupado")?
        .as_ref()
        .filter(|c| c.id == id)
        .cloned()
        .ok_or_else(|| "La captura ya termino".into())
}
pub fn hide(app: &tauri::AppHandle) {
    for label in ["screen-ink", "screen-tools", "screen-hud"] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.hide();
            let _ = w.emit("screen-editor-reset", Option::<Context>::None);
        }
    }
    if let Some(editor) = app.try_state::<Editor>() {
        editor.configured.store(false, Ordering::SeqCst);
        if let Ok(mut value) = editor.context.lock() {
            *value = None;
        }
        if let Ok(mut bytes) = editor.image.lock() {
            bytes.clear();
        }
    }
}
fn window(
    app: &tauri::AppHandle,
    label: &str,
    protected: bool,
) -> Result<tauri::WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(label) {
        return Ok(w);
    }
    let builder = tauri::WebviewWindowBuilder::new(
        app,
        label,
        tauri::WebviewUrl::App(format!("overlay.html?view={label}").into()),
    )
    .title(match label {
        "screen-tools" => "Whispera · Herramientas de captura",
        "screen-hud" => "Whispera · Controles de captura",
        _ => "Whispera · Anotaciones",
    })
    .transparent(true)
    .background_color(tauri::window::Color(0, 0, 0, 0))
    .content_protected(protected)
    .decorations(false)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(false)
    .focused(false);
    // Owned tool windows stay above the ink when it gains focus. Merely
    // setting an already-topmost window topmost again does not establish this
    // ordering; the ink could then intercept every subsequent toolbar click.
    let builder = if protected {
        builder
            .parent(
                &app.get_webview_window("screen-ink")
                    .ok_or("Editor no disponible")?,
            )
            .map_err(|e| e.to_string())?
    } else {
        builder
    };
    let window = builder.build().map_err(|e| e.to_string())?;
    if label == "screen-ink" {
        let app = app.clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Focused(true)) {
                raise_controls(&app);
            }
        });
    }
    Ok(window)
}
fn raise_controls(app: &tauri::AppHandle) {
    for label in ["screen-tools", "screen-hud"] {
        if let Some(window) = app.get_webview_window(label) {
            if window.is_visible().unwrap_or(false) {
                let _ = window.set_always_on_top(true);
            }
        }
    }
}
fn pixels(value: f64, scale: f64) -> u32 {
    (value * scale).round().max(1.) as u32
}
fn fit_start(value: i32, origin: i32, span: u32, size: u32, gap: i32) -> i32 {
    value.clamp(
        origin + gap,
        (origin + span as i32 - size as i32 - gap).max(origin + gap),
    )
}
fn intersection(a: Region, b: Region) -> u64 {
    let w = (a.x + a.width as i32).min(b.x + b.width as i32) - a.x.max(b.x);
    let h = (a.y + a.height as i32).min(b.y + b.height as i32) - a.y.max(b.y);
    w.max(0) as u64 * h.max(0) as u64
}
fn external_dock(region: Region, monitor: Region, width: u32, height: u32, gap: i32, vertical: bool, avoid: Option<Region>) -> Region {
    let cx = region.x + (region.width as i32 - width as i32) / 2;
    let cy = region.y + (region.height as i32 - height as i32) / 2;
    let left = (region.x - width as i32 - gap, cy);
    let right = (region.x + region.width as i32 + gap, cy);
    let above = (cx, region.y - height as i32 - gap);
    let below = (cx, region.y + region.height as i32 + gap);
    let mut candidates = if vertical { vec![left, right, above, below] } else { vec![below, above, right, left] };
    if let Some(other) = avoid {
        candidates.push((other.x + other.width as i32 + gap, cy));
        candidates.push((other.x - width as i32 - gap, cy));
        for x in [right.0, left.0] {
            candidates.push((x, other.y - height as i32 - gap));
            candidates.push((x, other.y + other.height as i32 + gap));
        }
    }
    candidates.into_iter().map(|(x,y)| Region {
        x: fit_start(x, monitor.x, monitor.width, width, gap),
        y: fit_start(y, monitor.y, monitor.height, height, gap),
        width, height,
    }).min_by_key(|dock| (
        intersection(*dock, region),
        avoid.map_or(0, |other| intersection(*dock, other)),
    )).unwrap()
}
fn tool_bounds(region: Region, monitor: Region, scale: f64, compact: bool) -> Region {
    let gap = pixels(10., scale) as i32;
    let width = pixels(50., scale).min(monitor.width.saturating_sub(2 * gap as u32));
    let full_height = pixels(450., scale).min(monitor.height.saturating_sub(2 * gap as u32));
    let height = if compact { pixels(96., scale).min(full_height) } else { full_height };
    external_dock(region, monitor, width, height, gap, true, None)
}
fn hud_anchor(region: Region, monitor: Region, scale: f64, _hud_scale: f64) -> (Region, bool) {
    let gap = pixels(10., scale) as i32;
    let width = pixels(260., scale)
        .min(monitor.width.saturating_sub(2 * gap as u32));
    let height = pixels(56., scale);
    let anchor = external_dock(region, monitor, width, height, gap, false,
        Some(tool_bounds(region, monitor, scale, false)));
    (anchor, false)
}
fn fit_hud_scale(region: Region, monitor: Region, scale: f64, preferred: f64) -> f64 {
    let initial = preferred.min(0.85);
    let (anchor, _) = hud_anchor(region, monitor, scale, initial);
    let gap = pixels(10., scale) as i32;
    let below = monitor.y + monitor.height as i32 - gap - anchor.y;
    let above = anchor.y + anchor.height as i32 - monitor.y - gap;
    initial.min(((above.max(below) as f64 / scale - 64.) / 450.).max(0.15))
}
fn dock_bounds(
    ctx: &Context,
    region: Region,
    monitor: Region,
    label: &str,
    compact: bool,
) -> Result<Region, String> {
    if label == "screen-tools" {
        return Ok(tool_bounds(region, monitor, ctx.scale, compact));
    }
    if label != "screen-hud" {
        return Err("Vista incorrecta".into());
    }
    let (dock, _) = hud_anchor(region, monitor, ctx.scale, ctx.hud_scale);
    Ok(dock)
}
fn place_controls(
    window: &tauri::WebviewWindow,
    ctx: &Context,
    region: Region,
    monitor: Region,
    compact: bool,
) -> Result<(), String> {
    let dock = dock_bounds(ctx, region, monitor, window.label(), compact)?;
    window
        .set_size(tauri::PhysicalSize::new(dock.width, dock.height))
        .map_err(|e| e.to_string())?;
    window
        .set_position(tauri::PhysicalPosition::new(dock.x, dock.y))
        .map_err(|e| e.to_string())
}
fn screenshot(app: &tauri::AppHandle, region: Region, exclude_ink: bool) -> Result<Vec<u8>, String> {
    // A hidden ink window may still be in DWM's close animation. Exclude it
    // only while sampling, then restore its affinity for video annotations.
    let _ink_exclusion=if exclude_ink {app.get_webview_window("screen-ink")} else {None}.map(|window| {
        let hwnd=windows::Win32::Foundation::HWND(window.hwnd().map_err(|e|e.to_string())?.0);
        crate::screen_capture::InkExclusion::new(hwnd)
    }).transpose()?;
    match crate::screen_capture::png(region) {
        Ok(bytes) => return Ok(bytes),
        Err(error) => { let _=app.state::<crate::storage::Store>().event(&format!("Captura nativa no disponible; usando respaldo FFmpeg: {error}")); }
    }
    screenshot_ffmpeg(app,region)
}
fn screenshot_ffmpeg(app: &tauri::AppHandle, region: Region) -> Result<Vec<u8>, String> {
    let root = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("screenshots");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let path = root.join(format!("{}.png", uuid::Uuid::new_v4()));
    let mut child = screen::command(&screen::ffmpeg(app)?)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "gdigrab",
            "-draw_mouse",
            "0",
            "-offset_x",
            &region.x.to_string(),
            "-offset_y",
            &region.y.to_string(),
            "-video_size",
            &format!("{}x{}", region.width, region.height),
            "-i",
            "desktop",
            "-frames:v",
            "1",
            "-update",
            "1",
        ])
        .arg(&path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if let Some(exit) = child.try_wait().map_err(|e| e.to_string())? {
            if !exit.success() {
                let _ = fs::remove_file(&path);
                return Err("No se pudo capturar la imagen".into());
            }
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&path);
            return Err("La captura de imagen no respondio".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string());
    let _ = fs::remove_file(&path);
    bytes
}
pub fn open(
    app: &tauri::AppHandle,
    source: &tauri::WebviewWindow,
    rect: Rect,
    region: Region,
    kind: &str,
) -> Result<(), String> {
    open_with_image(app, source, rect, region, kind, None)
}
pub fn copy_immediately(
    app: &tauri::AppHandle,
    source: &tauri::WebviewWindow,
    rect: Rect,
    region: Region,
    original: Vec<u8>,
) -> Result<bool, String> {
    let bytes = crate::screen_capture::rounded_png(&original, source.scale_factor().map_err(|e| e.to_string())?)?;
    let copy = (|| -> Result<(), String> {
        let root = app
            .path()
            .app_cache_dir()
            .map_err(|e| e.to_string())?
            .join("screenshots");
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let path = root.join(format!("{}.png", uuid::Uuid::new_v4()));
        fs::write(&path, &bytes).map_err(|e| e.to_string())?;
        crate::capture_history::remember(&app.state::<crate::storage::Store>(), "image", &path)?;
        let image = tauri::image::Image::from_bytes(&bytes).map_err(|e| e.to_string())?;
        app.clipboard()
            .write_image(&image)
            .map_err(|e| e.to_string())
    })();
    if let Err(error) = copy {
        open_with_image(app, source, rect, region, "image", Some(original))?;
        *app.state::<Editor>()
            .feedback
            .lock()
            .map_err(|_| "Editor ocupado")? = serde_json::json!({"error": format!("No se pudo copiar automaticamente: {error}. La imagen sigue abierta para reintentar.")});
        return Ok(false);
    }
    Ok(true)
}
pub(crate) fn open_with_image(
    app: &tauri::AppHandle,
    source: &tauri::WebviewWindow,
    rect: Rect,
    region: Region,
    kind: &str,
    image: Option<Vec<u8>>,
) -> Result<(), String> {
    hide(app);
    let bytes = match image {
        Some(bytes) => bytes,
        None if kind == "image" => screenshot(app, region, true)?,
        _ => Vec::new(),
    };
    let scale = source.scale_factor().map_err(|e| e.to_string())?;
    let position = source.inner_position().map_err(|e| e.to_string())?;
    let size = source.inner_size().map_err(|e| e.to_string())?;
    let monitor = Region {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    };
    let hud_scale = fit_hud_scale(
        region,
        monitor,
        scale,
        app.state::<crate::storage::Store>()
            .get::<crate::storage::Settings>("settings")?
            .recorder_scale,
    );
    let hud_above = hud_anchor(region, monitor, scale, hud_scale).1;
    let ctx = Context {
        id: uuid::Uuid::new_v4().to_string(),
        kind: kind.into(),
        width: rect.width,
        height: rect.height,
        scale,
        hud_scale,
        hud_above,
        frame_color: app
            .state::<crate::storage::Store>()
            .get::<screen::Preferences>("screen_preferences")
            .unwrap_or_default()
            .frame_color,
    };
    let editor = app.state::<Editor>();
    *editor.region.lock().map_err(|_| "Editor ocupado")? = Some(region);
    *editor.monitor.lock().map_err(|_| "Editor ocupado")? = Some(monitor);
    *editor.image.lock().map_err(|_| "Editor ocupado")? = bytes;
    *editor.context.lock().map_err(|_| "Editor ocupado")? = Some(ctx.clone());
    *editor.feedback.lock().map_err(|_| "Editor ocupado")? = serde_json::json!({});
    let ink = window(app, "screen-ink", false)?;
    ink.set_position(tauri::PhysicalPosition::new(region.x, region.y))
        .map_err(|e| e.to_string())?;
    ink.set_size(tauri::PhysicalSize::new(region.width, region.height))
        .map_err(|e| e.to_string())?;
    ink.set_ignore_cursor_events(kind == "video")
        .map_err(|e| e.to_string())?;
    ink.set_focusable(kind != "video")
        .map_err(|e| e.to_string())?;
    let tools = window(app, "screen-tools", true)?;
    screen::protect(&tools)?;
    place_controls(&tools, &ctx, region, monitor, false)?;
    let hud = window(app, "screen-hud", true)?;
    screen::protect(&hud)?;
    place_controls(&hud, &ctx, region, monitor, false)?;
    for w in [&ink, &tools, &hud] {
        w.emit("screen-editor-reset", &ctx)
            .map_err(|e| e.to_string())?;
    }
    editor.configured.store(true, Ordering::SeqCst);
    Ok(())
}
#[tauri::command]
pub fn screen_editor_context(editor: State<Editor>) -> Option<Context> {
    editor
        .context
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}
#[tauri::command]
pub fn screen_editor_image(app: tauri::AppHandle, id: String) -> Result<Vec<u8>, String> {
    if context(&app, &id)?.kind != "image" {
        return Err("No hay una imagen activa".into());
    }
    Ok(app
        .state::<Editor>()
        .image
        .lock()
        .map_err(|_| "Editor ocupado")?
        .clone())
}
#[tauri::command]
pub fn screen_editor_ready(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
) -> Result<bool, String> {
    let ctx = context(&app, &id)?;
    if !app.state::<Editor>().configured.load(Ordering::SeqCst) {
        return Ok(false);
    }
    if !["screen-tools", "screen-ink", "screen-hud"].contains(&window.label()) {
        return Err("Vista incorrecta".into());
    }
    window.show().map_err(|e| e.to_string())?;
    raise_controls(&app);
    if window.label() == "screen-ink" && ctx.kind == "image" {
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(true)
}
#[tauri::command]
pub fn screen_overlay_layout(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
    compact: bool,
) -> Result<(), String> {
    let ctx = context(&app, &id)?;
    let editor = app.state::<Editor>();
    let region = editor
        .region
        .lock()
        .map_err(|_| "Editor ocupado")?
        .ok_or("Captura no disponible")?;
    let monitor = editor
        .monitor
        .lock()
        .map_err(|_| "Editor ocupado")?
        .ok_or("Monitor no disponible")?;
    place_controls(&window, &ctx, region, monitor, compact)
}

// Expand only while a panel is visible; the rail stays at the same screen position.
#[tauri::command]
pub fn screen_tools_panel(app: tauri::AppHandle, window: tauri::WebviewWindow, id: String,
    open: bool, compact: bool, anchor: f64, panel_height: f64) -> Result<serde_json::Value, String> {
    if window.label() != "screen-tools" { return Err("Vista incorrecta".into()); }
    let ctx=context(&app,&id)?;
    let editor=app.state::<Editor>();
    let region=editor.region.lock().map_err(|_|"Editor ocupado")?.ok_or("Captura no disponible")?;
    let monitor=editor.monitor.lock().map_err(|_|"Editor ocupado")?.ok_or("Monitor no disponible")?;
    let rail=tool_bounds(region,monitor,ctx.scale,compact);
    if !open {
        place_controls(&window,&ctx,region,monitor,compact)?;
        return Ok(serde_json::json!({"railX":4,"railY":4,"menuX":0,"menuY":0}));
    }
    if !anchor.is_finite() || !panel_height.is_finite() { return Err("Posicion incorrecta".into()); }
    let gap=pixels(10.,ctx.scale) as i32;
    let width=pixels(292.,ctx.scale).min(monitor.width.saturating_sub(2*gap as u32));
    let height=pixels(panel_height.clamp(100.,380.),ctx.scale).min(monitor.height.saturating_sub(2*gap as u32));
    let right=rail.x+rail.width as i32+gap;
    let menu_x=if right+width as i32+gap<=monitor.x+monitor.width as i32 { right } else { rail.x-width as i32-gap };
    let menu_x=fit_start(menu_x,monitor.x,monitor.width,width,gap);
    let menu_y=fit_start(rail.y+(anchor.clamp(0.,442.)*ctx.scale).round() as i32-height as i32/2,monitor.y,monitor.height,height,gap);
    let x=rail.x.min(menu_x);let y=rail.y.min(menu_y);
    window.set_size(tauri::PhysicalSize::new((rail.x+rail.width as i32).max(menu_x+width as i32)-x,(rail.y+rail.height as i32).max(menu_y+height as i32)-y)).map_err(|e|e.to_string())?;
    window.set_position(tauri::PhysicalPosition::new(x,y)).map_err(|e|e.to_string())?;
    Ok(serde_json::json!({"railX":(rail.x-x) as f64/ctx.scale+4.,"railY":(rail.y-y) as f64/ctx.scale+4.,"menuX":(menu_x-x) as f64/ctx.scale+4.,"menuY":(menu_y-y) as f64/ctx.scale+4.}))
}

#[tauri::command]
pub fn screen_editor_action(
    app: tauri::AppHandle,
    id: String,
    action: Action,
) -> Result<(), String> {
    let ctx = context(&app, &id)?;
    if ![
        "tool", "color", "width", "undo", "redo", "clear", "delete", "copy", "save", "print",
        "close", "reselect",
    ]
    .contains(&action.action.as_str())
    {
        return Err("Accion desconocida".into());
    }
    if action.action == "tool" {
        let tool = action.value.as_deref().unwrap_or("");
        if ![
            "pointer",
            "pen",
            "line",
            "arrow",
            "rectangle",
            "ellipse", "triangle", "diamond", "hexagon", "star",
            "highlight",
            "text",
            "blur",
        ]
        .contains(&tool)
        {
            return Err("Herramienta desconocida".into());
        }
        if let Some(w) = app.get_webview_window("screen-ink") {
            let passthrough = ctx.kind == "video" && tool == "pointer";
            w.set_ignore_cursor_events(passthrough)
                .map_err(|e| e.to_string())?;
            w.set_focusable(!passthrough).map_err(|e| e.to_string())?;
            if !passthrough {
                w.set_focus().map_err(|e| e.to_string())?;
            }
        }
        raise_controls(&app);
    }
    app.emit_to(
        "screen-ink",
        "screen-editor-action",
        serde_json::json!({"id":id,"action":action}),
    )
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn screen_editor_feedback(
    app: tauri::AppHandle,
    id: String,
    feedback: serde_json::Value,
) -> Result<(), String> {
    context(&app, &id)?;
    *app.state::<Editor>()
        .feedback
        .lock()
        .map_err(|_| "Editor ocupado")? = feedback.clone();
    for label in ["screen-tools", "screen-hud"] {
        app.emit_to(
            label,
            "screen-editor-feedback",
            serde_json::json!({"id":id,"feedback":feedback}),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
pub fn screen_editor_feedback_get(editor: State<Editor>) -> serde_json::Value {
    editor
        .feedback
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}
#[tauri::command]
pub fn screen_editor_print(
    app: tauri::AppHandle,
    id: String,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    if context(&app, &id)?.kind != "image" || window.label() != "screen-ink" {
        return Err("No hay imagen para imprimir".into());
    }
    window.print().map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn screen_image_export(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
    bytes: Vec<u8>,
    action: String,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = context(&app, &id)?;
        if ctx.kind != "image" || window.label() != "screen-ink" || bytes.len() > 64 * 1024 * 1024 {
            return Err("Imagen no disponible".into());
        }
        let image = tauri::image::Image::from_bytes(&bytes).map_err(|e| e.to_string())?;
        if image.width() > 8192 || image.height() > 8192 {
            return Err("Imagen demasiado grande".into());
        }
        let bytes = crate::screen_capture::rounded_png(&bytes, ctx.scale)?;
        let image = tauri::image::Image::from_bytes(&bytes).map_err(|e| e.to_string())?;
        let saved_path = match action.as_str() {
            "copy" => {
                app.clipboard()
                    .write_image(&image)
                    .map_err(|e| e.to_string())?;
                let root = app
                    .path()
                    .app_cache_dir()
                    .map_err(|e| e.to_string())?
                    .join("screenshots");
                fs::create_dir_all(&root).map_err(|e| e.to_string())?;
                let path = root.join(format!("{}.png", uuid::Uuid::new_v4()));
                fs::write(&path, &bytes).map_err(|e| e.to_string())?;
                path
            }
            "save" => {
                let path = app
                    .dialog()
                    .file()
                    .add_filter("Imagen PNG", &["png"])
                    .set_file_name("Whispera.png")
                    .set_parent(&window)
                    .blocking_save_file();
                let Some(path) = path else {
                    return Ok(false);
                };
                let path = path.into_path().map_err(|e| e.to_string())?;
                fs::write(&path, &bytes).map_err(|e| e.to_string())?;
                path
            }
            _ => return Err("Accion desconocida".into()),
        };
        crate::capture_history::remember(
            &app.state::<crate::storage::Store>(),
            "image",
            &saved_path,
        )?;
        screen::cancel_selection(app.clone())?;
        Ok(true)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn screen_video_snapshot(app: tauri::AppHandle, window: tauri::WebviewWindow, id: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let screen = app.state::<screen::Screen>();
        let _gate = screen.gate.lock().map_err(|_| "Captura ocupada")?;
        let ctx = context(&app, &id)?;
        if ctx.kind != "video" || !["screen-hud", "screen-tools"].contains(&window.label()) {
            return Err("No hay un video para capturar".into());
        }
        if !["recording", "paused"].contains(&screen.state.lock().map_err(|_| "Estado ocupado")?.phase.as_str()) {
            return Err("El video no esta activo".into());
        }
        let region = app.state::<Editor>().region.lock().map_err(|_| "Editor ocupado")?
            .as_ref().copied().ok_or("Area de captura no disponible")?;
        // Keep live ink in this still frame; protected tools/HUD are excluded by Windows.
        let bytes = screenshot(&app, region, false)?;
        let bytes = crate::screen_capture::rounded_png(&bytes, ctx.scale)?;
        let root = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("screenshots");
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let path = root.join(format!("{}.png", uuid::Uuid::new_v4()));
        fs::write(&path, &bytes).map_err(|e| e.to_string())?;
        crate::capture_history::remember(&app.state::<crate::storage::Store>(), "image", &path)?;
        let image = tauri::image::Image::from_bytes(&bytes).map_err(|e| e.to_string())?;
        app.clipboard().write_image(&image).map_err(|e| format!("Captura guardada, pero no se pudo copiar: {e}"))?;
        Ok(true)
    }).await.map_err(|e| e.to_string())?
}
fn sample_region(area: Region, scale: f64, rect: Rect) -> Result<Region, String> {
    if ![rect.x, rect.y, rect.width, rect.height]
        .iter()
        .all(|v| v.is_finite())
        || rect.x < 0.
        || rect.y < 0.
        || rect.width < 1.
        || rect.height < 1.
    {
        return Err("Area de difuminado invalida".into());
    }
    let x = (rect.x * scale).round() as u32;
    let y = (rect.y * scale).round() as u32;
    let width = (rect.width * scale).round() as u32;
    let height = (rect.height * scale).round() as u32;
    if x.saturating_add(width) > area.width || y.saturating_add(height) > area.height {
        return Err("El difuminado debe quedar dentro de la captura".into());
    }
    Ok(Region {
        x: area.x + x as i32,
        y: area.y + y as i32,
        width,
        height,
    })
}
#[tauri::command]
pub async fn screen_editor_sample(
    app: tauri::AppHandle,
    id: String,
    rect: Rect,
) -> Result<Vec<u8>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = context(&app, &id)?;
        let area = app
            .state::<Editor>()
            .region
            .lock()
            .map_err(|_| "Editor ocupado")?
            .ok_or("Captura no disponible")?;
        let bytes = screenshot(&app, sample_region(area, ctx.scale, rect)?, false)?;
        context(&app, &id)?;
        Ok(bytes)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_docks_fit_edges_and_center_compact_toolbar() {
        for scale in [1., 1.25, 1.5, 2.] {
            for (x, y, width, height) in [
                (0, 0, 1920, 1080),
                (-1920, -500, 1920, 1080),
                (0, 0, 800, 600),
            ] {
                let monitor = Region {
                    x,
                    y,
                    width,
                    height,
                };
                for (rx, ry, rw, rh) in [
                    (x, y, width, height),
                    (x + 20, y + 20, 180, 140),
                    (x + width as i32 - 220, y + height as i32 - 160, 200, 140),
                    (x + width as i32 / 2, y + height as i32 / 2, 64, 64),
                ] {
                    let region = Region {
                        x: rx,
                        y: ry,
                        width: rw,
                        height: rh,
                    };
                    let hud_scale = fit_hud_scale(region, monitor, scale, 0.85);
                    let hud_above = hud_anchor(region, monitor, scale, hud_scale).1;
                    let ctx = Context {
                        id: "test".into(),
                        kind: "video".into(),
                        width: rw as f64 / scale,
                        height: rh as f64 / scale,
                        scale,
                        frame_color: "#ffffff".into(),
                        hud_scale,
                        hud_above,
                    };
                    for label in ["screen-tools", "screen-hud"] {
                        let full = dock_bounds(&ctx, region, monitor, label, false).unwrap();
                        let small = dock_bounds(&ctx, region, monitor, label, true).unwrap();
                        for dock in [full, small] {
                            assert!(
                                dock.x >= x
                                    && dock.y >= y
                                    && dock.x + dock.width as i32 <= x + width as i32
                                    && dock.y + dock.height as i32 <= y + height as i32,
                                "{label}: {dock:?} / {monitor:?} at {scale}"
                            );
                        }
                        if label == "screen-tools" {
                            let gap = pixels(10., scale) as i32;
                            if small.x + small.width as i32 <= region.x || small.x >= region.x + region.width as i32 {
                                assert_eq!(small.y, fit_start(region.y + (region.height as i32-small.height as i32)/2, monitor.y, monitor.height, small.height, gap));
                            }
                        } else {
                            assert_eq!(full, small);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn hud_stays_centered_below_when_there_is_room() {
        let monitor = Region { x: 0, y: 0, width: 1920, height: 1080 };
        let region = Region { x: 400, y: 200, width: 800, height: 500 };
        let hud = hud_anchor(region, monitor, 1., 0.85).0;
        assert_eq!(hud.x + hud.width as i32 / 2, region.x + region.width as i32 / 2);
        assert!(hud.y > region.y + region.height as i32);
    }
    #[test]
    fn docks_use_free_space_instead_of_covering_left_edge_capture() {
        for scale in [1., 1.25, 1.5, 2.] {
            let monitor = Region { x: -1920, y: -300, width: 1920, height: 1080 };
            let region = Region { x: -1920, y: -300, width: 700, height: 1080 };
            for compact in [false, true] {
                let tools = tool_bounds(region, monitor, scale, compact);
                let hud = hud_anchor(region, monitor, scale, 0.85).0;
                assert_eq!(intersection(tools, region), 0);
                assert_eq!(intersection(hud, region), 0);
                assert_eq!(intersection(hud, tools), 0);
            }
        }
    }
    #[test]
    fn blur_sampling_stays_inside_capture_at_scaled_negative_origin() {
        let area = Region {
            x: -1920,
            y: 0,
            width: 1200,
            height: 900,
        };
        assert_eq!(
            sample_region(
                area,
                1.5,
                Rect {
                    x: 20.,
                    y: 10.,
                    width: 100.,
                    height: 60.
                }
            )
            .unwrap(),
            Region {
                x: -1890,
                y: 15,
                width: 150,
                height: 90
            }
        );
        for x in [-1., f64::NAN, 800.] {
            assert!(sample_region(
                area,
                1.5,
                Rect {
                    x,
                    y: 0.,
                    width: 100.,
                    height: 60.
                }
            )
            .is_err());
        }
    }
}
