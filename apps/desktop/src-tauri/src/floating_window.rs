use serde::Deserialize;
use windows::Win32::Graphics::Gdi::{
    CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn, RGN_OR,
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_NCRENDERING_POLICY, DWMWA_SYSTEMBACKDROP_TYPE},
    UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
    UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE, HWND_TOPMOST,
        STYLESTRUCT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WM_NCACTIVATE,
        WM_NCDESTROY, WM_NCPAINT, WM_STYLECHANGING,
    },
};
#[derive(Deserialize)]
pub struct HitRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[tauri::command]
pub fn recorder_region(window: tauri::WebviewWindow, rects: Vec<HitRect>) -> Result<(), String> {
    if window.label() != "recorder" || rects.is_empty() || rects.len() > 32 {
        return Err("Region no valida".into());
    }
    if rects.iter().any(|r| {
        [r.x, r.y, r.width, r.height].iter().any(|v| !v.is_finite())
            || r.width <= 0.
            || r.height <= 0.
            || r.width > 2000.
            || r.height > 2000.
            || r.x.abs() > 2000.
            || r.y.abs() > 2000.
    }) {
        return Err("Coordenadas invalidas".into());
    }
    let clone = window.clone();
    window
        .run_on_main_thread(move || {
            let Ok(raw) = clone.hwnd() else {
                return;
            };
            let dpi = clone.scale_factor().unwrap_or(1.);
            unsafe {
                let region = CreateRectRgn(0, 0, 0, 0);
                if region.0.is_null() {
                    return;
                }
                for r in rects {
                    let part = CreateRoundRectRgn(
                        ((r.x - 5.) * dpi).floor() as i32,
                        ((r.y - 5.) * dpi).floor() as i32,
                        ((r.x + r.width + 5.) * dpi).ceil() as i32,
                        ((r.y + r.height + 5.) * dpi).ceil() as i32,
                        (12. * dpi) as i32,
                        (12. * dpi) as i32,
                    );
                    if !part.0.is_null() {
                        let _ = CombineRgn(Some(region), Some(region), Some(part), RGN_OR);
                        let _ = DeleteObject(part.into());
                    }
                }
                // Windows owns the region only after successful SetWindowRgn.
                if SetWindowRgn(HWND(raw.0), Some(region), true) == 0 {
                    let _ = DeleteObject(region.into());
                }
            }
        })
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn recorder_size(window: tauri::WebviewWindow, scale: f64) -> Result<(), String> {
    if window.label() != "recorder" || !scale.is_finite() || !(0.6..=1.25).contains(&scale) {
        return Err("Tamano invalido".into());
    }
    window
        .set_size(tauri::LogicalSize::new(520. * scale, 520. * scale))
        .map_err(|e| e.to_string())
}

fn popup_styles(style: u32, extended: u32) -> (u32, u32) {
    // Same popup contract as _aplicar_sin_bordes_win32 in the original app.
    // Preserve WebView2's compositor flags instead of adding WS_EX_LAYERED.
    let style = (style & 0x10000000) | 0x80000000 | 0x06000000;
    let extended = (extended & !(0x00040000 | 0x00000100 | 0x00000200)) | 0x80 | 0x08;
    (style, extended)
}

const SUBCLASS_ID: usize = 0x57535052;

unsafe extern "system" fn guard_popup(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _data: usize,
) -> LRESULT {
    // Native caption dragging can request non-client painting even without WS_CAPTION.
    // Keep Tao's activation handling, but tell DefWindowProc not to repaint the frame.
    if message == WM_NCPAINT {
        return LRESULT(0);
    }
    if message == WM_NCACTIVATE {
        return DefSubclassProc(hwnd, message, wparam, LPARAM(-1));
    }
    if message == WM_NCDESTROY {
        let _ = RemoveWindowSubclass(hwnd, Some(guard_popup), id);
    }
    let result = DefSubclassProc(hwnd, message, wparam, lparam);
    // Tao can restore its cached styles on later focus/drag/window updates.
    // Filter the proposed styles after the other handlers, before Windows applies them.
    if message == WM_STYLECHANGING && lparam.0 != 0 {
        let styles = &mut *(lparam.0 as *mut STYLESTRUCT);
        match wparam.0 as i32 {
            index if index == GWL_STYLE.0 => styles.styleNew = popup_styles(styles.styleNew, 0).0,
            index if index == GWL_EXSTYLE.0 => styles.styleNew = popup_styles(0, styles.styleNew).1,
            _ => {}
        }
    }
    result
}

pub fn apply(window: &tauri::WebviewWindow) -> Result<(), String> {
    let window_copy = window.clone();
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    // Comctl32 subclass installation must run on the HWND's owning thread.
    window
        .run_on_main_thread(move || {
            let result = window_copy
                .hwnd()
                .map_err(|e| e.to_string())
                .and_then(|raw| apply_native(HWND(raw.0)));
            let _ = send.send(result);
        })
        .map_err(|e| e.to_string())?;
    receive.recv().map_err(|e| e.to_string())?
}

fn apply_native(hwnd: HWND) -> Result<(), String> {
    unsafe {
        if !SetWindowSubclass(hwnd, Some(guard_popup), SUBCLASS_ID, 0).as_bool() {
            return Err("Could not protect floating window styles".into());
        }
        let (style, extended) = popup_styles(
            GetWindowLongPtrW(hwnd, GWL_STYLE) as u32,
            GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32,
        );
        SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, extended as isize);
        // Disable non-client rendering and automatic Mica/Acrylic for this window only.
        let disabled = 1u32;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            (&disabled as *const u32).cast(),
            4,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&disabled as *const u32).cast(),
            4,
        );
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_NOACTIVATE,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn popup_removes_caption_and_taskbar_styles() {
        let (style, ex) = popup_styles(0x16cf0000, 0x00240300);
        assert_eq!(style & 0x00cf0000, 0);
        assert_ne!(style & 0x80000000, 0);
        assert_ne!(style & 0x10000000, 0);
        assert_eq!(ex & 0x00040300, 0);
        assert_ne!(ex & 0x80, 0);
        assert_ne!(ex & 0x00200000, 0);
        assert_eq!(popup_styles(0, 0).0 & 0x10000000, 0);
    }
}
