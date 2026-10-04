use windows::Win32::{
    Foundation::HWND,
    System::Threading::{AttachThreadInput, GetCurrentThreadId},
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, SendInput, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
            VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN, VK_V,
        },
        WindowsAndMessaging::{
            GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, IsChild, IsIconic, IsWindow,
            SetForegroundWindow, ShowWindow, GUITHREADINFO, SW_RESTORE,
        },
    },
};
#[derive(Clone, Copy)]
pub struct Target {
    hwnd: isize,
    focus: isize,
    pid: u32,
}
// WebView/Electron child controls can belong to a renderer process. Window
// ancestry, rather than the child's PID, proves it is still our original field.
unsafe fn field_belongs_to(hwnd: HWND, focus: HWND) -> bool {
    IsWindow(Some(focus)).as_bool() && (focus == hwnd || IsChild(hwnd, focus).as_bool())
}
fn modifiers_down() -> bool {
    unsafe { [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
        .iter().any(|key| GetAsyncKeyState(key.0 as i32) < 0) }
}
pub fn capture() -> Option<Target> {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut pid = 0;
        let thread = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if thread == 0 || pid == std::process::id() {
            return None;
        }
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        GetGUIThreadInfo(thread, &mut info).ok()?;
        if !field_belongs_to(hwnd, info.hwndFocus) {
            return None;
        }
        Some(Target {
            hwnd: hwnd.0 as isize,
            focus: info.hwndFocus.0 as isize,
            pid,
        })
    }
}
pub fn restore_and_paste(target: Target) -> Result<(), String> {
    unsafe {
        let hwnd = HWND(target.hwnd as _);
        let mut pid = 0;
        let thread = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if !IsWindow(Some(hwnd)).as_bool() || pid != target.pid {
            return Err(
                "El destino original ya no existe. Texto conservado en el portapapeles.".into(),
            );
        }
        if target.focus != 0 {
            let focus = HWND(target.focus as _);
            if !field_belongs_to(hwnd, focus) {
                return Err(
                    "El campo original ya no existe. No se pego en otro campo; texto copiado."
                        .into(),
                );
            }
        }
        // The stop shortcut is handled on key-down. Do not turn Ctrl+V into
        // Alt+Ctrl+V (or release a physical key on the user's behalf).
        let released = std::time::Instant::now();
        while modifiers_down() {
            if released.elapsed() >= std::time::Duration::from_secs(2) {
                return Err("Solta las teclas del atajo para pegar. El texto sigue copiado.".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let current = GetCurrentThreadId();
        let foreground_thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let mut attached = vec![];
        let focus = HWND(target.focus as _);
        let focus_thread = GetWindowThreadProcessId(focus, None);
        for id in [foreground_thread, thread, focus_thread] {
            if id != 0
                && id != current
                && !attached.contains(&id)
                && AttachThreadInput(current, id, true).as_bool()
            {
                attached.push(id);
            }
        }
        let _ = SetForegroundWindow(hwnd);
        if field_belongs_to(hwnd, focus) {
            let _ = SetFocus(Some(focus));
        }
        for id in attached.into_iter().rev() {
            let _ = AttachThreadInput(current, id, false);
        }
        for _ in 0..8 {
            if GetForegroundWindow() == hwnd {
                // Allow the app to process activation before submitting input.
                std::thread::sleep(std::time::Duration::from_millis(60));
                let mut info = GUITHREADINFO {
                    cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                    ..Default::default()
                };
                if GetForegroundWindow() != hwnd || modifiers_down() {
                    continue;
                }
                if !field_belongs_to(hwnd, focus) {
                    return Err("El campo original ya no existe. Texto conservado en el portapapeles.".into());
                }
                if GetGUIThreadInfo(thread, &mut info).is_err() || info.hwndFocus != focus {
                    // Activation of a renderer can lag behind the top-level
                    // window. Retry readiness, never the actual Ctrl+V.
                    continue;
                }
                // Submit once only. Retrying Ctrl+V could duplicate already inserted text.
                let keys = [
                    (VK_CONTROL, false),
                    (VK_V, false),
                    (VK_V, true),
                    (VK_CONTROL, true),
                ];
                let inputs: Vec<INPUT> = keys
                    .into_iter()
                    .map(|(vk, up)| INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: vk,
                                dwFlags: if up {
                                    KEYEVENTF_KEYUP
                                } else {
                                    Default::default()
                                },
                                ..Default::default()
                            },
                        },
                    })
                    .collect();
                if SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) != inputs.len() as u32 {
                    return Err("Windows bloqueo el pegado. Puede haber distintos permisos entre las apps; el texto sigue copiado.".into());
                }
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(80));
            let _ = SetForegroundWindow(hwnd);
        }
        Err(
            "No se pudo volver al campo original. El texto sigue copiado; no se pego en otra ventana."
                .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_field_validation_rejects_another_window_and_destroyed_controls() {
        use windows::{core::w, Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_CHILD, WS_POPUP}};
        unsafe {
            let create = |parent: Option<HWND>| CreateWindowExW(WINDOW_EX_STYLE::default(), w!("STATIC"), w!("Whispera paste fixture"),
                if parent.is_some() { WS_CHILD } else { WS_POPUP }, 0, 0, 10, 10, parent, None, None, None).unwrap();
            let first = create(None);
            let second = create(None);
            let field = create(Some(first));
            assert!(field_belongs_to(first, first));
            assert!(field_belongs_to(first, field));
            assert!(!field_belongs_to(second, field));
            DestroyWindow(field).unwrap();
            assert!(!field_belongs_to(first, field));
            DestroyWindow(first).unwrap();
            DestroyWindow(second).unwrap();
        }
    }
}
