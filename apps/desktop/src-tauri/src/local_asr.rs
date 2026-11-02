//! On-device transcription through transcribe.cpp: Vulkan on Windows, Metal on macOS,
//! CPU as the fallback. Models are downloaded on demand into the profile folder,
//! pinned to a revision and verified by size and SHA-256 before use.
use crate::storage::Settings;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
use transcribe_cpp::{Backend, Model, ModelOptions, RunOptions, VadOptions, VadSession};

pub struct Spec {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    file: &'static str,
    repository: &'static str,
    revision: &'static str,
    pub bytes: u64,
    sha256: &'static str,
    /// Whisper takes a language hint; Parakeet detects it from the audio.
    language_hint: bool,
}

pub const MODELS: &[Spec] = &[
    Spec {
        id: "parakeet-v3",
        name: "Parakeet v3",
        description: "El más rápido (≈0,3 s). Detecta el idioma solo: en frases muy cortas puede confundirse.",
        file: "parakeet-tdt-0.6b-v3-Q8_0.gguf",
        repository: "handy-computer/parakeet-tdt-0.6b-v3-gguf",
        revision: "85ac09ea12fc4b1112fa76810059364bc6adc9de",
        bytes: 739_508_576,
        sha256: "5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7",
        language_hint: false,
    },
    Spec {
        id: "whisper-turbo",
        name: "Whisper large-v3-turbo",
        description: "Fija el idioma (español): mejor en frases cortas. Tarda ≈1 s.",
        file: "whisper-large-v3-turbo-Q8_0.gguf",
        repository: "handy-computer/whisper-large-v3-turbo-gguf",
        revision: "d222c9f621c1128299248f2ded4d8a1820519780",
        bytes: 886_381_824,
        sha256: "d5e65f2b0828802ae2c231673d31982cebe3a778c95d9494a9f3efee6bd17448",
        language_hint: true,
    },
];

pub fn spec(id: &str) -> Option<&'static Spec> {
    MODELS.iter().find(|m| m.id == id)
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Install {
    /// "", "downloading", "verifying" or "error".
    pub phase: String,
    pub model: String,
    pub downloaded: u64,
    pub total: u64,
    pub error: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    bytes: u64,
    installed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    models: Vec<ModelStatus>,
    install: Install,
    /// Device of the loaded model, e.g. "NVIDIA GeForce GTX 1660 Ti (vulkan)".
    device: String,
    loaded: String,
    /// Why the selected model could not be loaded, if it failed.
    error: String,
}

static INSTALL: Mutex<Install> = Mutex::new(Install { phase: String::new(), model: String::new(), downloaded: 0, total: 0, error: String::new() });
static CANCEL: AtomicBool = AtomicBool::new(false);
static LOADED: Mutex<Option<(String, Model, String)>> = Mutex::new(None);
static LOAD_ERROR: Mutex<String> = Mutex::new(String::new());

fn install_state() -> std::sync::MutexGuard<'static, Install> {
    INSTALL.lock().unwrap_or_else(|e| e.into_inner())
}

fn loaded() -> std::sync::MutexGuard<'static, Option<(String, Model, String)>> {
    LOADED.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn models_dir() -> Result<PathBuf, String> {
    let database = crate::profile::database_path()?;
    Ok(database.parent().ok_or("Perfil invalido")?.join("models"))
}

fn model_path(spec: &Spec) -> Result<PathBuf, String> {
    Ok(models_dir()?.join(spec.file))
}

// The download is verified before the rename, so a file at its final path with
// the pinned size is a complete, checked model.
pub fn installed(spec: &Spec) -> bool {
    model_path(spec).ok().and_then(|p| std::fs::metadata(p).ok()).is_some_and(|m| m.len() == spec.bytes)
}

pub fn status() -> Status {
    // A load in progress holds the lock for seconds; report "not loaded yet" instead of waiting.
    let (loaded, device) = match LOADED.try_lock() {
        Ok(current) => current.as_ref().map(|(id, _, device)| (id.clone(), device.clone())).unwrap_or_default(),
        Err(std::sync::TryLockError::Poisoned(current)) => current.into_inner().as_ref().map(|(id, _, device)| (id.clone(), device.clone())).unwrap_or_default(),
        Err(std::sync::TryLockError::WouldBlock) => Default::default(),
    };
    Status {
        models: MODELS.iter().map(|m| ModelStatus { id: m.id, name: m.name, description: m.description, bytes: m.bytes, installed: installed(m) }).collect(),
        install: install_state().clone(),
        device,
        loaded,
        error: LOAD_ERROR.lock().map(|e| e.clone()).unwrap_or_default(),
    }
}

