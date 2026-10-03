use crate::{library, storage::Store};
use base64::Engine;
use serde_json::{json, Value};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
static PREVIEW_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn is_video(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "mp4" | "webm" | "mov" | "mkv" | "avi"
    )
}
fn authorized(app: &tauri::AppHandle, path: &Path) -> Result<(), String> {
    if library::rows(&app.state::<Store>())?
        .iter()
        .any(|i| library::paths(&i["data"]).contains(&path.to_path_buf()))
        && path.is_file()
    {
        Ok(())
    } else {
        Err("Archivo fuera de la biblioteca".into())
    }
}
fn duration(stderr: &str) -> Option<f64> {
    let time = stderr.split("Duration: ").nth(1)?.split(',').next()?;
    let parts: Vec<f64> = time
        .split(':')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    (parts.len() == 3).then(|| parts[0] * 3600. + parts[1] * 60. + parts[2])
}
fn cache_path(app: &tauri::AppHandle, path: &Path) -> Result<PathBuf, String> {
    use std::hash::{Hash, Hasher};
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hash);
    meta.len().hash(&mut hash);
    meta.modified().ok().hash(&mut hash);
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("media-previews");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(format!("{:x}.png", hash.finish())))
}
pub(crate) fn info(app: &tauri::AppHandle, path: &Path) -> Result<Value, String> {
    authorized(app, path)?;
    let _guard = PREVIEW_GATE.lock().map_err(|_| "Miniatura ocupada")?;
    app.asset_protocol_scope()
        .allow_file(path)
        .map_err(|e| e.to_string())?;
    let poster = cache_path(app, path)?;
    let metadata = poster.with_extension("json");
    let result = if poster.exists() && metadata.exists() {
        serde_json::from_slice::<Value>(&fs::read(metadata).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
    } else {
        if !is_video(path) {
            let mut reader = image::ImageReader::open(path).map_err(|e| e.to_string())?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(16384);
            limits.max_image_height = Some(16384);
            limits.max_alloc = Some(160_000_000);
            reader.limits(limits);
            reader
                .decode()
                .map_err(|e| e.to_string())?
                .thumbnail(480, 320)
                .save(&poster)
                .map_err(|e| e.to_string())?;
            let result = json!({"previewPath":poster,"video":false});
            fs::write(
                &metadata,
                serde_json::to_vec(&result).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            app.asset_protocol_scope()
                .allow_file(&poster)
                .map_err(|e| e.to_string())?;
            return Ok(result);
        }
        // FFmpeg is already shipped for recording. Extract one bounded frame, not the whole movie.
        let mut command = Command::new(crate::screen::ffmpeg(app)?);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .args(["-hide_banner", "-nostdin", "-y", "-i"])
            .arg(path)
            .args([
                "-frames:v",
                "1",
                "-vf",
                "scale=480:320:force_original_aspect_ratio=decrease",
                "-threads",
                "1",
            ])
            .arg(&poster)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let started = std::time::Instant::now();
        while child.try_wait().map_err(|e| e.to_string())?.is_none() {
            if started.elapsed().as_secs() >= 10 {
                let _ = child.kill();
                let _ = child.wait();
                return Err("La miniatura del video excedio el tiempo limite".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() || !poster.exists() {
            return Err("No se pudo generar la miniatura del video".into());
        }
        let result = json!({"previewPath":poster,"duration":duration(&String::from_utf8_lossy(&output.stderr)),"video":true});
        fs::write(
            metadata,
            serde_json::to_vec(&result).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        result
    };
    app.asset_protocol_scope()
        .allow_file(&poster)
        .map_err(|e| e.to_string())?;
    Ok(result)
}
#[tauri::command]
pub async fn library_media_info(app: tauri::AppHandle, path: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || info(&app, Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
}
pub(crate) fn drag_image(app: &tauri::AppHandle, path: &Path) -> Result<Vec<u8>, String> {
    let value = info(app, path)?;
    let path = Path::new(value["previewPath"].as_str().ok_or("Sin miniatura")?);
    let image = image::open(path)
        .map_err(|e| e.to_string())?
        .thumbnail(176, 128)
        .to_rgba8();
    let tile = drag_tile(&image);
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(tile)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(bytes.into_inner())
}
fn drag_tile(image: &image::RgbaImage) -> image::RgbaImage {
    let mut tile = image::RgbaImage::new(image.width() + 20, image.height() + 20);
    let radius = 6f64
        .min(image.width() as f64 / 2.)
        .min(image.height() as f64 / 2.);
    let distance = |x: f64, y: f64| {
        let dx = (x - image.width() as f64 / 2.).abs() - (image.width() as f64 / 2. - radius);
        let dy = (y - image.height() as f64 / 2.).abs() - (image.height() as f64 / 2. - radius);
        dx.max(0.).hypot(dy.max(0.)) + dx.max(dy).min(0.) - radius
    };
    for (x, y, pixel) in tile.enumerate_pixels_mut() {
        let shadow = distance(x as f64 - 10., y as f64 - 12.).max(0.);
        pixel.0 = [0, 0, 0, (40. * (-shadow / 2.).exp()) as u8];
    }
    for (x, y, pixel) in image.enumerate_pixels() {
        let mut color = *pixel;
        color.0[3] = (color.0[3] as f64
            * (0.5 - distance(x as f64 + 0.5, y as f64 + 0.5)).clamp(0., 1.))
            as u8;
        tile.put_pixel(x + 10, y + 10, color);
    }
    tile
}
pub(crate) fn open(app: &tauri::AppHandle, id: &str, mode: &str) -> Result<(), String> {
    if mode != "canvas" {
        return Err("Vista invalida".into());
    }
    let item = library::rows(&app.state::<Store>())?
        .into_iter()
        .find(|i| i["id"] == id)
        .ok_or("Elemento inexistente")?;
    let paths = library::paths(&item["data"]);
    // Non-image cards open an empty composition; the renderer offers library images.
    let label = format!("media-{mode}-{}", uuid::Uuid::new_v4());
    let (width, height): (f64, f64) = (1040., 720.);
    for path in paths {
        let _ = app.asset_protocol_scope().allow_file(path);
    }
    let window = WebviewWindowBuilder::new(
        app,
        &label,
        WebviewUrl::App(format!("library/media.html?mode={mode}&id={id}").into()),
    )
    .title("Whispera")
    .inner_size(width, height)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(false)
    .skip_taskbar(true)
    .resizable(false)
    .focused(true)
    .visible(false)
    .build()
    .map_err(|e| e.to_string())?;
    crate::floating_window::apply(&window)?;
    if let Some(monitor) = window.current_monitor().map_err(|e| e.to_string())? {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let width = width.min((area.size.width as f64 / scale - 36.).max(240.));
        let height = height.min((area.size.height as f64 / scale - 36.).max(200.));
        window
            .set_size(tauri::LogicalSize::new(width, height))
            .map_err(|e| e.to_string())?;
        let (x, y) = (
            area.position.x as f64 + (area.size.width as f64 - width * scale) / 2.,
            area.position.y as f64 + (area.size.height as f64 - height * scale) / 2.,
        );
        window
            .set_position(tauri::PhysicalPosition::new(x as i32, y as i32))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
pub async fn library_media_open(
    app: tauri::AppHandle,
    id: String,
    mode: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open(&app, &id, &mode))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn library_media_window(window: tauri::WebviewWindow, action: String) -> Result<(), String> {
    if !window.label().starts_with("media-canvas-") {
        return Err("Ventana invalida".into());
    }
    match action.as_str() {
        "show" => window.show(),
        "close" => window.close(),
        "drag" => window.start_dragging(),
        _ => return Err("Accion invalida".into()),
    }
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn library_media_save(
    app: tauri::AppHandle,
    id: String,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        let item = library::rows(&app.state::<Store>())?
            .into_iter()
            .find(|i| i["id"] == id)
            .ok_or("Elemento inexistente")?;
        let source = library::paths(&item["data"])
            .into_iter()
            .next()
            .ok_or("Archivo inexistente")?;
        authorized(&app, &source)?;
        let picked = app
            .dialog()
            .file()
            .set_file_name(source.file_name().unwrap_or_default().to_string_lossy())
            .blocking_save_file();
        if let Some(picked) = picked {
            let target = picked.into_path().map_err(|e| e.to_string())?;
            if target != source {
                fs::copy(&source, &target).map_err(|e| e.to_string())?;
            }
            return Ok(Some(target.to_string_lossy().into_owned()));
        }
        Ok(None)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn library_media_export(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    png: String,
) -> Result<String, String> {
    if !window.label().starts_with("media-canvas-") {
        return Err("Exportacion solo desde el lienzo".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        if png.len() > 60_000_000 {
            return Err("Imagen demasiado grande".into());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(
                png.strip_prefix("data:image/png;base64,")
                    .ok_or("PNG invalido")?,
            )
            .map_err(|_| "PNG invalido")?;
        let mut reader =
            image::ImageReader::with_format(Cursor::new(&bytes), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(160_000_000);
        reader.limits(limits);
        let image = reader.decode().map_err(|e| e.to_string())?;
        if image.width() as u64 * image.height() as u64 > 40_000_000 {
            return Err("Lienzo demasiado grande".into());
        }
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("compositions");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("Whispera-{}.png", uuid::Uuid::new_v4()));
        fs::write(&path, bytes).map_err(|e| e.to_string())?;
        library::remember_file(&app.state::<Store>(), &path)?;
        use tauri_plugin_clipboard_manager::ClipboardExt;
        app.clipboard()
            .write_image(&tauri::image::Image::new_owned(
                image.to_rgba8().into_raw(),
                image.width(),
                image.height(),
            ))
            .map_err(|e| e.to_string())?;
        library::suppress_clipboard(&app);
        Ok(path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_video_duration() {
        assert_eq!(duration("Duration: 01:02:03.45, start"), Some(3723.45));
        assert_eq!(duration("Duration: N/A, start"), None);
    }
    #[test]
    fn video_extensions_are_case_insensitive() {
        assert!(is_video(Path::new("x.MP4")));
        assert!(!is_video(Path::new("x.png")));
    }
    #[test]
    fn drag_preview_has_round_transparent_corners_and_keeps_content() {
        let image = image::RgbaImage::from_pixel(32, 24, image::Rgba([120, 40, 80, 255]));
        let tile = drag_tile(&image);
        assert_eq!(tile.dimensions(), (52, 44));
        assert_eq!(tile.get_pixel(10, 10).0[3], 0);
        assert_eq!(tile.get_pixel(26, 22).0, [120, 40, 80, 255]);
        assert_eq!(tile.get_pixel(0, 0).0[3], 0);
    }
}
