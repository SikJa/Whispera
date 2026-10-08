//! Verify the executable, not just the existence of a Run value. Packaged
//! launchers can provide a private HKCU view even without package identity.
//! Always use the real Windows provider, caching reads between explicit actions.
use serde::Deserialize;
use std::{path::Path, sync::Mutex};
#[cfg(test)]
const ENABLED: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
static GATE: Mutex<()> = Mutex::new(());
static CACHE: Mutex<Option<Result<Entry, String>>> = Mutex::new(None);

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
struct Entry {
    command: Option<String>,
    approval: Option<Vec<u8>>,
}
impl Entry {
    fn allowed(&self) -> bool {
        match &self.approval {
            None => true,
            Some(bytes) if bytes.len() == 12 => {
                matches!(u32::from_le_bytes(bytes[..4].try_into().unwrap()), 2 | 6)
            }
            // Unknown/disabled states are never silently re-enabled on launch.
            Some(_) => false,
        }
    }
    fn enabled(&self, expected: &str) -> bool {
        self.allowed()
            && self
                .command
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case(expected))
    }
}
fn command(exe: &Path) -> Result<String, String> {
    let path = exe.to_str().ok_or("La ruta de Whispera no es válida")?;
    if !exe.is_absolute() || !exe.is_file() || path.contains(['"', '\r', '\n']) {
        return Err("No se encontró el ejecutable instalado de Whispera".into());
    }
    Ok(format!("\"{path}\" --autostart"))
}
trait Registry {
    fn read(&mut self) -> Result<Entry, String>;
    fn apply(&mut self, operation: &str, command: &str) -> Result<Entry, String>;
}
fn repair<R: Registry>(registry: &mut R, expected: &str) -> Result<Entry, String> {
    let entry = registry.read()?;
    if entry.command.is_some() && entry.allowed() && !entry.enabled(expected) {
        let actual = registry.apply("command", expected)?;
        if !actual.enabled(expected) || actual.approval != entry.approval {
            return Err("Windows no confirmó la reparación del inicio automático".into());
        }
        return Ok(actual);
    }
    Ok(entry)
}
fn set<R: Registry>(registry: &mut R, expected: &str, enabled: bool) -> Result<Entry, String> {
    let actual = registry.apply(if enabled { "enable" } else { "disable" }, expected)?;
    if (enabled && !actual.enabled(expected)) || (!enabled && actual.command.is_some()) {
        return Err("Windows no confirmó el cambio del inicio automático".into());
    }
    Ok(actual)
}
struct WindowsRegistry;
impl Registry for WindowsRegistry {
    fn read(&mut self) -> Result<Entry, String> {
        provider("read", "")
    }
    fn apply(&mut self, operation: &str, command: &str) -> Result<Entry, String> {
        provider(operation, command)
    }
}
fn provider(operation: &str, command: &str) -> Result<Entry, String> {
    use base64::Engine;
    use std::{
        io::Read,
        os::windows::process::CommandExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    use windows::Win32::System::SystemInformation::GetSystemDirectoryW;
    let mut system = [0u16; 32768];
    let len = unsafe { GetSystemDirectoryW(Some(&mut system)) } as usize;
    if len == 0 || len >= system.len() {
        return Err("No se pudo localizar Windows PowerShell".into());
    }
    let executable = std::path::PathBuf::from(String::from_utf16_lossy(&system[..len]))
        .join(r"WindowsPowerShell\v1.0\powershell.exe");
    let script: Vec<u8> = include_str!("startup_registry.ps1")
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(script);
    let mut child = Command::new(executable)
        .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
        .env("WHISPERA_STARTUP_OPERATION", operation)
        .env("WHISPERA_STARTUP_COMMAND", command)
        .creation_flags(0x08000000)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("No se pudo consultar el inicio de Windows: {e}"))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err("Windows no permitió consultar o cambiar el inicio automático. Volvé a intentarlo desde Whispera abierto desde Inicio.".into());
                }
                let mut output = String::new();
                child
                    .stdout
                    .take()
                    .ok_or("Respuesta de Windows ausente")?
                    .take(32768)
                    .read_to_string(&mut output)
                    .map_err(|e| e.to_string())?;
                return serde_json::from_str(output.trim_start_matches('\u{feff}').trim())
                    .map_err(|_| "Respuesta de inicio automático inválida".into());
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
            _ if start.elapsed() > Duration::from_secs(12) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Windows tardó demasiado en consultar el inicio automático".into());
            }
            _ => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}
