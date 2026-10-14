//! Optional capture shortcut routing. No key remapping, keyboard log or polling.
#[cfg(target_os = "windows")]
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    mpsc, OnceLock,
};
use tauri::Manager;
#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
        },
        WindowsAndMessaging::{
            CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT,
            MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
        },
    },
};

const SETTING: &str = "windows_capture_shortcuts";
#[cfg(target_os = "windows")]
static ENABLED: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static HELD: AtomicU32 = AtomicU32::new(0);
#[cfg(target_os = "windows")]
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
#[cfg(target_os = "windows")]
static SENDER: OnceLock<mpsc::SyncSender<Action>> = OnceLock::new();
#[cfg(target_os = "windows")]
static STARTED: OnceLock<Result<(), String>> = OnceLock::new();
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Whispera,
    Block,
    Windows,
}

#[cfg(target_os = "windows")]
fn classify(vk: u32, ctrl: bool, alt: bool, shift: bool, win: bool) -> Option<Action> {
    if vk == 0x2c {
        return Some(if ctrl || alt || shift || win {
            Action::Block
        } else {
            Action::Whispera
        });
    }
    if vk == 0x53 && win && shift && !ctrl && !alt {
        return Some(Action::Block);
    }
    if vk == 0x7b && ctrl && alt && shift && !win {
        return Some(Action::Windows);
    }
    None
}
#[cfg(target_os = "windows")]
fn bit(vk: u32) -> u32 {
    match vk {
        0x2c => 1,
        0x53 => 2,
        0x7b => 4,
        _ => 0,
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code != 0 {
        return CallNextHookEx(None, code, wp, lp);
    }
    let key = &*(lp.0 as *const KBDLLHOOKSTRUCT);
    // Leave synthetic input from accessibility tools alone.
    if key.flags.0 & 0x10 != 0 {
        return CallNextHookEx(None, code, wp, lp);
    }
    let mask = bit(key.vkCode);
    let message = wp.0 as u32;
    if mask != 0
        && [WM_KEYUP, WM_SYSKEYUP].contains(&message)
        && HELD.fetch_and(!mask, Ordering::SeqCst) & mask != 0
    {
        return LRESULT(1);
    }
    if !ENABLED.load(Ordering::SeqCst)
        || mask == 0
        || ![WM_KEYDOWN, WM_SYSKEYDOWN].contains(&message)
    {
        return CallNextHookEx(None, code, wp, lp);
    }
    let down = |vk| GetAsyncKeyState(vk) < 0;
    let action = classify(
        key.vkCode,
        down(VK_CONTROL.0 as i32),
        down(VK_MENU.0 as i32),
        down(VK_SHIFT.0 as i32),
        down(VK_LWIN.0 as i32) || down(VK_RWIN.0 as i32),
    );
    if let Some(action) = action {
        if APP.get().is_some_and(|app| {
            app.state::<crate::shortcuts::Capture>()
                .active
                .load(Ordering::SeqCst)
        }) {
            return CallNextHookEx(None, code, wp, lp);
        }
        if HELD.fetch_or(mask, Ordering::SeqCst) & mask == 0 && action != Action::Block {
            if let Some(sender) = SENDER.get() {
                let _ = sender.try_send(action);
            }
        }
        return LRESULT(1);
    }
    CallNextHookEx(None, code, wp, lp)
}

#[cfg(target_os = "windows")]
fn ensure_hook(app: &tauri::AppHandle) -> Result<(), String> {
    STARTED
        .get_or_init(|| {
            let _ = APP.set(app.clone());
            let (sender, receiver) = mpsc::sync_channel(8);
            let _ = SENDER.set(sender);
            let worker_app = app.clone();
            std::thread::Builder::new()
                .name("whispera-capture-shortcuts".into())
                .spawn(move || {
                    while let Ok(action) = receiver.recv() {
                        if !ENABLED.load(Ordering::SeqCst) {
                            continue;
                        }
                        let result = match action {
                            // select_image already handles an open editor and checks the gate.
                            // The old busy check silently discarded Print Screen in editing mode.
                            Action::Whispera => crate::screen::select_image(&worker_app),
                            Action::Windows => open_windows_capture(),
                            _ => Ok(()),
                        };
                        if let Err(error) = result {
                            let _ = worker_app
                                .state::<crate::storage::Store>()
                                .event(&format!("Atajo de captura: {error}"));
                        }
                    }
                })
                .map_err(|e| e.to_string())?;
            let (ready, result) = mpsc::channel();
            std::thread::Builder::new()
                .name("whispera-capture-keyboard".into())
                .spawn(move || unsafe {
                    let module = GetModuleHandleW(None).map(|value| HINSTANCE(value.0));
                    let installed = module.and_then(|module| {
                        SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), Some(module), 0)
                    });
                    match installed {
                        Ok(handle) => {
                            let _ = ready.send(Ok(()));
                            let mut message = MSG::default();
                            while GetMessageW(&mut message, None, 0, 0).0 > 0 {}
                            let _ = UnhookWindowsHookEx(handle);
                        }
                        Err(error) => {
                            let _ = ready.send(Err(format!(
                                "Windows no permitio separar los atajos: {error}"
                            )));
                        }
                    }
                })
                .map_err(|e| e.to_string())?;
            result
                .recv_timeout(std::time::Duration::from_secs(2))
                .map_err(|e| e.to_string())?
        })
        .clone()
}
#[cfg(target_os = "windows")]
fn open_windows_capture() -> Result<(), String> {
    use windows::{
        core::w,
        Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    };
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            w!("ms-screenclip:"),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        return Err("Windows no pudo abrir su herramienta de captura".into());
    }
    Ok(())
}
#[cfg(target_os = "windows")]
pub fn start(app: &tauri::AppHandle) {
    if app
        .state::<crate::storage::Store>()
        .get::<bool>(SETTING)
        .unwrap_or(false)
    {
        match ensure_hook(app) {
            Ok(()) => {
                ENABLED.store(true, Ordering::SeqCst);
                let _ = app
                    .state::<crate::storage::Store>()
                    .event("Atajos de captura de Windows separados");
            }
            Err(error) => {
                let _ = app.state::<crate::storage::Store>().event(&error);
            }
        }
    }
}
#[cfg(target_os = "macos")]
pub fn start(app: &tauri::AppHandle) {
    if app
        .state::<crate::storage::Store>()
        .get::<bool>(SETTING)
        .unwrap_or(false)
    {
        let _ = app.state::<crate::storage::Store>().event("La separacion de atajos de Windows no se aplica en macOS; usa los atajos configurables de Whispera");
    }
}
#[tauri::command]
pub fn windows_capture_shortcuts(
    store: tauri::State<crate::storage::Store>,
) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    return store.get(SETTING);
    #[cfg(target_os = "macos")]
    {
        let _ = store;
        Ok(false)
    }
}
#[tauri::command]
pub fn save_windows_capture_shortcuts(enabled: bool, app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    if enabled {
        ensure_hook(&app)?;
    }
    #[cfg(target_os = "macos")]
    if enabled {
        return Err("Esta opcion separa Impr Pant de la captura de Windows. En macOS configura los atajos de Whispera; Command+Shift+3/4/5 siguen perteneciendo al sistema".into());
    }
    app.state::<crate::storage::Store>()
        .put(SETTING, &enabled)?;
    #[cfg(target_os = "windows")]
    ENABLED.store(enabled, Ordering::SeqCst);
    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    #[test]
    fn native_hook_can_install_without_blocking_or_remapping_keys() {
        unsafe extern "system" fn pass(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
            CallNextHookEx(None, code, wp, lp)
        }
        std::thread::spawn(|| unsafe {
            let module = GetModuleHandleW(None).unwrap();
            let handle =
                SetWindowsHookExW(WH_KEYBOARD_LL, Some(pass), Some(HINSTANCE(module.0)), 0)
                    .unwrap();
            UnhookWindowsHookEx(handle).unwrap();
        })
        .join()
        .unwrap();
    }
    #[test]
    fn capture_routing_leaves_typing_and_other_windows_shortcuts_alone() {
        assert_eq!(
            classify(0x2c, false, false, false, false),
            Some(Action::Whispera)
        );
        assert_eq!(
            classify(0x2c, false, false, false, true),
            Some(Action::Block)
        );
        assert_eq!(
            classify(0x53, false, false, true, true),
            Some(Action::Block)
        );
        assert_eq!(classify(0x53, false, false, false, true), None);
        assert_eq!(classify(0x53, false, false, false, false), None);
        assert_eq!(
            classify(0x7b, true, true, true, false),
            Some(Action::Windows)
        );
        assert_eq!(classify(0x7b, false, false, false, false), None);
        assert_eq!(classify(0x5a, true, false, false, false), None);
    }
}
