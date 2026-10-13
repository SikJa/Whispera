//! AX objects stay in Swift. A target is an opaque, bounded-lifetime registry token.
#[derive(Clone, Copy)]
pub struct Target {
    token: u64,
    capture_status: i32,
}

unsafe extern "C" {
    fn whispera_paste_capture(token: *mut u64) -> i32;
    fn whispera_paste_restore(
        token: u64,
        expected: *const u8,
        length: usize,
        check_text: i32,
    ) -> i32;
}

pub fn capture() -> Option<Target> {
    crate::paste_focus::start();
    let mut token = 0;
    let capture_status = unsafe { whispera_paste_capture(&mut token) };
    if capture_status == 2 {
        return None;
    }
    // Keep permission/capture errors until the caller can display them after copying.
    Some(Target {
        token,
        capture_status,
    })
}

pub fn restore_and_paste(target: Target) -> Result<(), String> {
    restore(target, None)
}

pub fn restore_and_paste_text(target: Target, expected: &str) -> Result<(), String> {
    restore(target, Some(expected))
}

fn restore(target: Target, expected: Option<&str>) -> Result<(), String> {
    if target.capture_status != 0 {
        return Err(error(target.capture_status));
    }
    let (bytes, length, check) = expected.map_or((std::ptr::null(), 0, 0), |value| {
        (value.as_ptr(), value.len(), 1)
    });
    let status = unsafe { whispera_paste_restore(target.token, bytes, length, check) };
    if status == 0 {
        Ok(())
    } else {
        Err(error(status))
    }
}

fn error(status: i32) -> String {
    match status {
        1 => "Dale permiso de Accesibilidad a Whispera en Ajustes del Sistema > Privacidad y seguridad. El texto sigue copiado.",
        2 => "No habia un campo de destino externo al iniciar. El contenido sigue copiado.",
        3 => "El destino original ya no existe o cambio. No se pego en otra ventana; el contenido sigue copiado.",
        4 => "Solta las teclas del atajo para pegar. El contenido sigue copiado.",
        5 => "No se pudo volver al campo original. El contenido sigue copiado; no se pego en otro campo.",
        6 => "El portapapeles cambio antes del pegado. La transcripcion sigue en el historial.",
        7 => "macOS no permite enviar teclas. Revisa el permiso de Accesibilidad de Whispera; el contenido sigue copiado.",
        8 => "El destino es un campo protegido o no admite texto editable accesible. El contenido sigue copiado.",
        9 => "Hay otro pegado en curso. El contenido sigue copiado.",
        _ => "macOS no pudo comprobar el destino de pegado. El contenido sigue copiado.",
    }.into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn permission_and_clipboard_failures_are_distinguishable() {
        assert!(super::error(1).contains("Accesibilidad"));
        assert!(super::error(6).contains("portapapeles cambio"));
        assert!(super::error(5).contains("no se pego en otro campo"));
    }
}