pub async fn install(id: String) -> Result<(), String> {
    let spec = spec(&id).ok_or("Modelo local desconocido")?;
    {
        let mut state = install_state();
        if matches!(state.phase.as_str(), "downloading" | "verifying") {
            return Err("Ya se está descargando un modelo".into());
        }
        *state = Install { phase: "downloading".into(), model: id.clone(), total: spec.bytes, ..Default::default() };
    }
    CANCEL.store(false, Ordering::SeqCst);
    let result = download(spec, true).await;
    if result.is_ok()&&!CANCEL.load(Ordering::SeqCst) {prepare_speech_detector().await;}
    let mut state = install_state();
    match &result {
        Ok(()) => *state = Install::default(),
        Err(e) => {
            state.phase = if CANCEL.load(Ordering::SeqCst) { String::new() } else { "error".into() };
            state.error = if CANCEL.load(Ordering::SeqCst) { String::new() } else { e.clone() };
        }
    }
    result
}

pub fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

pub fn remove(id: &str) -> Result<(), String> {
    let spec = spec(id).ok_or("Modelo local desconocido")?;
    let mut current = loaded();
    if current.as_ref().is_some_and(|(loaded, ..)| loaded == id) {
        *current = None;
    }
    drop(current);
    match std::fs::remove_file(model_path(spec)?) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("No se pudo borrar el modelo: {e}")),
        _ => Ok(()),
    }
}

async fn download(spec: &Spec, report: bool) -> Result<(), String> {
    let dir = models_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(spec.file);
    let partial = dir.join(format!("{}.part", spec.file));
    let client = reqwest::Client::builder()
        .user_agent("Whispera")
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("https://huggingface.co/{}/resolve/{}/{}", spec.repository, spec.revision, spec.file);
    let mut response = client.get(&url).send().await.map_err(|e| format!("No se pudo conectar con Hugging Face: {e}"))?
        .error_for_status().map_err(|e| format!("Descarga rechazada: {e}"))?;
    let mut file = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    let mut reported = Instant::now();
    let outcome: Result<(), String> = async {
        while let Some(chunk) = response.chunk().await.map_err(|e| format!("La descarga se cortó: {e}"))? {
            if report && CANCEL.load(Ordering::SeqCst) {
                return Err("Descarga cancelada".into());
            }
            downloaded += chunk.len() as u64;
            if downloaded > spec.bytes {
                return Err("El archivo descargado es más grande de lo esperado".into());
            }
            hasher.update(&chunk);
            file.write_all(&chunk).map_err(|e| format!("No se pudo guardar el modelo: {e}"))?;
            if report && reported.elapsed() > Duration::from_millis(200) {
                install_state().downloaded = downloaded;
                reported = Instant::now();
            }
        }
        if report {
            install_state().phase = "verifying".into();
        }
        file.flush().map_err(|e| e.to_string())?;
        let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if downloaded != spec.bytes || digest != spec.sha256 {
            return Err("El modelo descargado no pasó la verificación de integridad".into());
        }
        Ok(())
    }
    .await;
    drop(file);
    if let Err(e) = outcome {
        let _ = std::fs::remove_file(&partial);
        return Err(e);
    }
    std::fs::rename(&partial, &target).map_err(|e| e.to_string())
}

/// Loads (or reuses) the selected model. The first load on a GPU compiles its
/// shaders and takes a few seconds; later dictations reuse the loaded model.
pub fn prepare(id: &str) -> Result<(), String> {
    let spec = spec(id).ok_or("Modelo local desconocido")?;
    let mut current = loaded();
    if current.as_ref().is_some_and(|(loaded, ..)| loaded == id) {
        return Ok(());
    }
    if !installed(spec) {
        return Err(format!("Instalá el modelo local {} en Configuración › Transcripción", spec.name));
    }
    if !vulkan_available() {
        return Err("La transcripción local necesita un driver de video con Vulkan (NVIDIA, AMD o Intel actualizado)".into());
    }
    *current = None;
    transcribe_cpp::disable_logging();
    let model = Model::load(model_path(spec)?).map_err(|e| format!("No se pudo cargar el modelo local: {e}"))?;
    let device = model.device().map(|d| format!("{} ({})", d.description, d.kind)).unwrap_or_else(|_| model.backend());
    // The first run compiles GPU pipelines (several seconds); pay it here, not on a dictation.
    if let Ok(mut session) = model.session() {
        let _ = session.run(&vec![0.0; 16_000], &RunOptions::default());
    }
    *current = Some((id.to_owned(), model, device));
    Ok(())
}

