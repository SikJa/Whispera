use windows::Win32::{
    Foundation::HWND,
    System::Threading::{AttachThreadInput, GetCurrentThreadId},
    UI::{
        Input::KeyboardAndMouse::{
            SendInput, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
            VK_CONTROL, VK_V,
        },
        WindowsAndMessaging::{
            GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, IsIconic, IsWindow,
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
        let _ = GetGUIThreadInfo(thread, &mut info);
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
            let mut focus_pid = 0;
            GetWindowThreadProcessId(focus, Some(&mut focus_pid));
            if !IsWindow(Some(focus)).as_bool() || focus_pid != target.pid {
                return Err(
                    "El campo original ya no existe. No se pego en otro campo; texto copiado."
                        .into(),
                );
            }
        }
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let current = GetCurrentThreadId();
        let foreground_thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let mut attached = vec![];
        for id in [foreground_thread, thread] {
            if id != 0
                && id != current
                && !attached.contains(&id)
                && AttachThreadInput(current, id, true).as_bool()
            {
                attached.push(id);
            }
        }
        let _ = SetForegroundWindow(hwnd);
        let focus = HWND(target.focus as _);
        let mut focus_pid = 0;
        GetWindowThreadProcessId(focus, Some(&mut focus_pid));
        if IsWindow(Some(focus)).as_bool() && focus_pid == target.pid {
            let _ = SetFocus(Some(focus));
        }
        for id in attached {
            let _ = AttachThreadInput(current, id, false);
        }
        for _ in 0..8 {
            if GetForegroundWindow() == hwnd {
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
            "No se pudo volver al destino. El texto sigue copiado; no se pego en otra ventana."
                .into(),
        )
    }
}
