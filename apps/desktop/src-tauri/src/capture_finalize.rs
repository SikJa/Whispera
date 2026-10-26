use serde::{Deserialize, Serialize};
use std::{fs, io::{Read, Write}, path::Path, time::Duration};

// Encoding a long recording is not an interactive operation: budget for its
// duration, while retaining a finite bound for a stuck encoder.
pub fn budget(seconds: f64) -> Duration {
    let seconds = if seconds.is_finite() { seconds.max(0.) } else { 0. };
    Duration::from_secs((seconds * 2. + 300.).clamp(900., 7200.).ceil() as u64)
}

#[derive(Serialize, Deserialize)]
pub struct Microphone {
    pub rate: u32,
    pub channels: u16,
}

#[derive(Serialize)]
pub struct PendingVideo { pub id: String, pub error: String }

pub fn pending(root: &Path) -> Result<Vec<PendingVideo>, String> {
    if !root.exists() { return Ok(Vec::new()); }
    let mut entries = Vec::new();
    for entry in fs::read_dir(root).map_err(|e| e.to_string())?.filter_map(Result::ok) {
        if !entry.file_type().is_ok_and(|t| t.is_dir()) { continue; }
        let id = entry.file_name().to_string_lossy().into_owned();
        if uuid::Uuid::parse_str(&id).is_err() { continue; }
        if let Ok(error) = fs::read_to_string(entry.path().join("recovery-error.txt")) {
            entries.push(PendingVideo { id, error });
        }
    }
    Ok(entries)
}

pub fn remember_microphone(part: &Path, rate: u32, channels: u16) -> Result<(), String> {
    let bytes = serde_json::to_vec(&Microphone { rate, channels }).map_err(|e| e.to_string())?;
    fs::write(part.join("microphone-format.json"), bytes).map_err(|e| e.to_string())
}

// Preserve a voice-recovery session, independently of whether MP4 muxing works.
// The manifest is published last, so the recovery UI never sees half a copy.
pub fn preserve_microphone(session: &Path, recordings: &Path) -> Result<Option<String>, String> {
    let id = session.file_name().and_then(|n| n.to_str()).ok_or("Sesion sin identificador")?;
    uuid::Uuid::parse_str(id).map_err(|e| e.to_string())?;
    let destination = recordings.join(id);
    if destination.join("session.json").is_file() { return Ok(Some(id.into())); }
    let mut parts: Vec<_> = fs::read_dir(session).map_err(|e| e.to_string())?
        .filter_map(Result::ok).map(|e| e.path())
        .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("part-")))
        .collect();
    parts.sort();
    if !parts.iter().any(|p| p.join("microphone.pcm").is_file()) { return Ok(None); }
    fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
    let temporary = destination.join(format!("recovery-{}.partial", uuid::Uuid::new_v4()));
    let result = (|| -> Result<(), String> {
        let mut output = fs::File::create(&temporary).map_err(|e| e.to_string())?;
        let mut rate = None;
        for part in parts {
            let pcm = part.join("microphone.pcm");
            if !pcm.is_file() { return Err("Falta una pista de microfono; originales conservados".into()); }
            let format: Microphone = serde_json::from_slice(&fs::read(part.join("microphone-format.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            if format.rate == 0 || format.channels == 0 || format.channels > 32 { return Err("Formato de microfono invalido".into()); }
            if rate.is_some_and(|r| r != format.rate) { return Err("Cambio de frecuencia entre segmentos; originales conservados".into()); }
            rate = Some(format.rate);
            let mut input = fs::File::open(pcm).map_err(|e| e.to_string())?;
            let frame_size = usize::from(format.channels) * 2;
            if input.metadata().map_err(|e| e.to_string())?.len() % frame_size as u64 != 0 {
                return Err("Pista incompleta; originales conservados".into());
            }
            let mut frame = vec![0; frame_size];
            let mut input = std::io::BufReader::new(&mut input);
            let mut writer = std::io::BufWriter::new(&mut output);
            loop {
                match input.read_exact(&mut frame) {
                    Ok(()) => {
                        let sum: i32 = frame.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]]) as i32).sum();
                        writer.write_all(&((sum / i32::from(format.channels)) as i16).to_le_bytes()).map_err(|e| e.to_string())?;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e.to_string()),
                }
            }
            writer.flush().map_err(|e| e.to_string())?;
        }
        output.sync_all().map_err(|e| e.to_string())?;
        if output.metadata().map_err(|e| e.to_string())?.len() == 0 { return Err("Microfono vacio; originales conservados".into()); }
        drop(output);
        let target = destination.join("audio.pcm");
        if target.exists() { return Err("Ya existe audio recuperado; no se reemplazo".into()); }
        fs::rename(&temporary, target).map_err(|e| e.to_string())?;
        let manifest = crate::audio::Manifest { id: id.into(), sample_rate: rate.ok_or("Sin pistas")?, created_at: chrono::Utc::now().to_rfc3339() };
        fs::write(destination.join("session.json"), serde_json::to_vec(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temporary); }
    result.map(|_| Some(id.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_video_gets_duration_aware_budget() {
        assert_eq!(budget(30.).as_secs(), 900);
        assert_eq!(budget(767.).as_secs(), 1834);
        assert_eq!(budget(3600.).as_secs(), 7200);
        assert_eq!(budget(f64::NAN).as_secs(), 900);
    }
    #[test]
    fn failed_video_becomes_recoverable_voice_without_touching_originals() {
        let root = std::env::temp_dir().join(format!("whispera-finalize-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        let session = root.join(&id);
        let part = session.join("part-0000");
        fs::create_dir_all(&part).unwrap();
        remember_microphone(&part, 48000, 2).unwrap();
        let samples: Vec<u8> = [1000i16, 3000, -2000, 0].into_iter().flat_map(i16::to_le_bytes).collect();
        fs::write(part.join("microphone.pcm"), &samples).unwrap();
        let recordings = root.join("recordings");
        assert_eq!(preserve_microphone(&session, &recordings).unwrap(), Some(id.clone()));
        let audio = fs::read(recordings.join(&id).join("audio.pcm")).unwrap();
        assert_eq!(audio, [2000i16, -1000].into_iter().flat_map(i16::to_le_bytes).collect::<Vec<_>>());
        assert_eq!(crate::audio::recoveries(&recordings).unwrap().len(), 1);
        assert_eq!(fs::read(part.join("microphone.pcm")).unwrap(), samples);
        preserve_microphone(&session, &recordings).unwrap();
        assert_eq!(fs::read(recordings.join(&id).join("audio.pcm")).unwrap(), audio);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_audio_never_publishes_incomplete_recovery() {
        let root = std::env::temp_dir().join(format!("whispera-finalize-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        let session = root.join(&id);
        let part = session.join("part-0000");
        fs::create_dir_all(&part).unwrap();
        remember_microphone(&part, 48000, 2).unwrap();
        fs::write(part.join("microphone.pcm"), [1, 2, 3]).unwrap();
        let recordings = root.join("recordings");
        assert!(preserve_microphone(&session, &recordings).is_err());
        assert!(!recordings.join(&id).join("session.json").exists());
        assert_eq!(fs::read(part.join("microphone.pcm")).unwrap(), vec![1, 2, 3]);
        fs::write(session.join("recovery-error.txt"), "Encoder timeout").unwrap();
        assert_eq!(pending(&root).unwrap()[0].id, id);
        assert_eq!(pending(&root).unwrap()[0].error, "Encoder timeout");
        fs::remove_dir_all(root).unwrap();
    }
}
