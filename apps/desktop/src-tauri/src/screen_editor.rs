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
}
#[derive(Default)]
pub struct Editor {
    configured: AtomicBool,
    context: Mutex<Option<Context>>,
    region: Mutex<Option<Region>>,
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
    for label in ["screen-ink", "screen-tools"] {
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
    tauri::WebviewWindowBuilder::new(
        app,
        label,
        tauri::WebviewUrl::App(format!("overlay.html?view={label}").into()),
    )
    .title("Whispera · Herramientas de captura")
    .transparent(true)
    .background_color(tauri::window::Color(0, 0, 0, 0))
    .content_protected(protected)
    .decorations(false)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(false)
    .focused(false)
    .build()
    .map_err(|e| e.to_string())
}
fn screenshot(app: &tauri::AppHandle, region: Region) -> Result<Vec<u8>, String> {
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
    hide(app);
    let bytes = if kind == "image" {
        screenshot(app, region)?
    } else {
        Vec::new()
    };
    let scale = source.scale_factor().map_err(|e| e.to_string())?;
    let ctx = Context {
        id: uuid::Uuid::new_v4().to_string(),
        kind: kind.into(),
        width: rect.width,
        height: rect.height,
        scale,
        frame_color: app
            .state::<crate::storage::Store>()
            .get::<screen::Preferences>("screen_preferences")
            .unwrap_or_default()
            .frame_color,
    };
    let editor = app.state::<Editor>();
    *editor.region.lock().map_err(|_| "Editor ocupado")? = Some(region);
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
    let monitor = source.inner_position().map_err(|e| e.to_string())?;
    let size = source.inner_size().map_err(|e| e.to_string())?;
    let width = (360. * scale).round() as u32;
    let height = (148. * scale).round() as u32;
    let gap = (10. * scale).round() as i32;
    let x = region.x.clamp(
        monitor.x + gap,
        (monitor.x + size.width as i32 - width as i32 - gap).max(monitor.x + gap),
    );
    let below = region.y + region.height as i32 + gap;
    let y = if below + height as i32 <= monitor.y + size.height as i32 - gap {
        below
    } else if region.y - height as i32 - gap >= monitor.y + gap {
        region.y - height as i32 - gap
    } else {
        (monitor.y + size.height as i32 - height as i32 - gap).max(monitor.y)
    };
    tools
        .set_position(tauri::PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    tools
        .set_size(tauri::PhysicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    for w in [&ink, &tools] {
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
    if !["screen-tools", "screen-ink"].contains(&window.label()) {
        return Err("Vista incorrecta".into());
    }
    window.show().map_err(|e| e.to_string())?;
    if window.label() == "screen-ink" {
        if let Some(tools) = app.get_webview_window("screen-tools") {
            if tools.is_visible().unwrap_or(false) {
                tools.set_always_on_top(true).map_err(|e| e.to_string())?;
            }
        }
    }
    if window.label() == "screen-ink" && ctx.kind == "image" {
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(true)
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
    app.emit_to(
        "screen-tools",
        "screen-editor-feedback",
        serde_json::json!({"id":id,"feedback":feedback}),
    )
    .map_err(|e| e.to_string())
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
        let bytes = screenshot(&app, sample_region(area, ctx.scale, rect)?)?;
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
