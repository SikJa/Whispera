//! Remember the last editable control per external window. COM objects stay on
//! one MTA thread; only numeric tokens cross into the dictation worker.
use std::{collections::{HashMap, VecDeque}, sync::{mpsc, OnceLock}, time::Duration};
use windows::Win32::{
    Foundation::HWND,
    System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED},
    UI::{Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationElement, UIA_EditControlTypeId},
        WindowsAndMessaging::{GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, IsChild, IsWindow, GUITHREADINFO}},
};

#[derive(Clone, Copy)]
pub struct Tracked { pub focus: isize, pub token: u64 }
enum Request {
    Capture(isize, u32, mpsc::Sender<Option<Tracked>>),
    Focus(u64, bool, mpsc::Sender<bool>),
}
struct Field { hwnd: isize, pid: u32, focus: isize, element: IUIAutomationElement }
static TRACKER: OnceLock<mpsc::SyncSender<Request>> = OnceLock::new();

pub fn start() {
    TRACKER.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel(8);
        let _ = std::thread::Builder::new().name("whispera-input-focus".into()).spawn(move || unsafe {
            if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() { return; }
            let Ok(automation): Result<IUIAutomation, _> = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) else { return; };
            let mut recent = HashMap::<isize, Field>::new();
            let mut order = VecDeque::new();
            let mut pending = HashMap::<u64, Field>::new();
            let mut tokens = VecDeque::new();
            let mut next_token = 0u64;
            loop {
                let request = match receiver.recv_timeout(Duration::from_millis(250)) {
                    Ok(request) => Some(request),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                };
                remember(&automation, &mut recent, &mut order);
                match request {
                    Some(Request::Capture(hwnd, pid, reply)) => {
                        let tracked = recent.get(&hwnd).filter(|field| field.pid == pid).map(|field| {
                            next_token = next_token.wrapping_add(1);
                            pending.insert(next_token, Field { hwnd, pid, focus: field.focus, element: field.element.clone() });
                            tokens.push_back(next_token);
                            while tokens.len() > 16 { if let Some(old) = tokens.pop_front() { pending.remove(&old); } }
                            Tracked { focus: field.focus, token: next_token }
                        });
                        let _ = reply.send(tracked);
                    }
                    Some(Request::Focus(token, restore, reply)) => {
                        let ready = pending.get(&token).is_some_and(|field| {
                            let hwnd = HWND(field.hwnd as _);
                            let mut pid = 0;
                            GetWindowThreadProcessId(hwnd, Some(&mut pid));
                            if pid != field.pid || !IsWindow(Some(hwnd)).as_bool() || GetForegroundWindow() != hwnd
                                || !belongs_to_window(&automation, &field.element, hwnd) { return false; }
                            if restore && field.element.SetFocus().is_err() { return false; }
                            automation.GetFocusedElement().and_then(|current| automation.CompareElements(&current, &field.element))
                                .map(|equal| equal.as_bool()).unwrap_or(false)
                        });
                        let _ = reply.send(ready);
                    }
                    None => {}
                }
            }
        });
        sender
    });
}

unsafe fn remember(automation: &IUIAutomation, recent: &mut HashMap<isize, Field>, order: &mut VecDeque<isize>) {
    let hwnd = GetForegroundWindow();
    let mut pid = 0;
    let thread = GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if thread == 0 || pid == std::process::id() { return; }
    let Ok(element) = automation.GetFocusedElement() else { return; };
    // Do not remember generic document/body controls or buttons as text fields.
    if element.CurrentControlType().ok() != Some(UIA_EditControlTypeId)
        || !element.CurrentIsEnabled().map(|v| v.as_bool()).unwrap_or(false)
        || !element.CurrentIsKeyboardFocusable().map(|v| v.as_bool()).unwrap_or(false)
        || !belongs_to_window(automation, &element, hwnd) { return; }
    let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    if GetGUIThreadInfo(thread, &mut info).is_err() || GetForegroundWindow() != hwnd
        || !IsWindow(Some(info.hwndFocus)).as_bool()
        || (info.hwndFocus != hwnd && !IsChild(hwnd, info.hwndFocus).as_bool()) { return; }
    let key = hwnd.0 as isize;
    recent.insert(key, Field { hwnd: key, pid, focus: info.hwndFocus.0 as isize, element });
    order.retain(|&old| old != key);
    order.push_back(key);
    while order.len() > 16 { if let Some(old) = order.pop_front() { recent.remove(&old); } }
}

unsafe fn belongs_to_window(automation: &IUIAutomation, element: &IUIAutomationElement, hwnd: HWND) -> bool {
    let Ok(walker) = automation.ControlViewWalker() else { return false; };
    let mut node = element.clone();
    for _ in 0..32 {
        if let Ok(native) = node.CurrentNativeWindowHandle() {
            if !native.0.is_null() { return native == hwnd || IsChild(hwnd, native).as_bool(); }
        }
        let Ok(parent) = walker.GetParentElement(&node) else { return false; };
        node = parent;
    }
    false
}

pub fn capture(hwnd: isize, pid: u32) -> Option<Tracked> {
    start();
    let (reply, response) = mpsc::channel();
    TRACKER.get()?.try_send(Request::Capture(hwnd, pid, reply)).ok()?;
    response.recv_timeout(Duration::from_millis(400)).ok().flatten()
}
pub fn focus(token: u64, restore: bool) -> bool {
    let (reply, response) = mpsc::channel();
    TRACKER.get().is_some_and(|tracker| tracker.try_send(Request::Focus(token, restore, reply)).is_ok())
        && response.recv_timeout(Duration::from_millis(400)).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_accessibility_identifies_text_fields_instead_of_their_host_window() {
        std::thread::spawn(|| unsafe {
            use windows::{core::w, Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_CHILD, WS_POPUP}};
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
            let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).unwrap();
            let parent = CreateWindowExW(WINDOW_EX_STYLE::default(), w!("STATIC"), w!("Whispera focus fixture"), WS_POPUP,
                0, 0, 10, 10, None, None, None, None).unwrap();
            let edit = CreateWindowExW(WINDOW_EX_STYLE::default(), w!("EDIT"), w!(""), WS_CHILD,
                0, 0, 10, 10, Some(parent), None, None, None).unwrap();
            let field = automation.ElementFromHandle(edit).unwrap();
            let host = automation.ElementFromHandle(parent).unwrap();
            assert_eq!(field.CurrentControlType().unwrap(), UIA_EditControlTypeId);
            assert!(!automation.CompareElements(&host, &field).unwrap().as_bool());
            assert!(automation.CompareElements(&field, &field.clone()).unwrap().as_bool());
            assert!(belongs_to_window(&automation, &field, parent));
            let other = CreateWindowExW(WINDOW_EX_STYLE::default(), w!("STATIC"), w!("Other paste fixture"), WS_POPUP,
                0, 0, 10, 10, None, None, None, None).unwrap();
            assert!(!belongs_to_window(&automation, &field, other));
            DestroyWindow(other).unwrap();
            DestroyWindow(edit).unwrap();
            DestroyWindow(parent).unwrap();
        }).join().unwrap();
    }
}
