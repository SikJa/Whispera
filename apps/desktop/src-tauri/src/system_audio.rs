use serde::{Deserialize, Serialize};
use std::path::Path;
use windows::Win32::{
    Media::Audio::{
        eRender, Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator, MMDeviceEnumerator,
        DEVICE_STATE_ACTIVE,
    },
    System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
        COINIT_MULTITHREADED,
    },
};

#[derive(Serialize, Deserialize)]
struct Previous {
    id: String,
    muted: bool,
}
struct Com(bool);
impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() }
        }
    }
}
fn enumerator() -> Result<(Com, IMMDeviceEnumerator), String> {
    unsafe {
        let com = Com(CoInitializeEx(None, COINIT_MULTITHREADED).is_ok());
        let devices =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
        Ok((com, devices))
    }
}
pub fn mute(journal: &Path) -> Result<(), String> {
    if journal.exists() {
        return Err("Hay una restauracion de sonido pendiente".into());
    }
    let (_com, enumerator) = enumerator()?;
    let mut previous = vec![];
    unsafe {
        let devices = enumerator
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .map_err(|e| e.to_string())?;
        for index in 0..devices.GetCount().map_err(|e| e.to_string())? {
            let device = devices.Item(index).map_err(|e| e.to_string())?;
            let endpoint: IAudioEndpointVolume = device
                .Activate(CLSCTX_ALL, None)
                .map_err(|e| e.to_string())?;
            let raw = device.GetId().map_err(|e| e.to_string())?;
            let id = raw.to_string().map_err(|e| e.to_string());
            CoTaskMemFree(Some(raw.0.cast()));
            previous.push(Previous {
                id: id?,
                muted: endpoint.GetMute().map_err(|e| e.to_string())?.as_bool(),
            });
        }
    }
    crate::audio::write_new(
        journal,
        &serde_json::to_vec(&previous).map_err(|e| e.to_string())?,
    )?;
    for item in &previous {
        if let Err(e) = set(&enumerator, &item.id, true) {
            let _ = restore(journal);
            return Err(e);
        }
    }
    Ok(())
}
fn set(enumerator: &IMMDeviceEnumerator, id: &str, muted: bool) -> Result<(), String> {
    unsafe {
        let id: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
        let device = enumerator
            .GetDevice(windows::core::PCWSTR(id.as_ptr()))
            .map_err(|e| e.to_string())?;
        let endpoint: IAudioEndpointVolume = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| e.to_string())?;
        endpoint
            .SetMute(muted, std::ptr::null())
            .map_err(|e| e.to_string())
    }
}
pub fn restore(journal: &Path) -> Result<(), String> {
    if !journal.exists() {
        return Ok(());
    }
    let previous: Vec<Previous> =
        serde_json::from_slice(&std::fs::read(journal).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let (_com, enumerator) = enumerator()?;
    let mut failed = false;
    for item in previous {
        if set(&enumerator, &item.id, item.muted).is_err() {
            failed = true;
        }
    }
    if failed {
        return Err(
            "No se pudo restaurar algun dispositivo de sonido. Se reintentara al abrir Whispera."
                .into(),
        );
    }
    std::fs::remove_file(journal).map_err(|e| e.to_string())?;
    Ok(())
}