// vulkan-1.dll is delay-loaded (build.rs); the engine must not run without it.
#[cfg(windows)]
fn vulkan_available() -> bool {
    use windows::{core::w, Win32::System::LibraryLoader::LoadLibraryW};
    unsafe { LoadLibraryW(w!("vulkan-1.dll")).is_ok() }
}
#[cfg(not(windows))]
fn vulkan_available() -> bool {
    true
}

/// Warms the model off the dictation path; errors surface when transcribing.
pub fn preload(settings: &Settings) {
    if settings.transcription_engine != "local" {
        return;
    }
    let id = settings.local_model.clone();
    std::thread::spawn(move || {
        let result = prepare(&id);
        if let Ok(mut error) = LOAD_ERROR.lock() {
            *error = result.err().unwrap_or_default();
        }
    });
}

/// Silero VAD, as Hex uses it: models invent "Gracias" or "Ss." for clicks,
/// bumps and room noise, so audio it hears no speech in is never transcribed.
const SPEECH_DETECTOR: Spec = Spec {
    id: "silero-vad",
    name: "Detector de voz",
    description: "",
    file: "silero-vad-v6.2-F32.gguf",
    repository: "handy-computer/silero-vad-v6.2-gguf",
    revision: "a9b2d61c20ae4ddf3644edb34969e8185a485448",
    bytes: 1_241_056,
    sha256: "0d871a800c420fdd5d1b17a3aea4296d1c39608f0ff4a9295eb76457eb1c88e6",
    language_hint: false,
};
static DETECTOR: Mutex<Option<VadSession>> = Mutex::new(None);

/// Downloads (1.2 MB, once) and loads the speech detector off the dictation path.
pub async fn prepare_speech_detector() {
    if !installed(&SPEECH_DETECTOR) && download(&SPEECH_DETECTOR, false).await.is_err() {
        return;
    }
    load_installed_speech_detector().await;
}
/// Startup may reuse an installed detector, but never initiates a background download.
pub async fn load_installed_speech_detector() {
    if installed(&SPEECH_DETECTOR) {let _=tauri::async_runtime::spawn_blocking(|| speech_in(&[0.0;1_600])).await;}
}

/// Whether 16 kHz mono audio holds speech; `None` while the detector is
/// unavailable, in which case callers keep their level-based check.
fn speech_in(pcm: &[f32]) -> Option<bool> {
    let mut detector = DETECTOR.lock().unwrap_or_else(|e| e.into_inner());
    if detector.is_none() {
        if !installed(&SPEECH_DETECTOR) || !vulkan_available() {
            return None;
        }
        transcribe_cpp::disable_logging();
        let path = model_path(&SPEECH_DETECTOR).ok()?;
        let model = Model::load_with(path, &ModelOptions { backend: Backend::Cpu, ..Default::default() }).ok()?;
        *detector = Some(model.vad_session().ok()?);
    }
    // Hex's tuning: 0.4 still hears a whisper at -43 dBFS; 160 ms keeps a short
    // "Ok" and rejects clicks, keys and bumps.
    let options = VadOptions { threshold: 0.4, min_speech_ms: 160, ..Default::default() };
    detector.as_mut()?.run(pcm, &options).ok().map(|result| !result.segments.is_empty())
}

pub fn contains_speech(wav: &[u8]) -> Option<bool> {
    // Unit tests must not depend on a detector installed in this machine's profile.
    if cfg!(test) {
        return None;
    }
    speech_in(&pcm_16k(wav).ok()?)
}

