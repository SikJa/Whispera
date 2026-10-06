//! Separate ink and tool windows: annotations enter the recording; controls do not.
use crate::screen::{self, Rect, Region};
use crate::capture_session as session;
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
use tauri::{Emitter, Manager};
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
    pub rect: Rect,
    pub monitor_width: f64,
    pub monitor_height: f64,
    pub source_label: String,
}
#[derive(Default)]
pub struct Editor {
    configured: AtomicBool,
    dragging: AtomicBool,
    compact: AtomicBool,
    context: Mutex<Option<Context>>,
    region: Mutex<Option<Region>>,
    monitor: Mutex<Option<Region>>,
    image: Mutex<Vec<u8>>,
    feedback: Mutex<serde_json::Value>,
}
impl Editor {
    fn clear(&self) {
        self.configured.store(false,Ordering::SeqCst);
        self.dragging.store(false,Ordering::SeqCst);
        self.compact.store(false,Ordering::SeqCst);
        if let Ok(mut value)=self.context.lock(){*value=None;}
        if let Ok(mut bytes)=self.image.lock(){bytes.clear();}
        if let Ok(mut region)=self.region.lock(){*region=None;}
        if let Ok(mut monitor)=self.monitor.lock(){*monitor=None;}
        if let Ok(mut feedback)=self.feedback.lock(){*feedback=serde_json::json!({});}
    }
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Action {
    pub action: String,
    pub value: Option<String>,
}
fn context(app: &tauri::AppHandle, id: &str) -> Result<Context, String> {
    session::editor(app,id)
        .context
        .lock()
        .map_err(|_| "Editor ocupado")?
        .as_ref()
        .filter(|c| c.id == id)
        .cloned()
        .ok_or_else(|| "La captura ya termino".into())
}
pub fn hide_session(app: &tauri::AppHandle, secondary:bool) {
    let scope=if secondary {"image-session"}else{"screen"};
    for base in ["screen-ink","screen-tools","screen-hud"] {
        if let Some(w)=app.get_webview_window(&session::label(scope,base)) {
            let _=w.hide();let _=session::emit_window(&w,"screen-editor-reset",Option::<Context>::None);
        }
    }
    let editor=session::editor(app,scope);
    editor.clear();
}
pub fn context_for(app:&tauri::AppHandle,scope:&str)->Option<Context> {
    session::editor(app,scope).context.lock().unwrap_or_else(|e|e.into_inner()).clone()
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
        tauri::WebviewUrl::App(format!("overlay.html?view={}",label.replacen("image-","screen-",1)).into()),
    )
    .title(match label.replacen("image-","screen-",1).as_str() {
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
    let builder = if protected && !label.ends_with("-ink") {
        builder
            .parent(
                &app.get_webview_window(&session::label(label,"screen-ink"))
                    .ok_or("Editor no disponible")?,
            )
            .map_err(|e| e.to_string())?
    } else {
        builder
    };
    let window = builder.build().map_err(|e| e.to_string())?;
    if label.ends_with("-ink") {
        let app = app.clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Focused(true)) {
                raise_controls(&app);
            }
        });
    }
    Ok(window)
}
pub(crate) fn raise_controls(app: &tauri::AppHandle) {
    let mut windows=Vec::new();
    let image_freeze_visible=app.webview_windows().iter().any(|(label,window)|label.starts_with("image-freeze-")&&window.is_visible().unwrap_or(false));
    for scope in ["screen","image-session"] {
        // Keep the ongoing video's input surfaces below the still-image freeze.
        if !session::secondary(scope)&&image_freeze_visible {continue;}
        // The still editor must also stay above the primary video's controls.
        {
            if let Some(ink)=app.get_webview_window(&session::label(scope,"screen-ink")) {
                if ink.is_visible().unwrap_or(false){windows.push(ink);}
            }
        }
        for (label,source) in app.webview_windows() {
            if label.starts_with(if session::secondary(scope){"image-select-"}else{"screen-select-"})&&source.is_visible().unwrap_or(false){windows.push(source);}
        }
        for base in ["screen-tools","screen-hud"] {
            if let Some(window)=app.get_webview_window(&session::label(scope,base)) {
                if window.is_visible().unwrap_or(false){windows.push(window);}
            }
        }
    }
    let app_copy=app.clone();
    let _=app.run_on_main_thread(move||{
        for window in windows {
            // Tao skips set_always_on_top(true) when the flag is already true.
            // Reorder the actual HWND so the ink cannot cover the white border.
            let result=window.hwnd().map_err(|e|e.to_string()).and_then(|hwnd|raise_native(windows::Win32::Foundation::HWND(hwnd.0)));
            if let Err(error)=result {let _=app_copy.state::<crate::storage::Store>().event(&format!("No se pudo mostrar el borde de captura: {error}"));}
        }
    });
}
pub(crate) fn raise_native(hwnd:windows::Win32::Foundation::HWND)->Result<(),String> {
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos,HWND_TOPMOST,SWP_NOMOVE,SWP_NOSIZE,SWP_NOACTIVATE,SWP_NOOWNERZORDER};
    unsafe {SetWindowPos(hwnd,Some(HWND_TOPMOST),0,0,0,0,SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE|SWP_NOOWNERZORDER).map_err(|e|e.to_string())}
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
fn tool_bounds(region: Region, monitor: Region, scale: f64, compact: bool) -> Region {
    let gap = pixels(10., scale) as i32;
    let width = pixels(50., scale).min(monitor.width.saturating_sub(2 * gap as u32));
    let full_height = pixels(450., scale).min(monitor.height.saturating_sub(2 * gap as u32));
    let height = if compact { pixels(96., scale).min(full_height) } else { full_height };
    let left = region.x - width as i32 - gap;
    let x = if left >= monitor.x + gap {
        left
    } else {
        region.x + region.width as i32 - width as i32 - gap
    };
    let mut x = fit_start(x, monitor.x, monitor.width, width, gap);
    let hud_width = pixels(300., scale);
    let hud_height = pixels(56., scale);
    let left_room = x - monitor.x - gap;
    let right_room = monitor.x + monitor.width as i32 - gap - x - width as i32;
    if left_room < hud_width as i32 + gap && right_room < hud_width as i32 + gap
        && full_height + hud_height + 3 * gap as u32 > monitor.height
        && width + hud_width + 3 * gap as u32 <= monitor.width
    {
        // At large Windows scaling there may be no room beside a centered rail.
        // Dock it to the nearer monitor edge so the recording bar stays usable.
        x = if left_room <= right_room { monitor.x + gap }
            else { monitor.x + monitor.width as i32 - width as i32 - gap };
    }
    Region {
        x,
        y: fit_start(region.y + (region.height as i32-height as i32)/2, monitor.y, monitor.height, height, gap),
        width,
        height,
    }
}
fn hud_anchor(region: Region, monitor: Region, scale: f64, _hud_scale: f64) -> (Region, bool) {
    let gap = pixels(10., scale) as i32;
    let width = pixels(300., scale)
        .min(monitor.width.saturating_sub(2 * gap as u32));
    let height = pixels(56., scale);
    let below = region.y + region.height as i32 + gap;
    let y = if below + height as i32 + gap <= monitor.y + monitor.height as i32 {
        below
    } else if region.y - height as i32 - gap >= monitor.y + gap {
        region.y - height as i32 - gap
    } else {
        region.y + region.height as i32 - height as i32 - gap
    };
    let x = region.x + (region.width as i32 - width as i32) / 2;
    let mut anchor = Region {
        x: fit_start(x, monitor.x, monitor.width, width, gap),
        y: fit_start(y, monitor.y, monitor.height, height, gap),
        width,
        height,
    };
    // The wide recording bar can cross the drawing rail for a narrow selection.
    // Keep each dock visible and reserve a gap even near monitor edges.
    let tools = tool_bounds(region, monitor, scale, false);
    let overlaps = |dock: Region| {
        dock.x < tools.x + tools.width as i32 + gap
            && dock.x + dock.width as i32 + gap > tools.x
            && dock.y < tools.y + tools.height as i32 + gap
            && dock.y + dock.height as i32 + gap > tools.y
    };
    if overlaps(anchor) {
        for (candidate_x, candidate_y) in [
            (tools.x + tools.width as i32 + gap, anchor.y),
            (tools.x - width as i32 - gap, anchor.y),
            (anchor.x, tools.y - height as i32 - gap),
            (anchor.x, tools.y + tools.height as i32 + gap),
        ] {
            let candidate = Region {
                x: fit_start(candidate_x, monitor.x, monitor.width, width, gap),
                y: fit_start(candidate_y, monitor.y, monitor.height, height, gap),
                ..anchor
            };
            if !overlaps(candidate) {
                anchor = candidate;
                break;
            }
        }
    }
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
    if label.ends_with("-tools") {
        return Ok(tool_bounds(region, monitor, ctx.scale, compact));
    }
    if !label.ends_with("-hud") {
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
pub(crate) fn drag_controls(app:&tauri::AppHandle,source:&tauri::WebviewWindow,active:bool)->Result<(),String> {
    let ctx=context_for(app,source.label()).ok_or("Captura no disponible")?;
    let editor=session::editor(app,source.label());
    editor.dragging.store(active,Ordering::SeqCst);
    let region=editor.region.lock().map_err(|_|"Editor ocupado")?.ok_or("Captura no disponible")?;
    let monitor=editor.monitor.lock().map_err(|_|"Editor ocupado")?.ok_or("Monitor no disponible")?;
    if active {
        if let Some(tools)=app.get_webview_window(&session::label(source.label(),"screen-tools")) {
            session::emit_window(&tools,"screen-editor-drag",&ctx.id).map_err(|e|e.to_string())?;
            place_controls(&tools,&ctx,region,monitor,editor.compact.load(Ordering::SeqCst))?;
        }
    }
    // Restore the committed docks on cancellation/failure, or finish at the new region.
    if !active { preview_controls(app,source,region)?; }
    Ok(())
}
pub(crate) fn preview_controls(app:&tauri::AppHandle,source:&tauri::WebviewWindow,region:Region)->Result<(),String> {
    let ctx=context_for(app,source.label()).ok_or("Captura no disponible")?;
    let editor=session::editor(app,source.label());
    let monitor=editor.monitor.lock().map_err(|_|"Editor ocupado")?.ok_or("Monitor no disponible")?;
    let mut positions=Vec::new();
    for base in ["screen-tools","screen-hud"] {
        if let Some(window)=app.get_webview_window(&session::label(source.label(),base)) {
            let dock=dock_bounds(&ctx,region,monitor,window.label(),editor.compact.load(Ordering::SeqCst))?;
            positions.push((window,dock.x,dock.y));
        }
    }
    let app_copy=app.clone();let scope=source.label().to_string();let id=ctx.id;
    app.run_on_main_thread(move||{
        if context_for(&app_copy,&scope).is_none_or(|c|c.id!=id){return;}
        let native=positions.iter().map(|(window,x,y)|window.hwnd().map(|h|(windows::Win32::Foundation::HWND(h.0),*x,*y))).collect::<Result<Vec<_>,_>>();
        let result=native.map_err(|e|e.to_string()).and_then(|windows|move_native_controls(&windows));
        if let Err(error)=result {let _=app_copy.state::<crate::storage::Store>().event(&format!("No se pudieron mover las herramientas de captura: {error}"));}
    }).map_err(|e|e.to_string())
}
fn move_native_controls(windows:&[(windows::Win32::Foundation::HWND,i32,i32)])->Result<(),String> {
    use windows::Win32::UI::WindowsAndMessaging::{BeginDeferWindowPos,DeferWindowPos,EndDeferWindowPos,SWP_NOSIZE,SWP_NOZORDER,SWP_NOACTIVATE,SWP_NOOWNERZORDER};
    if windows.is_empty(){return Ok(());}
    unsafe {
        let mut batch=BeginDeferWindowPos(windows.len() as i32).map_err(|e|e.to_string())?;
        for (window,x,y) in windows {
            batch=DeferWindowPos(batch,*window,None,*x,*y,0,0,SWP_NOSIZE|SWP_NOZORDER|SWP_NOACTIVATE|SWP_NOOWNERZORDER).map_err(|e|e.to_string())?;
        }
        EndDeferWindowPos(batch).map_err(|e|e.to_string())
    }
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
        *session::editor(app,source.label())
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
    hide_session(app,session::secondary(source.label()));
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
        id: format!("{}{}",if session::secondary(source.label()) {"image-"}else{""},uuid::Uuid::new_v4()),
        kind: kind.into(),
        width: rect.width,
        height: rect.height,
        scale,
        hud_scale,
        hud_above,
        rect,
        monitor_width: size.width as f64 / scale,
        monitor_height: size.height as f64 / scale,
        source_label: source.label().into(),
        frame_color: app
            .state::<crate::storage::Store>()
            .get::<screen::Preferences>("screen_preferences")
            .unwrap_or_default()
            .frame_color,
    };
    let editor = session::editor(app,source.label());
    *editor.region.lock().map_err(|_| "Editor ocupado")? = Some(region);
    *editor.monitor.lock().map_err(|_| "Editor ocupado")? = Some(monitor);
    *editor.image.lock().map_err(|_| "Editor ocupado")? = bytes;
    *editor.context.lock().map_err(|_| "Editor ocupado")? = Some(ctx.clone());
    *editor.feedback.lock().map_err(|_| "Editor ocupado")? = serde_json::json!({});
    let ink = window(app, &session::label(source.label(),"screen-ink"), session::secondary(source.label()))?;
    ink.set_position(tauri::PhysicalPosition::new(region.x, region.y))
        .map_err(|e| e.to_string())?;
    ink.set_size(tauri::PhysicalSize::new(region.width, region.height))
        .map_err(|e| e.to_string())?;
    ink.set_ignore_cursor_events(kind == "video")
        .map_err(|e| e.to_string())?;
    ink.set_focusable(kind != "video")
        .map_err(|e| e.to_string())?;
    if session::secondary(source.label()){screen::protect(&ink)?;}
    let tools = window(app, &session::label(source.label(),"screen-tools"), true)?;
    screen::protect(&tools)?;
    place_controls(&tools, &ctx, region, monitor, false)?;
    let hud = window(app, &session::label(source.label(),"screen-hud"), true)?;
    screen::protect(&hud)?;
    place_controls(&hud, &ctx, region, monitor, false)?;
    for w in [&ink, &tools, &hud] {
        session::emit_window(&w,"screen-editor-reset", &ctx)
            .map_err(|e| e.to_string())?;
    }
    editor.configured.store(true, Ordering::SeqCst);
    source.set_ignore_cursor_events(false).map_err(|e|e.to_string())?;
    screen::set_frame_region(source,Some(rect))?;
    session::emit_window(source,"screen-editor-reset",&ctx).map_err(|e|e.to_string())?;
    Ok(())
}
pub(crate) fn resize(app:&tauri::AppHandle,source:&tauri::WebviewWindow,id:&str,rect:Rect,region:Region,bytes:Option<Vec<u8>>) -> Result<(),String> {
    let mut ctx=context(app,id)?;let editor=session::editor(app,source.label());
    let monitor=editor.monitor.lock().map_err(|_|"Editor ocupado")?.ok_or("Pantalla no disponible")?;
    ctx.rect=rect;ctx.width=rect.width;ctx.height=rect.height;
    ctx.hud_scale=fit_hud_scale(region,monitor,ctx.scale,app.state::<crate::storage::Store>().get::<crate::storage::Settings>("settings")?.recorder_scale);
    ctx.hud_above=hud_anchor(region,monitor,ctx.scale,ctx.hud_scale).1;
    if let Some(bytes)=bytes{*editor.image.lock().map_err(|_|"Editor ocupado")?=bytes;}
    *editor.region.lock().map_err(|_|"Editor ocupado")?=Some(region);
    *editor.context.lock().map_err(|_|"Editor ocupado")?=Some(ctx.clone());
    let ink=app.get_webview_window(&session::label(source.label(),"screen-ink")).ok_or("Editor no disponible")?;
    ink.set_position(tauri::PhysicalPosition::new(region.x,region.y)).map_err(|e|e.to_string())?;
    ink.set_size(tauri::PhysicalSize::new(region.width,region.height)).map_err(|e|e.to_string())?;
    for label in ["screen-tools","screen-hud"] {
        if let Some(window)=app.get_webview_window(&session::label(source.label(),label)){place_controls(&window,&ctx,region,monitor,editor.compact.load(Ordering::SeqCst))?;}
    }
    // The image border restores its hit-test region after the updated DOM frame.
    // Video reframing may wait for an encoder, so restore its region immediately.
    if ctx.kind=="video" {screen::set_frame_region(source,Some(rect))?;}
    for window in [Some(ink),app.get_webview_window(&session::label(source.label(),"screen-tools")),app.get_webview_window(&session::label(source.label(),"screen-hud")),Some(source.clone())].into_iter().flatten(){
        session::emit_window(&window,"screen-editor-reset",&ctx).map_err(|e|e.to_string())?;
    }
    raise_controls(app);
    Ok(())
}
#[tauri::command]
pub fn screen_editor_context(app:tauri::AppHandle,window:tauri::WebviewWindow) -> Option<Context> {
    context_for(&app,window.label())
}
#[tauri::command]
pub fn screen_editor_image(app: tauri::AppHandle, id: String) -> Result<tauri::ipc::Response, String> {
    if context(&app, &id)?.kind != "image" {
        return Err("No hay una imagen activa".into());
    }
    Ok(tauri::ipc::Response::new(session::editor(&app,&id)
        .image
        .lock()
        .map_err(|_| "Editor ocupado")?
        .clone()))
}
#[tauri::command]
pub fn screen_editor_ready(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
) -> Result<bool, String> {
    let ctx = context(&app, &id)?;
    if !session::editor(&app,&id).configured.load(Ordering::SeqCst) {
        return Ok(false);
    }
    if !["screen-tools", "screen-ink", "screen-hud"].iter().any(|base|session::label(&id,base)==window.label()) {
        return Err("Vista incorrecta".into());
    }
    window.show().map_err(|e| e.to_string())?;
    if window.label() == session::label(&id,"screen-ink") && ctx.kind == "image" {
        window.set_focus().map_err(|e| e.to_string())?;
    }
    // Focusing the ink window can raise it above other topmost windows.
    // Restore the editable border afterwards so it never disappears behind the image.
    raise_controls(&app);
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
    let editor = session::editor(&app,&id);
    if window.label()==session::label(&id,"screen-tools") {
        if editor.dragging.load(Ordering::SeqCst){return Ok(());}
        editor.compact.store(compact,Ordering::SeqCst);
    }
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
    if window.label() != session::label(&id,"screen-tools") { return Err("Vista incorrecta".into()); }
    let ctx=context(&app,&id)?;
    let editor=session::editor(&app,&id);
    if editor.dragging.load(Ordering::SeqCst){return Ok(serde_json::json!({"railX":4,"railY":4,"menuX":0,"menuY":0}));}
    editor.compact.store(compact,Ordering::SeqCst);
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
        if let Some(w) = app.get_webview_window(&session::label(&id,"screen-ink")) {
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
        session::label(&id,"screen-ink"),
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
    *session::editor(&app,&id)
        .feedback
        .lock()
        .map_err(|_| "Editor ocupado")? = feedback.clone();
    for label in ["screen-tools", "screen-hud"] {
        app.emit_to(
            session::label(&id,label),
            "screen-editor-feedback",
            serde_json::json!({"id":id,"feedback":feedback}),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
pub fn screen_editor_feedback_get(app:tauri::AppHandle,window:tauri::WebviewWindow) -> serde_json::Value {
    session::editor(&app,window.label()).feedback.lock().unwrap_or_else(|e|e.into_inner()).clone()
}
#[tauri::command]
pub fn screen_editor_print(
    app: tauri::AppHandle,
    id: String,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    if context(&app, &id)?.kind != "image" || window.label() != session::label(&id,"screen-ink") {
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
        if ctx.kind != "image" || window.label() != session::label(&id,"screen-ink") || bytes.len() > 64 * 1024 * 1024 {
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
                context(&app,&id)?;
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
        screen::finish_image(app.clone(),&id)?;
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
        let region = session::editor(&app,&id).region.lock().map_err(|_| "Editor ocupado")?
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
        let area = session::editor(&app,&id)
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
    #[test]
    fn video_bar_does_not_cover_drawing_tools_for_narrow_regions() {
        for scale in [1., 1.25, 1.5, 2.] {
            let monitor = Region { x: -800, y: -100, width: 800, height: 600 };
            for (x,y) in [(-790,-90),(-410,170),(-100,400)] {
                let region = Region { x, y, width: 64, height: 64 };
                let hud = hud_anchor(region, monitor, scale, 0.85).0;
                let tools = tool_bounds(region, monitor, scale, false);
                assert!(hud.x + hud.width as i32 <= tools.x
                    || tools.x + tools.width as i32 <= hud.x
                    || hud.y + hud.height as i32 <= tools.y
                    || tools.y + tools.height as i32 <= hud.y,
                    "HUD {hud:?} covers tools {tools:?} at {scale}");
            }
        }
    }
    use super::*;
    #[test]
    fn live_controls_move_together_without_resizing_showing_or_taking_focus() {
        use windows::{core::w,Win32::{Foundation::HWND,UI::WindowsAndMessaging::{CreateWindowExW,DestroyWindow,GetForegroundWindow,GetWindowRect,IsWindowVisible,WS_POPUP,WS_EX_TOOLWINDOW}}};
        struct Owned(HWND);impl Drop for Owned{fn drop(&mut self){unsafe{let _=DestroyWindow(self.0);}}}
        unsafe {
            let tools=Owned(CreateWindowExW(WS_EX_TOOLWINDOW,w!("STATIC"),w!("Whispera tools movement test"),WS_POPUP,10,20,50,450,None,None,None,None).unwrap());
            let hud=Owned(CreateWindowExW(WS_EX_TOOLWINDOW,w!("STATIC"),w!("Whispera HUD movement test"),WS_POPUP,100,300,260,56,None,None,None,None).unwrap());
            let foreground=GetForegroundWindow();
            for (tx,ty,hx,hy) in [(150,120,210,600),(-1800,-400,-1700,-200),(30,40,80,90)] {
                move_native_controls(&[(tools.0,tx,ty),(hud.0,hx,hy)]).unwrap();
                for (window,x,y,width,height) in [(tools.0,tx,ty,50,450),(hud.0,hx,hy,260,56)] {
                    let mut rect=Default::default();GetWindowRect(window,&mut rect).unwrap();
                    assert_eq!((rect.left,rect.top,rect.right-rect.left,rect.bottom-rect.top),(x,y,width,height));
                    assert!(!IsWindowVisible(window).as_bool());
                }
                assert_eq!(GetForegroundWindow(),foreground);
            }
        }
    }
    #[test]
    fn closing_or_resizing_still_image_preserves_the_video_editor() {
        let video=Editor::default();let image=Editor::default();
        let video_region=Region{x:40,y:50,width:320,height:200};
        *video.region.lock().unwrap()=Some(video_region);
        *video.feedback.lock().unwrap()=serde_json::json!({"tool":"pen","count":3});
        video.configured.store(true,Ordering::SeqCst);
        *image.region.lock().unwrap()=Some(Region{x:100,y:80,width:180,height:120});
        *image.image.lock().unwrap()=vec![1,2,3];
        *image.region.lock().unwrap()=Some(Region{x:-500,y:150,width:280,height:170});
        image.clear();
        assert_eq!(*video.region.lock().unwrap(),Some(video_region));
        assert_eq!(*video.feedback.lock().unwrap(),serde_json::json!({"tool":"pen","count":3}));
        assert!(video.configured.load(Ordering::SeqCst));
        assert!(image.region.lock().unwrap().is_none());assert!(image.image.lock().unwrap().is_empty());
    }
    #[test]
    fn native_border_is_raised_even_when_both_windows_are_already_topmost() {
        use windows::{core::w,Win32::{Foundation::HWND,UI::WindowsAndMessaging::{CreateWindowExW,DestroyWindow,GetForegroundWindow,GetWindow,GetWindowRect,GW_HWNDPREV,WS_POPUP,WS_EX_TOOLWINDOW}}};
        struct Owned(HWND);impl Drop for Owned{fn drop(&mut self){unsafe{let _=DestroyWindow(self.0);}}}
        unsafe {
            // Hidden windows test the native ordering without drawing on the user's desktop.
            let frame=Owned(CreateWindowExW(WS_EX_TOOLWINDOW,w!("STATIC"),w!("Whispera frame test"),WS_POPUP,10,20,100,80,None,None,None,None).unwrap());
            let ink=Owned(CreateWindowExW(WS_EX_TOOLWINDOW,w!("STATIC"),w!("Whispera ink test"),WS_POPUP,10,20,100,80,None,None,None,None).unwrap());
            fn above(first:HWND,second:HWND)->bool {
                let mut cursor=second;
                for _ in 0..4096 {match unsafe{GetWindow(cursor,GW_HWNDPREV)}{Ok(next) if !next.0.is_null()=>{if next==first{return true;}cursor=next;},_=>break}}
                false
            }
            let foreground=GetForegroundWindow();let mut original=Default::default();GetWindowRect(frame.0,&mut original).unwrap();
            raise_native(frame.0).unwrap();raise_native(ink.0).unwrap();assert!(above(ink.0,frame.0));
            // Repeated focus/resize cycles must keep the same frame HWND above the ink.
            for _ in 0..4 {raise_native(frame.0).unwrap();assert!(above(frame.0,ink.0));raise_native(ink.0).unwrap();}
            raise_native(frame.0).unwrap();assert!(above(frame.0,ink.0));
            let mut current=Default::default();GetWindowRect(frame.0,&mut current).unwrap();
            assert_eq!((original.left,original.top,original.right,original.bottom),(current.left,current.top,current.right,current.bottom));
            assert_eq!(GetForegroundWindow(),foreground);
        }
    }
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
                        rect:Rect{x:(rx-x) as f64/scale,y:(ry-y) as f64/scale,width:rw as f64/scale,height:rh as f64/scale},
                        monitor_width:width as f64/scale,
                        monitor_height:height as f64/scale,
                        source_label:"screen-select-0".into(),
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
                        assert_eq!(full.x, small.x);
                        if label.ends_with("-tools") {
                            let gap = pixels(10., scale) as i32;
                            assert_eq!(small.y, fit_start(region.y + (region.height as i32-small.height as i32)/2, monitor.y, monitor.height, small.height, gap));
                        } else {
                            assert_eq!(full, small);
                            let tools = tool_bounds(region, monitor, scale, false);
                            assert!(small.x + small.width as i32 <= tools.x
                                || tools.x + tools.width as i32 <= small.x
                                || small.y + small.height as i32 <= tools.y
                                || tools.y + tools.height as i32 <= small.y,
                                "HUD {small:?} overlaps drawing rail {tools:?}");
                        }
                    }
                }
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
