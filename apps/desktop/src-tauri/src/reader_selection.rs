//! Read only an explicit accessibility selection. Never synthesize Copy or inspect the clipboard.
#[cfg(windows)]
mod platform {
    use std::{sync::{mpsc, OnceLock}, time::{Duration, Instant}};
    use windows::Win32::{Foundation::HWND, System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED}, UI::{Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId}, WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, IsChild}}};
    type Reply = mpsc::Sender<Result<String, String>>;
    static WORKER: OnceLock<mpsc::SyncSender<(isize, Instant, Reply)>> = OnceLock::new();
    const UNAVAILABLE: &str = "Seleccioná texto en una aplicación compatible y volvé a pulsar el atajo. También podés pegarlo en Lectura.";
    pub fn foreground() -> isize { unsafe { GetForegroundWindow().0 as isize } }
    pub fn selected_text(hwnd: isize) -> Result<String, String> {
        let worker = WORKER.get_or_init(|| {
            let (tx, rx) = mpsc::sync_channel::<(isize, Instant, Reply)>(1);
            std::thread::spawn(move || unsafe {
                if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() { return; }
                let Ok(automation): Result<IUIAutomation, _> = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) else { return; };
                for (hwnd, deadline, reply) in rx {
                    let result = if Instant::now() > deadline { Err(UNAVAILABLE.into()) } else { read(&automation, hwnd).map_err(|error| explain_error(error, hwnd)) };
                    let _ = reply.send(result);
                }
            });
            tx
        });
        let (tx, rx) = mpsc::channel();
        worker.try_send((hwnd, Instant::now() + Duration::from_secs(2), tx)).map_err(|_| UNAVAILABLE.to_string())?;
        rx.recv_timeout(Duration::from_secs(2)).map_err(|_| UNAVAILABLE.to_string())?
    }
    // Windows Terminal applications can handle mouse input themselves. Explain
    // native Shift-selection only for ordinary empty/unavailable selection errors;
    // successful reads, password/limit errors and foreground checks stay intact.
    unsafe fn explain_error(error: String, target: isize) -> String {
        let hwnd = HWND(target as _);
        if target != 0 && GetForegroundWindow() == hwnd &&
            (error == UNAVAILABLE || error == "No hay texto seleccionado. Seleccioná un fragmento y pulsá el atajo.") {
            let mut class = [0u16; 128];
            let length = GetClassNameW(hwnd, &mut class) as usize;
            if String::from_utf16_lossy(&class[..length]) == "CASCADIA_HOSTING_WINDOW_CLASS" {
                return "Windows Terminal no entregó una selección de texto. Mantené Shift mientras arrastrás para seleccionar, soltá Shift y volvé a pulsar el atajo.".into();
            }
        }
        error
    }
    unsafe fn read(automation: &IUIAutomation, target: isize) -> Result<String, String> {
        let hwnd = HWND(target as _);
        if target == 0 || GetForegroundWindow() != hwnd { return Err(UNAVAILABLE.into()); }
        let fail = |_| UNAVAILABLE.to_string();
        let focused = automation.GetFocusedElement().map_err(fail)?;
        if focused.CurrentIsPassword().map_err(fail)?.as_bool() { return Err("No se leen campos de contraseña.".into()); }
        let walker = automation.ControlViewWalker().map_err(fail)?;
        let mut node = focused.clone();
        let mut belongs = false;
        for _ in 0..32 {
            if let Ok(native) = node.CurrentNativeWindowHandle() {
                if !native.0.is_null() { belongs = native == hwnd || IsChild(hwnd, native).as_bool(); break; }
            }
            let Ok(parent) = walker.GetParentElement(&node) else { break; }; node = parent;
        }
        if !belongs { return Err(UNAVAILABLE.into()); }
        let mut node = focused;
        for _ in 0..16 {
            if node.CurrentIsPassword().map_err(fail)?.as_bool() { return Err("No se leen campos de contraseña.".into()); }
            if let Ok(pattern) = node.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
                let selections = pattern.GetSelection().map_err(fail)?;
                if selections.Length().map_err(fail)? != 1 { return Err(UNAVAILABLE.into()); }
                let text = selections.GetElement(0).map_err(fail)?.GetText(50001).map_err(fail)?.to_string();
                if GetForegroundWindow() != hwnd { return Err("La ventana cambió. Seleccioná el texto otra vez.".into()); }
                super::validate_selection(&text)?;
                return Ok(text);
            }
            // Do not climb outside the selected application's window.
            if node.CurrentNativeWindowHandle().ok() == Some(hwnd) { break; }
            let Ok(parent) = walker.GetParentElement(&node) else { break; }; node = parent;
        }
        Err(UNAVAILABLE.into())
    }
}
#[cfg(windows)]
pub use platform::*;
#[cfg(not(windows))]
pub fn foreground() -> isize { 0 }
#[cfg(not(windows))]
pub fn selected_text(_: isize) -> Result<String, String> { Err("La lectura por atajo está disponible en Windows.".into()) }
fn validate_selection(text: &str) -> Result<(), String> {
    if text.trim().is_empty() { return Err("No hay texto seleccionado. Seleccioná un fragmento y pulsá el atajo.".into()); }
    if text.encode_utf16().count() > 50000 || text.contains('\0') { return Err("Seleccioná hasta 50.000 caracteres para leer.".into()); }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test] fn selection_requires_actual_bounded_text() {
        assert!(super::validate_selection("Texto seleccionado").is_ok());
        assert!(super::validate_selection(" \n").is_err());
        assert!(super::validate_selection(&"😀".repeat(25001)).is_err());
        assert!(super::validate_selection("bad\0selection").is_err());
    }
}