pub fn transcribe(path: &Path, settings: &Settings) -> Result<String, String> {
    let spec = spec(&settings.local_model).ok_or("Modelo local desconocido")?;
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut pcm = pcm_16k(&bytes)?;
    if pcm.len() < 1_600 || speech_in(&pcm) == Some(false) {
        return Ok(String::new());
    }
    // Silence around the clip, at least 1.5 s in all: without it Parakeet drops the
    // last word of a take that ends right at the release, and garbles short ones.
    let pad = 8_000usize.max(24_000usize.saturating_sub(pcm.len()) / 2);
    pcm.splice(0..0, std::iter::repeat_n(0.0, pad));
    pcm.extend(std::iter::repeat_n(0.0, pad));
    prepare(spec.id)?;
    let current = loaded();
    let (_, model, _) = current.as_ref().ok_or("El modelo local no está cargado")?;
    let mut session = model.session().map_err(|e| e.to_string())?;
    let language = (spec.language_hint && settings.language != "auto").then(|| settings.language.clone());
    let limit = model.capabilities().map(|c| c.max_audio_ms).unwrap_or(0);
    // Models with a window (Whisper: 30 s) get consecutive chunks; Parakeet takes it whole.
    let chunk = if limit > 0 { (limit as usize * 16).max(16_000) } else { pcm.len() };
    let mut texts = vec![];
    for part in pcm.chunks(chunk) {
        let result = session
            .run(part, &RunOptions { language: language.clone(), ..Default::default() })
            .map_err(|e| format!("Falló la transcripción local: {e}"))?;
        let text = result.text.trim();
        if !text.is_empty() {
            texts.push(text.to_owned());
        }
    }
    Ok(crate::groq::drop_phantom(&texts.join(" "), &bytes))
}

/// Decodes a WAV into 16 kHz mono f32, the input every transcribe.cpp model takes.
fn pcm_16k(bytes: &[u8]) -> Result<Vec<f32>, String> {
    let mut wav = hound::WavReader::new(std::io::Cursor::new(bytes)).map_err(|e| format!("Audio no válido: {e}"))?;
    let spec = wav.spec();
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => wav.samples::<f32>().filter_map(Result::ok).collect(),
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample.max(1) - 1)) as f32;
            wav.samples::<i32>().filter_map(Result::ok).map(|s| s as f32 / scale).collect()
        }
    };
    let channels = usize::from(spec.channels.max(1));
    let mono: Vec<f32> = interleaved.chunks(channels).map(|f| f.iter().sum::<f32>() / f.len() as f32).collect();
    Ok(resample(&mono, spec.sample_rate))
}

// Windowed-sinc (Blackman) band-limited resampling, cut just below the lower
// Nyquist. A box average aliased consonants and measurably cost words.
fn resample(input: &[f32], rate: u32) -> Vec<f32> {
    if rate == 16_000 || rate == 0 || input.is_empty() {
        return input.to_vec();
    }
    let ratio = 16_000.0 / rate as f64;
    let cutoff = ratio.min(1.0) * 0.95;
    let width = 16.0 / cutoff;
    let count = (input.len() as f64 * ratio).floor() as usize;
    (0..count)
        .map(|i| {
            let center = i as f64 / ratio;
            let first = (center - width).ceil().max(0.0) as usize;
            let last = ((center + width).floor() as usize).min(input.len() - 1);
            let (mut sum, mut weights) = (0.0f64, 0.0f64);
            for (k, sample) in input.iter().enumerate().take(last + 1).skip(first) {
                let t = k as f64 - center;
                let x = std::f64::consts::PI * cutoff * t;
                let sinc = if x.abs() < 1e-9 { 1.0 } else { x.sin() / x };
                let u = std::f64::consts::PI * t / width;
                let window = 0.42 + 0.5 * u.cos() + 0.08 * (2.0 * u).cos();
                sum += f64::from(*sample) * sinc * window;
                weights += sinc * window;
            }
            (sum / weights) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_keeps_duration_and_level() {
        let tone: Vec<f32> = (0..48_000).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.5).collect();
        let out = resample(&tone, 48_000);
        assert_eq!(out.len(), 16_000);
        let peak = out.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!((0.45..=0.52).contains(&peak), "{peak}");
        assert_eq!(resample(&tone[..10], 16_000).len(), 10);
        assert_eq!(resample(&tone[..100], 8_000).len(), 200);
    }

    #[test]
    fn resampling_rejects_frequencies_above_the_new_nyquist() {
        // 11 kHz cannot exist at 16 kHz; a box average folds it back to 5 kHz.
        let hiss: Vec<f32> = (0..48_000).map(|i| (i as f32 * 11_000.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.5).collect();
        let out = resample(&hiss, 48_000);
        let rms = (out[1_000..15_000].iter().map(|s| s * s).sum::<f32>() / 14_000.0).sqrt();
        assert!(rms < 0.01, "{rms}");
    }

    #[test]
    fn catalog_ids_are_unique_and_pinned() {
        for model in MODELS {
            assert_eq!(MODELS.iter().filter(|m| m.id == model.id).count(), 1);
            assert_eq!(model.sha256.len(), 64);
            assert_eq!(model.revision.len(), 40);
        }
    }
}
