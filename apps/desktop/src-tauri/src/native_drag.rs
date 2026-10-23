//! Windows isolates OLE in a helper. macOS starts a real AppKit drag on the
//! source NSView, then waits for its completion only on a background worker.
#[cfg(windows)]
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::atomic::{AtomicBool, Ordering}, time::{Duration, Instant}};
#[cfg(windows)]
use std::{io::{Read, Write}, process::{Command, Stdio}};
#[cfg(windows)]
use windows::Win32::{Foundation::HWND, UI::{Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON}, WindowsAndMessaging::{IsWindow, IsWindowVisible}}};

#[cfg(windows)]
const HELPER: &str = "--whispera-file-drag";
#[cfg(windows)]
const RELEASE_GRACE: Duration = Duration::from_millis(1500);
const MAX_DRAG: Duration = Duration::from_secs(60);
static ACTIVE: AtomicBool = AtomicBool::new(false);

pub struct Permit;
impl Drop for Permit { fn drop(&mut self) { ACTIVE.store(false, Ordering::SeqCst); } }
pub fn reserve() -> Result<Permit, String> {
    ACTIVE.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map(|_| Permit).map_err(|_| "Ya hay un arrastre en curso".into())
}
#[cfg(windows)]
pub fn button_down() -> bool { unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) < 0 } }
#[cfg(windows)]
pub fn release_source_capture(window:&tauri::WebviewWindow)->Result<(),String> {
    let source=window.clone();let (send,receive)=std::sync::mpsc::channel();
    window.run_on_main_thread(move||{
        use windows::Win32::UI::{Input::KeyboardAndMouse::{GetCapture,ReleaseCapture},WindowsAndMessaging::IsChild};
        let result=(||{
            let owner=HWND(source.hwnd().map_err(|e|e.to_string())?.0);
            let capture=unsafe{GetCapture()};
            if capture==owner||unsafe{IsChild(owner,capture).as_bool()} {
                unsafe{ReleaseCapture()}.map_err(|e|e.to_string())?;
            }
            Ok(())
        })();let _=send.send(result);
    }).map_err(|e|e.to_string())?;
    receive.recv_timeout(Duration::from_secs(5)).map_err(|_|"No respondió el inicio del arrastre".to_string())?
}
#[cfg(target_os = "macos")]
pub fn button_down() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" { fn CGEventSourceButtonState(state: i32, button: u32) -> bool; }
    // kCGEventSourceStateCombinedSessionState, kCGMouseButtonLeft.
    unsafe { CGEventSourceButtonState(0, 0) }
}
#[cfg(windows)]
fn source_visible(source: i64) -> bool {
    let hwnd = HWND(source as usize as *mut _);
    unsafe { IsWindow(Some(hwnd)).as_bool() && IsWindowVisible(hwnd).as_bool() }
}

#[cfg(windows)]
#[derive(Serialize, Deserialize)]
struct Request { source: i64, files: Vec<PathBuf>, preview: Vec<u8> }

#[cfg(windows)]
#[derive(Default)]
struct Deadline { ended: Option<Instant> }
#[cfg(windows)]
impl Deadline {
    fn expired(&mut self, now: Instant, started: Instant, held: bool, visible: bool, escape: bool) -> bool {
        if !held || !visible || escape { self.ended.get_or_insert(now); }
        now.duration_since(started) >= MAX_DRAG || self.ended.is_some_and(|at| now.duration_since(at) >= RELEASE_GRACE)
    }
}

