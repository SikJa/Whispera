use std::path::Path;

pub fn open_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("x-apple.systempreferences:") {
        return Err("Direcci\u{f3}n no permitida".into());
    }
    open(url)
}

pub fn open_directory(path: &Path) -> Result<(), String> {
    if !path.is_dir() { return Err("La carpeta ya no existe".into()); }
    open(path.to_str().ok_or("Ruta no v\u{e1}lida")?)
}

pub fn reveal_path(path: &Path) -> Result<(), String> {
    if !path.exists() { return Err("El archivo ya no existe".into()); }
    #[cfg(windows)]
    let mut command = { let mut c = std::process::Command::new("explorer.exe"); c.arg(format!("/select,{}", path.display())); c };
    #[cfg(target_os = "macos")]
    let mut command = { let mut c = std::process::Command::new("/usr/bin/open"); c.arg("-R").arg(path); c };
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn open(target: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::{core::{w, HSTRING}, Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL}};
        let result = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(target), None, None, SW_SHOWNORMAL) };
        if result.0 as isize <= 32 { return Err("No se pudo abrir el destino".into()); }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    { std::process::Command::new("/usr/bin/open").arg(target).spawn().map(|_| ()).map_err(|e| e.to_string()) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn unsafe_urls_are_rejected_without_launching_a_process() {
        for url in ["file:///etc/passwd", "javascript:alert(1)", "http://localhost", "-a Calculator"] {
            assert!(super::open_url(url).is_err());
        }
    }
}
