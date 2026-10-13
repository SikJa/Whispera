//! ScreenCaptureKit/AVFoundation bridge. Included by screen_capture, not main.
//! Build Capture*.swift into the shared native library, targeting macOS 13.
use crate::screen::Region;
use serde::Serialize;
use std::{
    ffi::{c_char, c_void, CStr, CString},
    io::Write,
    process::ChildStdin,
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub use super::capture_geometry::Display;
type Callback = unsafe extern "C" fn(*mut c_void, i32, *const u8, usize, f64);
unsafe extern "C" {
    fn whispera_capture_start(config: *const c_char, callback: Callback, context: *mut c_void,
        error: *mut c_char, capacity: usize) -> *mut c_void;
    fn whispera_capture_stop(handle: *mut c_void);
    fn whispera_capture_display(window: *mut c_void, x: i32, y: i32, width: u32, height: u32, out: *mut f64) -> i32;
    fn whispera_capture_window(window: *mut c_void, action: i32, x: f64, y: f64, width: f64, height: f64);
}

fn display(window: *mut c_void, region: Region) -> Result<Display, String> {
    let mut out = [0.; 4];
    let result = unsafe { whispera_capture_display(window, region.x, region.y, region.width, region.height, out.as_mut_ptr()) };
    if result == 0 || !out[3].is_finite() || out[3] <= 0. {
        return Err("La pantalla cambio o ya no esta disponible".into());
    }
    Ok(Display { id: out[0] as u32, x: out[1] as i32, y: out[2] as i32, scale: out[3] })
}
pub fn monitor_region(monitor: &tauri::Monitor) -> Result<Region, String> {
    let mut region = Region { x: monitor.position().x, y: monitor.position().y,
        width: monitor.size().width, height: monitor.size().height, ..Default::default() };
    region.display = Some(display(std::ptr::null_mut(), region)?);
    Ok(region)
}
pub fn bind(window: &tauri::WebviewWindow, mut region: Region) -> Result<Region, String> {
    region.display = Some(display(window.ns_window().map_err(|e| e.to_string())?, region)?);
    Ok(region)
}
pub fn window_action(window: &tauri::WebviewWindow, action: i32, rect: Option<crate::screen::Rect>) -> Result<(), String> {
    let native = window.ns_window().map_err(|e| e.to_string())? as usize;
    let rect = rect.unwrap_or(crate::screen::Rect { x: 0., y: 0., width: 0., height: 0. });
    window.run_on_main_thread(move || unsafe {
        whispera_capture_window(native as *mut c_void, action, rect.x, rect.y, rect.width, rect.height);
    }).map_err(|e| e.to_string())
}
pub fn place(window: &tauri::WebviewWindow, region: Region, resize: bool) -> Result<(), String> {
    let display = region.display.ok_or("La ventana no tiene una pantalla asociada")?;
    window_action(window, if resize { 6 } else { 7 }, Some(crate::screen::Rect {
        x: region.x as f64 / display.scale, y: region.y as f64 / display.scale,
        width: region.width as f64 / display.scale, height: region.height as f64 / display.scale,
    }))
}

#[derive(Serialize)]
struct Config {
    mode: &'static str, display: u32, x: f64, y: f64, width: f64, height: f64,
    output_width: u32, output_height: u32, ink: bool, cursor: bool,
}
fn config(region: Region, size: (u32, u32), mode: &'static str, ink: bool) -> Result<Config, String> {
    super::buffer_len(size.0, size.1)?;
    let display = region.display.ok_or("La captura no tiene una pantalla asociada")?;
    let [x, y, width, height] = display.local_rect(region.x, region.y, region.width, region.height)?;
    Ok(Config { mode, display: display.id,
        x, y, width, height,
        output_width: size.0, output_height: size.1, ink, cursor: mode == "video" })
}

type AudioCallback = Box<dyn Fn(Instant, Vec<i16>) + Send + Sync>;
struct Context {
    frame: Arc<Mutex<Option<Arc<Vec<u8>>>>>,
    error: Arc<Mutex<String>>,
    audio: Option<AudioCallback>,
    expected: usize,
}
unsafe extern "C" fn receive(context: *mut c_void, kind: i32, bytes: *const u8, len: usize, age: f64) {
    // Swift serializes callbacks and stop drains that queue before releasing this box.
    if context.is_null() || bytes.is_null() { return; }
    let context = unsafe { &*(context as *const Context) };
    if len > 256 * 1024 * 1024 { return; }
    let data = unsafe { std::slice::from_raw_parts(bytes, len) };
    match kind {
        0 if len == context.expected => {
            if let Ok(mut frame) = context.frame.lock() { *frame = Some(Arc::new(data.to_vec())); }
        }
        1 => {
            if let Some(callback) = &context.audio {
                let values = data.chunks_exact(4).map(|v| {
                    let value = f32::from_ne_bytes(v.try_into().unwrap());
                    if value.is_finite() { (value.clamp(-1., 1.) * 32767.).round() as i16 } else { 0 }
                }).collect();
                let age = if age.is_finite() { age.clamp(0., 60.) } else { 0. };
                let now = Instant::now();
                callback(now.checked_sub(Duration::from_secs_f64(age)).unwrap_or(now), values);
            }
        }
        2 => { if let Ok(mut error) = context.error.lock() { *error = String::from_utf8_lossy(data).into_owned(); } }
        _ => {}
    }
}
struct Native {
    handle: *mut c_void,
    context: Box<Context>,
}
impl Native {
    fn new(config: Config, audio: Option<AudioCallback>) -> Result<Self, String> {
        let mut context = Box::new(Context { frame: Arc::new(Mutex::new(None)), error: Arc::new(Mutex::new(String::new())),
            expected: config.output_width as usize * config.output_height as usize * 4, audio });
        let json = CString::new(serde_json::to_vec(&config).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mut error = [0 as c_char; 2048];
        let handle = unsafe { whispera_capture_start(json.as_ptr(), receive, (&mut *context as *mut Context).cast(), error.as_mut_ptr(), error.len()) };
        if handle.is_null() {
            return Err(unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy().into_owned());
        }
        Ok(Self { handle, context })
    }
    fn error(&self) -> Result<(), String> {
        let error = self.context.error.lock().map_err(|_| "Captura ocupada")?;
        if error.is_empty() { Ok(()) } else { Err(error.clone()) }
    }
    fn first_frame(&self) -> Result<Arc<Vec<u8>>, String> {
        let start = Instant::now();
        loop {
            self.error()?;
            if let Some(frame) = self.context.frame.lock().map_err(|_| "Captura ocupada")?.clone() { return Ok(frame); }
            if start.elapsed() > Duration::from_secs(10) { return Err("ScreenCaptureKit no entrego una imagen a tiempo".into()); }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Native { fn drop(&mut self) { unsafe { whispera_capture_stop(self.handle); } } }

pub fn snapshot(region: Region, ink: bool) -> Result<Vec<u8>, String> {
    let native = Native::new(config(region, (region.width, region.height), "snapshot", ink)?, None)?;
    Ok((*native.first_frame()?).clone())
}

pub struct AudioStream { native: Native }
impl AudioStream {
    pub fn new(system: bool, callback: AudioCallback) -> Result<Self, String> {
        if !system { crate::permissions::ensure_microphone()?; }
        let native = Native::new(Config { mode: if system { "system" } else { "microphone" }, display: 0,
            x: 0., y: 0., width: 0., height: 0., output_width: 2, output_height: 2, ink: false, cursor: false }, Some(callback))?;
        Ok(Self { native })
    }
    pub fn error(&self) -> Result<(), String> { self.native.error() }
}

pub struct Video {
    native: Option<Native>,
    stop: Arc<AtomicBool>,
    writer: Option<JoinHandle<()>>,
    error: Arc<Mutex<String>>,
    pub started: Instant,
}
impl Video {
    pub fn start(region: Region, size: (u32, u32), mut input: ChildStdin) -> Result<Self, String> {
        let native = Native::new(config(region, size, "video", true)?, None)?;
        native.first_frame()?;
        let frames = native.context.frame.clone();
        let error = native.context.error.clone();
        let worker_error = error.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let started = Instant::now();
        let writer = std::thread::spawn(move || {
            let mut count = 0u64;
            while !worker_stop.load(Ordering::Acquire) {
                let frame = frames.lock().ok().and_then(|frame| frame.clone());
                if let Some(frame) = frame {
                    if let Err(e) = input.write_all(&frame) {
                        if let Ok(mut error) = worker_error.lock() { *error = format!("FFmpeg cerro el video: {e}"); }
                        break;
                    }
                    count += 1;
                }
                // Repeat the most recent complete image when the desktop is idle.
                // This keeps video time, audio time and one-second replay segments aligned.
                let deadline = started + Duration::from_secs_f64(count as f64 / 30.);
                if let Some(delay) = deadline.checked_duration_since(Instant::now()) { std::thread::sleep(delay); }
                if started.elapsed().as_secs_f64() - count as f64 / 30. > 2. {
                    if let Ok(mut error) = worker_error.lock() { *error = "El codificador no puede mantener 30 FPS".into(); }
                    break;
                }
            }
            // Closing stdin is the FFmpeg EOF; never write `q` into raw video.
        });
        Ok(Self { native: Some(native), stop, writer: Some(writer), error, started })
    }
    pub fn error(&self) -> Result<(), String> {
        let error = self.error.lock().map_err(|_| "Captura ocupada")?;
        if error.is_empty() { Ok(()) } else { Err(error.clone()) }
    }
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.native.take();
    }
    pub fn join(&mut self) { if let Some(writer) = self.writer.take() { let _ = writer.join(); } }
}
impl Drop for Video {
    fn drop(&mut self) {
        // The owning encoder must be killed/waited before joining a blocked pipe writer.
        self.stop();
        if self.writer.as_ref().is_some_and(|writer| writer.is_finished()) { self.join(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retina_regions_are_display_local_points_not_global_pixels() {
        let region = Region { x: -2800, y: -600, width: 1200, height: 800,
            display: Some(Display { id: 42, x: -2880, y: -800, scale: 2. }) };
        let config = config(region, (600, 400), "video", true).unwrap();
        assert_eq!((config.display, config.x, config.y, config.width, config.height), (42, 40., 100., 600., 400.));
        assert_eq!((config.output_width, config.output_height), (600, 400));
    }
    #[test]
    fn overlapping_mixed_dpi_coordinates_keep_the_selected_display() {
        let a = Region { x: 1600, y: 0, width: 100, height: 100, display: Some(Display { id: 1, x: 0, y: 0, scale: 2. }) };
        let b = Region { display: Some(Display { id: 2, x: 1440, y: 0, scale: 1. }), ..a };
        assert_eq!(config(a, (100, 100), "snapshot", false).unwrap().x, 800.);
        assert_eq!(config(b, (100, 100), "snapshot", false).unwrap().x, 160.);
    }
}
