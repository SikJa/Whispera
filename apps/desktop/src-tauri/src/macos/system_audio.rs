//! Persist original CoreAudio controls before changing any output device.
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path, sync::Mutex};

static GATE: Mutex<()> = Mutex::new(());
const MAX_JOURNAL: usize = 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    kind: String,
    channel: u32,
    value: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Device {
    uid: String,
    controls: Vec<Control>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u32,
    devices: Vec<Device>,
}

unsafe extern "C" {
    fn whispera_system_audio_snapshot(buffer: *mut u8, capacity: usize) -> isize;
    fn whispera_system_audio_set(bytes: *const u8, length: usize, restore: i32) -> i32;
}

fn validate(journal: &Journal) -> Result<(), String> {
    let mut uids = HashSet::new();
    if journal.version != 1 || journal.devices.len() > 128 {
        return Err("Registro de restauracion de audio incompatible".into());
    }
    for device in &journal.devices {
        let mut channels = HashSet::new();
        if device.uid.is_empty()
            || device.uid.len() > 8192
            || !uids.insert(&device.uid)
            || device.controls.is_empty()
            || device.controls.len() > 256
            || device.controls.iter().any(|c| {
                !["mute", "volume"].contains(&c.kind.as_str())
                    || c.channel > 256
                    || !channels.insert((&c.kind, c.channel))
                    || !c.value.is_finite()
                    || !(0.0..=1.0).contains(&c.value)
                    || (c.kind == "mute" && c.value != 0.0 && c.value != 1.0)
            })
        {
            return Err("Registro de restauracion de audio invalido".into());
        }
    }
    Ok(())
}

fn error(status: i32) -> String {
    match status {
        1 => "macOS no encontro dispositivos de salida activos".into(),
        2 => "Una salida de audio no permite controlar mute ni volumen desde macOS. No se puede silenciar todo el sistema con ese dispositivo conectado".into(),
        4 => "El registro de restauracion de audio no es valido".into(),
        5 => "Una salida de audio esta desconectada. Su restauracion quedo pendiente hasta reconectarla".into(),
        _ => format!("CoreAudio no confirmo el cambio de sonido (codigo {status}). Se conserva el registro de restauracion"),
    }
}

fn set(device: &Device, restore: bool) -> Result<(), String> {
    let data = serde_json::to_vec(device).map_err(|e| e.to_string())?;
    let status =
        unsafe { whispera_system_audio_set(data.as_ptr(), data.len(), i32::from(restore)) };
    if status == 0 {
        Ok(())
    } else {
        Err(error(status))
    }
}

pub fn mute(path: &Path) -> Result<(), String> {
    let _guard = GATE.lock().map_err(|_| "Sonido del sistema ocupado")?;
    if path.exists() {
        return Err("Hay una restauracion de sonido pendiente".into());
    }
    let mut buffer = vec![0u8; MAX_JOURNAL];
    let length = unsafe { whispera_system_audio_snapshot(buffer.as_mut_ptr(), buffer.len()) };
    if length <= 0 {
        return Err(error((-length) as i32));
    }
    if length as usize > buffer.len() {
        return Err(error(4));
    }
    buffer.truncate(length as usize);
    let devices: Vec<Device> = serde_json::from_slice(&buffer).map_err(|e| e.to_string())?;
    let journal = Journal {
        version: 1,
        devices,
    };
    validate(&journal)?;
    if journal.devices.is_empty() {
        return Err(error(1));
    }
    crate::audio::write_new(
        path,
        &serde_json::to_vec(&journal).map_err(|e| e.to_string())?,
    )?;
    for device in &journal.devices {
        if let Err(failure) = set(device, false) {
            return match restore_locked(path) {
                Ok(()) => Err(failure),
                Err(recovery) => Err(format!("{failure}. {recovery}")),
            };
        }
    }
    Ok(())
}

pub fn restore(path: &Path) -> Result<(), String> {
    let _guard = GATE.lock().map_err(|_| "Sonido del sistema ocupado")?;
    restore_locked(path)
}

fn restore_locked(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_JOURNAL as u64 {
        return Err(error(4));
    }
    let journal: Journal = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| {
            format!("Registro de audio no legible; no se modificaron dispositivos: {e}")
        })?;
    validate(&journal)?;
    let mut remaining = vec![];
    let mut failure = None;
    for device in journal.devices {
        if let Err(reason) = set(&device, true) {
            failure = Some(reason);
            remaining.push(device);
        }
    }
    if remaining.is_empty() {
        return std::fs::remove_file(path).map_err(|e| e.to_string());
    }
    // Do not reapply already-restored volumes on every retry if another output is unplugged.
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let data = serde_json::to_vec(&Journal {
        version: 1,
        devices: remaining,
    })
    .map_err(|e| e.to_string())?;
    let saved = crate::audio::write_new(&temporary, &data)
        .and_then(|_| std::fs::rename(&temporary, path).map_err(|e| e.to_string()));
    if saved.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    saved?;
    Err(failure.unwrap_or_else(|| error(3)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn journal(value: f64) -> Journal {
        Journal {
            version: 1,
            devices: vec![Device {
                uid: "BuiltInSpeakerDevice".into(),
                controls: vec![Control {
                    kind: "volume".into(),
                    channel: 0,
                    value,
                }],
            }],
        }
    }
    #[test]
    fn volume_journal_retains_original_scalar_and_device_identity() {
        let original = journal(0.37);
        let decoded: Journal =
            serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        validate(&decoded).unwrap();
        assert_eq!(decoded.devices[0].controls[0].value, 0.37);
        assert_eq!(decoded.devices[0].uid, "BuiltInSpeakerDevice");
    }
    #[test]
    fn malformed_audio_journal_cannot_change_devices() {
        for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            assert!(validate(&journal(value)).is_err());
        }
        let mut value = journal(0.5);
        value.devices[0].controls[0].kind = "mute".into();
        assert!(validate(&value).is_err());
        value.devices[0].controls[0].value = 1.0;
        validate(&value).unwrap();
        value.version = 2;
        assert!(validate(&value).is_err());
        assert!(serde_json::from_str::<Journal>(r#"[{"id":"windows-id","muted":false}]"#).is_err());
    }
}