fn access(force: bool, operation: Option<bool>, reconcile: bool) -> Result<bool, String> {
    let _guard = GATE.lock().map_err(|_| "Inicio automático ocupado")?;
    let expected = command(&std::env::current_exe().map_err(|e| e.to_string())?)?;
    let mut registry = WindowsRegistry;
    // No PowerShell process per clipboard poll; explicit settings reads refresh.
    if !force {
        if let Some(result) = CACHE
            .lock()
            .map_err(|_| "Inicio automático ocupado")?
            .as_ref()
        {
            return result.clone().map(|entry| entry.enabled(&expected));
        }
    }
    let result = if let Some(enabled) = operation {
        set(&mut registry, &expected, enabled)
    } else if reconcile {
        repair(&mut registry, &expected)
    } else {
        registry.read()
    };
    let mut cache = CACHE.lock().map_err(|_| "Inicio automático ocupado")?;
    // Cache failures too: a temporarily unavailable provider must not start a
    // new PowerShell process on every clipboard poll. Explicit reads retry.
    *cache = Some(result.clone());
    result.map(|entry| entry.enabled(&expected))
}
pub fn enabled() -> Result<bool, String> {
    access(true, None, false)
}
pub fn cached_enabled() -> Result<bool, String> {
    access(false, None, false)
}
pub fn set_enabled(enabled: bool) -> Result<bool, String> {
    access(true, Some(enabled), false)
}
pub fn reconcile() -> Result<bool, String> {
    access(true, None, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Memory {
        entry: Entry,
        writes: Vec<String>,
        reject: bool,
        ignore: bool,
    }
    impl Registry for Memory {
        fn read(&mut self) -> Result<Entry, String> {
            Ok(self.entry.clone())
        }
        fn apply(&mut self, op: &str, cmd: &str) -> Result<Entry, String> {
            if self.reject {
                return Err("denied".into());
            }
            self.writes.push(op.into());
            if self.ignore {
                return Ok(self.entry.clone());
            }
            self.entry.command = if op == "disable" {
                None
            } else {
                Some(cmd.into())
            };
            if op == "enable" {
                self.entry.approval = Some(ENABLED.to_vec());
            }
            Ok(self.entry.clone())
        }
    }
    const EXPECTED: &str = r#""C:\New Folder\Whispera.exe" --autostart"#;
    #[test]
    fn stale_enabled_path_is_repaired_and_idempotent() {
        let mut r = Memory {
            entry: Entry {
                command: Some("C:\\deleted\\Whispera.exe --autostart".into()),
                approval: None,
            },
            ..Default::default()
        };
        assert!(!r.entry.enabled(EXPECTED));
        assert!(repair(&mut r, EXPECTED).unwrap().enabled(EXPECTED));
        repair(&mut r, EXPECTED).unwrap();
        assert_eq!(r.writes, vec!["command"]);
        assert_eq!(r.entry.approval, None);
    }
    #[test]
    fn startup_repair_does_not_enable_missing_or_disabled_entries() {
        for approval in [
            None,
            Some(vec![3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            Some(vec![7, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]),
            Some(vec![2]),
        ] {
            let mut r = Memory {
                entry: Entry {
                    command: approval.as_ref().map(|_| "old".into()),
                    approval,
                },
                ..Default::default()
            };
            let before = r.entry.clone();
            repair(&mut r, EXPECTED).unwrap();
            assert_eq!(r.entry, before);
            assert!(r.writes.is_empty());
        }
    }
    #[test]
    fn explicit_enable_repairs_and_disable_is_idempotent() {
        let mut r = Memory {
            entry: Entry {
                command: Some("old".into()),
                approval: Some(vec![3; 12]),
            },
            ..Default::default()
        };
        assert!(set(&mut r, EXPECTED, true).unwrap().enabled(EXPECTED));
        assert!(!set(&mut r, EXPECTED, false).unwrap().enabled(EXPECTED));
        set(&mut r, EXPECTED, false).unwrap();
        assert!(r.entry.command.is_none());
    }
    #[test]
    fn rejected_or_ignored_writes_never_report_success() {
        for reject in [true, false] {
            let mut r = Memory {
                reject,
                ignore: !reject,
                ..Default::default()
            };
            assert!(set(&mut r, EXPECTED, true).is_err());
        }
        let mut r = Memory {
            entry: Entry {
                command: Some("stale".into()),
                approval: None,
            },
            ignore: true,
            ..Default::default()
        };
        assert!(repair(&mut r, EXPECTED).is_err());
    }
    #[test]
    fn commands_are_exact_and_paths_with_spaces_are_quoted() {
        let dir = std::env::temp_dir().join(format!("Whispera ñ test {}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let exe = dir.join("app.exe");
        std::fs::write(&exe, []).unwrap();
        let cmd = command(&exe).unwrap();
        assert_eq!(cmd, format!("\"{}\" --autostart", exe.display()));
        assert!(command(&dir.join("missing.exe")).is_err());
        let e = Entry {
            command: Some(EXPECTED.into()),
            approval: None,
        };
        assert!(e.enabled(EXPECTED));
        assert!(!e.enabled(&format!("{EXPECTED} extra")));
        std::fs::remove_file(exe).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn provider_identifies_the_actual_user_without_interpolating_commands() {
        let script = include_str!("startup_registry.ps1");
        assert!(script.contains("2147483651"));
        assert!(script.contains("WindowsIdentity]::GetCurrent().User.Value"));
        assert!(script.contains("$env:WHISPERA_STARTUP_COMMAND"));
        assert!(!script.contains("Invoke-Expression"));
    }
    #[test]
    fn repairs_preserve_enabled_approval_variants() {
        for state in [2, 6] {
            let mut bytes = ENABLED.to_vec();
            bytes[0] = state;
            let mut r = Memory {
                entry: Entry {
                    command: Some("old".into()),
                    approval: Some(bytes.clone()),
                },
                ..Default::default()
            };
            assert!(repair(&mut r, EXPECTED).unwrap().enabled(EXPECTED));
            assert_eq!(r.entry.approval, Some(bytes));
            assert_eq!(r.writes, vec!["command"]);
        }
        for bytes in [
            vec![0; 12],
            vec![1; 12],
            vec![2; 4],
            vec![3; 12],
            vec![7; 12],
        ] {
            assert!(!Entry {
                command: Some(EXPECTED.into()),
                approval: Some(bytes)
            }
            .enabled(EXPECTED));
        }
    }
    #[test]
    #[ignore = "Read-only Windows integration check; requires the real registry provider"]
    fn real_provider_read_is_non_mutating() {
        let before = provider("read", "").unwrap();
        assert_eq!(before, provider("read", "").unwrap());
        println!("Windows startup provider read verified without changing registration");
    }
}
