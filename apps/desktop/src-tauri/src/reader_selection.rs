//! Read only an explicit accessibility selection. Never synthesize Copy or inspect the clipboard.
#[cfg(any(windows, test))]
#[path = "reader_selection_terminal.rs"]
mod terminal;
#[cfg(windows)]
mod platform {
    use super::terminal::{self, Candidate, Failure, Tree};
    use std::{sync::{mpsc, OnceLock}, time::{Duration, Instant}};
    use windows::Win32::{Foundation::HWND, System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED}, UI::{Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern, TreeScope_Children, UIA_TextPatternId}, WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, IsChild}}};
    type Reply = mpsc::Sender<Result<String, String>>;
    static WORKER: OnceLock<mpsc::SyncSender<(isize, Instant, Reply)>> = OnceLock::new();
    pub fn foreground() -> isize { unsafe { GetForegroundWindow().0 as isize } }
    pub fn selected_text(hwnd: isize) -> Result<String, String> {
        let worker = WORKER.get_or_init(|| {
            let (tx, rx) = mpsc::sync_channel::<(isize, Instant, Reply)>(1);
            std::thread::spawn(move || unsafe {
                if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() { return; }
                let Ok(automation): Result<IUIAutomation, _> = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) else { return; };
                for (hwnd, deadline, reply) in rx {
                    let result = read(&automation, hwnd, deadline).map_err(Failure::message);
                    let _ = reply.send(result);
                }
            });
            tx
        });
        let (tx, rx) = mpsc::channel();
        worker.try_send((hwnd, Instant::now() + Duration::from_secs(2), tx)).map_err(|_| Failure::Unavailable.message())?;
        rx.recv_timeout(Duration::from_secs(2)).map_err(|_| Failure::Timeout.message())?
    }
    unsafe fn guard(hwnd: HWND, deadline: Instant) -> Result<(), Failure> {
        if hwnd.0.is_null() || GetForegroundWindow() != hwnd { return Err(Failure::Changed); }
        if Instant::now() > deadline { return Err(Failure::Timeout); }
        Ok(())
    }
    unsafe fn read(automation: &IUIAutomation, target: isize, deadline: Instant) -> Result<String, Failure> {
        let hwnd = HWND(target as _);
        guard(hwnd, deadline)?;
        let focused = read_focused(automation, hwnd);
        guard(hwnd, deadline)?;
        let mut class = [0u16; 128];
        let length = GetClassNameW(hwnd, &mut class) as usize;
        terminal::resolve(focused, terminal::is_terminal_class(&String::from_utf16_lossy(&class[..length])), || {
            let focus = automation.GetFocusedElement().map_err(|_| Failure::Unavailable)?;
            let root = automation.ElementFromHandle(hwnd).map_err(|_| Failure::Unavailable)?;
            let text = terminal::selected(&mut TerminalTree { automation, hwnd, deadline }, root)?;
            let current = automation.GetFocusedElement().map_err(|_| Failure::Unavailable)?;
            if !automation.CompareElements(&focus, &current).map_err(|_| Failure::Unavailable)?.as_bool() { return Err(Failure::Changed); }
            guard(hwnd, deadline)?;
            Ok(text)
        })
    }
    unsafe fn read_focused(automation: &IUIAutomation, hwnd: HWND) -> Result<String, Failure> {
        let fail = |_| Failure::Unavailable;
        let focused = automation.GetFocusedElement().map_err(fail)?;
        if focused.CurrentIsPassword().map_err(fail)?.as_bool() { return Err(Failure::Password); }
        let walker = automation.ControlViewWalker().map_err(fail)?;
        let mut node = focused.clone();
        let mut belongs = false;
        for _ in 0..32 {
            if let Ok(native) = node.CurrentNativeWindowHandle() {
                if !native.0.is_null() { belongs = native == hwnd || IsChild(hwnd, native).as_bool(); break; }
            }
            let Ok(parent) = walker.GetParentElement(&node) else { break; }; node = parent;
        }
        if !belongs { return Err(Failure::Unavailable); }
        let mut node = focused;
        for _ in 0..16 {
            if node.CurrentIsPassword().map_err(fail)?.as_bool() { return Err(Failure::Password); }
            if let Ok(pattern) = node.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
                let selections = pattern.GetSelection().map_err(fail)?;
                let count = selections.Length().map_err(fail)?;
                let terminal_control = node.CurrentClassName().map(|name| name.to_string() == "TermControl").unwrap_or(false);
                if count != 1 {
                    return Err(if terminal_control { if count == 0 { Failure::TerminalEmpty } else { Failure::Ambiguous } } else { Failure::Unavailable });
                }
                let text = selections.GetElement(0).map_err(fail)?.GetText(50001).map_err(fail)?.to_string();
                // An explicitly focused terminal pane without a selection must not
                // fall through to an older selection in another visible pane.
                if terminal_control && text.trim().is_empty() { return Err(Failure::TerminalEmpty); }
                if GetForegroundWindow() != hwnd { return Err(Failure::Changed); }
                super::validate_selection(&text).map_err(|_| if text.trim().is_empty() { Failure::Empty } else { Failure::Invalid })?;
                return Ok(text);
            }
            // Do not climb outside the selected application's window.
            if node.CurrentNativeWindowHandle().ok() == Some(hwnd) { break; }
            let Ok(parent) = walker.GetParentElement(&node) else { break; }; node = parent;
        }
        Err(Failure::Unavailable)
    }
    struct TerminalTree<'a> { automation: &'a IUIAutomation, hwnd: HWND, deadline: Instant }
    impl Tree for TerminalTree<'_> {
        type Node = IUIAutomationElement;
        fn guard(&mut self) -> Result<(), Failure> { unsafe { guard(self.hwnd, self.deadline) } }
        fn candidate(&mut self, node: &Self::Node) -> Result<Candidate, Failure> {
            unsafe {
                if node.CurrentIsOffscreen().map_err(|_| Failure::Unavailable)?.as_bool() { return Ok(Candidate::Hidden); }
                if node.CurrentClassName().map_err(|_| Failure::Unavailable)?.to_string() != "TermControl" { return Ok(Candidate::Container); }
                if node.CurrentIsPassword().map_err(|_| Failure::Unavailable)?.as_bool() { return Err(Failure::Password); }
                let pattern = node.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId).map_err(|_| Failure::Unavailable)?;
                let selections = pattern.GetSelection().map_err(|_| Failure::Unavailable)?;
                match selections.Length().map_err(|_| Failure::Unavailable)? {
                    0 => Ok(Candidate::Terminal(None)),
                    1 => Ok(Candidate::Terminal(Some(selections.GetElement(0).map_err(|_| Failure::Unavailable)?.GetText(50001).map_err(|_| Failure::Unavailable)?.to_string()))),
                    _ => Err(Failure::Ambiguous),
                }
            }
        }
        fn children(&mut self, node: &Self::Node, limit: usize) -> Result<Vec<Self::Node>, Failure> {
            unsafe {
                let condition = self.automation.CreateTrueCondition().map_err(|_| Failure::Unavailable)?;
                let children = node.FindAll(TreeScope_Children, &condition).map_err(|_| Failure::Unavailable)?;
                let length = children.Length().map_err(|_| Failure::Unavailable)?;
                if length < 0 || length as usize > limit { return Err(Failure::Unavailable); }
                (0..length).map(|i| children.GetElement(i).map_err(|_| Failure::Unavailable)).collect()
            }
        }
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