#[cfg(windows)]
pub fn run(source: i64, files: Vec<PathBuf>, preview: Vec<u8>, _permit: Permit) -> Result<(), String> {
    // Preparation can outlast the gesture. Never enter OLE after the user let go.
    if !button_down() || !source_visible(source) {
        return Err("El arrastre se interrumpio durante la preparacion. Volve a arrastrar manteniendo pulsado el boton del mouse.".into());
    }
    if files.is_empty() || files.len() > 20 || preview.len() > 1_000_000 { return Err("Arrastre invalido".into()); }
    let input = serde_json::to_vec(&Request { source, files, preview }).map_err(|e| e.to_string())?;
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    use std::os::windows::process::CommandExt;
    command.arg(HELPER).creation_flags(0x08000000).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());
    // The job also kills this one helper if the app exits or crashes.
    let mut helper = crate::encoder_job::Encoder::spawn(&mut command)?;
    helper.child.stdin.take().ok_or("No se pudo iniciar el arrastre")?
        .write_all(&input).map_err(|e| e.to_string())?;
    supervise(&mut helper, || (button_down(), source_visible(source), unsafe { GetAsyncKeyState(VK_ESCAPE.0 as i32) < 0 }))
}

#[cfg(windows)]
fn supervise(helper: &mut crate::encoder_job::Encoder, mut input: impl FnMut() -> (bool, bool, bool)) -> Result<(), String> {
    let started = Instant::now();
    let mut deadline = Deadline::default();
    loop {
        if let Some(status) = helper.child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() { Ok(()) } else { Err("No se pudo completar el arrastre. Los archivos siguen guardados.".into()) };
        }
        let (held, visible, escape) = input();
        if deadline.expired(Instant::now(), started, held, visible, escape) {
            // A timeout on an IPC future cannot unwind DoDragDrop. Terminate only
            // the isolated helper, and reap it before releasing the drag permit.
            helper.child.kill().map_err(|e| e.to_string())?;
            helper.child.wait().map_err(|e| e.to_string())?;
            return Err("Se cancelo un arrastre sin respuesta. Podes volver a intentarlo; los archivos siguen guardados.".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(windows)]
pub fn helper_entry() -> Option<i32> {
    if std::env::args().nth(1).as_deref() != Some(HELPER) { return None; }
    Some(if helper().is_ok() { 0 } else { 1 })
}

#[cfg(windows)]
fn helper() -> Result<(), String> {
    use raw_window_handle::{HandleError, HasWindowHandle, Win32WindowHandle, WindowHandle};
    use std::num::NonZeroIsize;
    use windows::{core::w, Win32::{Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM}, System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::{CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP}}};
    unsafe extern "system" fn wndproc(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT { DefWindowProcW(hwnd, message, wp, lp) }
    struct Window(HWND);
    impl HasWindowHandle for Window {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let raw = Win32WindowHandle::new(NonZeroIsize::new(self.0.0 as isize).ok_or(HandleError::Unavailable)?);
            Ok(unsafe { WindowHandle::borrow_raw(raw.into()) })
        }
    }
    impl Drop for Window { fn drop(&mut self) { unsafe { let _ = DestroyWindow(self.0); } } }
    let request: Request = serde_json::from_reader(std::io::stdin().take(2_000_000)).map_err(|e| e.to_string())?;
    if request.files.is_empty() || request.files.len() > 20 || request.preview.len() > 1_000_000 || request.files.iter().any(|f| !f.is_file()) {
        return Err("Archivos invalidos".into());
    }
    if !button_down() || !source_visible(request.source) { return Ok(()); }
    let instance = unsafe { GetModuleHandleW(None) }.map_err(|e| e.to_string())?;
    let class = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: HINSTANCE(instance.0), lpszClassName: w!("WhisperaFileDrag"), ..Default::default() };
    unsafe { RegisterClassW(&class); }
    let window = Window(unsafe { CreateWindowExW(WS_EX_TOOLWINDOW, w!("WhisperaFileDrag"), w!("Whispera · Arrastre"), WS_POPUP, 0, 0, 1, 1, None, None, Some(HINSTANCE(instance.0)), None) }.map_err(|e| e.to_string())?);
    if !button_down() { return Ok(()); }
    drag::start_drag(&window, drag::DragItem::Files(request.files), drag::Image::Raw(request.preview), |_, _| {}, Default::default()).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
pub fn helper_entry() -> Option<i32> { None }

#[cfg(target_os = "macos")]
pub fn run(window: tauri::WebviewWindow, files: Vec<PathBuf>, preview: Vec<u8>, permit: Permit) -> Result<(), String> {
    extern "C" { fn pthread_main_np() -> i32; }
    if unsafe { pthread_main_np() } != 0 {
        return Err("El arrastre debe prepararse fuera del hilo principal".into());
    }
    if files.is_empty() || files.len() > 20 || files.iter().any(|f| !f.is_file()) || preview.len() > 1_000_000 {
        return Err("Archivos invalidos para arrastrar".into());
    }
    // drag 2.1.1 expects NSImage creation to succeed; reject a bad preview first.
    image::load_from_memory(&preview).map_err(|_| "Vista previa de arrastre invalida")?;
    let (done, result) = std::sync::mpsc::channel();
    let started = Instant::now();
    let dispatcher = window.clone();
    dispatcher.run_on_main_thread(move || {
        let start = (|| -> Result<(), String> {
            if started.elapsed() >= MAX_DRAG { return Err("Se agoto el tiempo para iniciar el arrastre".into()); }
            if !button_down() || !window.is_visible().map_err(|e| e.to_string())? {
                let _ = done.send(Ok(()));
                return Ok(());
            }
            // Tauri's HasWindowHandle yields this window's AppKit NSView. Never
            // substitute the Windows helper or an HTML-only dataTransfer.
            let completed = done.clone();
            let active = std::sync::Mutex::new(Some(permit));
            drag::start_drag(&window, drag::DragItem::Files(files), drag::Image::Raw(preview), move |_, _| {
                if let Ok(mut active) = active.lock() { drop(active.take()); }
                let _ = completed.send(Ok(()));
            }, Default::default()).map_err(|e| e.to_string())
        })();
        if let Err(error) = start { let _ = done.send(Err(error)); }
    }).map_err(|e| e.to_string())?;
    // AppKit owns cancellation and the native session. On timeout the callback
    // still owns the permit, preventing a second drag until that session ends.
    result.recv_timeout(MAX_DRAG).map_err(|_| "El arrastre no respondio. Los archivos siguen guardados.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    fn release_cancels_even_when_ole_never_calls_back() {
        let start = Instant::now(); let mut limit = Deadline::default();
        assert!(!limit.expired(start, start, true, true, false));
        let up = start + Duration::from_millis(100);
        assert!(!limit.expired(up, start, false, true, false));
        // Another click cannot revive the previous gesture.
        assert!(limit.expired(up + RELEASE_GRACE, start, true, true, false));
    }
    #[test]
    #[cfg(windows)]
    fn escape_hidden_source_and_absolute_limit_end_the_helper() {
        for (held, visible, escape) in [(false,true,false),(true,false,false),(true,true,true)] {
            let start = Instant::now();let mut limit = Deadline::default();
            assert!(!limit.expired(start,start,held,visible,escape));
            assert!(limit.expired(start+RELEASE_GRACE,start,held,visible,escape));
        }
        let start=Instant::now(); assert!(Deadline::default().expired(start+MAX_DRAG,start,true,true,false));
    }
    #[test]
    fn only_one_drag_is_allowed_and_cancellation_releases_the_guard() {
        let first=reserve().unwrap(); assert!(reserve().is_err());drop(first);assert!(reserve().is_ok());
    }
    #[test]
    #[cfg(windows)]
    fn mouse_release_reaps_a_real_unresponsive_child_process() {
        use std::os::windows::process::CommandExt;
        let mut command=Command::new("cmd.exe");
        command.args(["/D","/C","ping -n 60 127.0.0.1 >nul"]).creation_flags(0x08000000);
        let mut helper=crate::encoder_job::Encoder::spawn(&mut command).unwrap();
        let start=Instant::now();
        assert!(supervise(&mut helper,|| (false,true,false)).unwrap_err().contains("sin respuesta"));
        assert!(start.elapsed()<Duration::from_secs(4));
        assert!(helper.child.try_wait().unwrap().is_some());
    }
}
