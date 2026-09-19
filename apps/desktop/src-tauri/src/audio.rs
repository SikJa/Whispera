use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingState {
    pub phase: String,
    pub seconds: f64,
    pub level: f32,
    pub session: String,
    pub error: String,
    pub text: String,
    pub muted: bool,
    pub progress: String,
}
#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub sample_rate: u32,
    pub created_at: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recovery {
    pub id: String,
    pub created_at: String,
    pub seconds: f64,
    pub path: String,
}
type Reply = mpsc::Sender<Result<String, String>>;
enum Command {
    Start(Reply),
    Pause(Reply),
    Stop(Reply),
}
#[derive(Clone)]
pub struct Recorder {
    tx: mpsc::Sender<Command>,
    pub state: Arc<Mutex<RecordingState>>,
    pub root: PathBuf,
}
struct Capture {
    stream: Option<cpal::Stream>,
    rx: mpsc::Receiver<Vec<i16>>,
    fault: Arc<AtomicBool>,
    file: File,
    dir: PathBuf,
    rate: u32,
    samples: u64,
    synced: Instant,
    paused: Arc<AtomicBool>,
}

impl Recorder {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel();
        let state = Arc::new(Mutex::new(RecordingState {
            phase: "idle".into(),
            ..Default::default()
        }));
        let view = state.clone();
        let dir = root.clone();
        std::thread::Builder::new()
            .name("whispera-audio".into())
            .spawn(move || worker(rx, view, dir))
            .map_err(|e| e.to_string())?;
        Ok(Self { tx, state, root })
    }
    fn request(&self, make: impl FnOnce(Reply) -> Command) -> Result<String, String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(make(tx))
            .map_err(|_| "El motor de audio se cerro")?;
        rx.recv().map_err(|_| "El motor de audio no respondio")?
    }
    pub fn start(&self) -> Result<String, String> {
        self.request(Command::Start)
    }
    pub fn pause(&self) -> Result<String, String> {
        self.request(Command::Pause)
    }
    pub fn stop(&self) -> Result<String, String> {
        self.request(Command::Stop)
    }
    pub fn snapshot(&self) -> RecordingState {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub fn change(&self, f: impl FnOnce(&mut RecordingState)) {
        f(&mut self.state.lock().unwrap_or_else(|e| e.into_inner()));
    }
}
fn send_samples<T: Copy>(
    data: &[T],
    channels: usize,
    convert: impl Fn(T) -> f32,
    tx: &mpsc::SyncSender<Vec<i16>>,
    paused: &AtomicBool,
    fault: &AtomicBool,
) {
    if paused.load(Ordering::Relaxed) {
        return;
    }
    let mono = data
        .chunks_exact(channels)
        .map(|frame| {
            let x = frame.iter().map(|v| convert(*v)).sum::<f32>() / channels as f32;
            (x.clamp(-1., 1.) * 32767.).round() as i16
        })
        .collect();
    if tx.try_send(mono).is_err() {
        fault.store(true, Ordering::Relaxed);
    }
}
fn begin(root: &Path) -> Result<Capture, String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("No hay microfono disponible")?;
    let config = device
        .default_input_config()
        .map_err(|e| format!("No se pudo abrir el microfono: {e}"))?;
    let rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let id = uuid::Uuid::new_v4().to_string();
    let dir = root.join(&id);
    fs::create_dir(&dir).map_err(|e| e.to_string())?;
    let manifest = Manifest {
        id,
        sample_rate: rate,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    write_new(
        &dir.join("session.json"),
        &serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
    )?;
    let file = File::create(dir.join("audio.pcm")).map_err(|e| e.to_string())?;
    let (tx, rx) = mpsc::sync_channel(128);
    let paused = Arc::new(AtomicBool::new(false));
    let fault = Arc::new(AtomicBool::new(false));
    let p = paused.clone();
    let f = fault.clone();
    let err = fault.clone();
    let error = move |_: cpal::StreamError| {
        err.store(true, Ordering::Relaxed);
    };
    let cfg: cpal::StreamConfig = config.clone().into();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &cfg,
            move |data: &[f32], _| send_samples(data, channels, |x| x, &tx, &p, &f),
            error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &cfg,
            move |data: &[i16], _| send_samples(data, channels, |x| x as f32 / 32768., &tx, &p, &f),
            error,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &cfg,
            move |data: &[u16], _| {
                send_samples(
                    data,
                    channels,
                    |x| (x as f32 - 32768.) / 32768.,
                    &tx,
                    &p,
                    &f,
                )
            },
            error,
            None,
        ),
        _ => {
            return Err(
                "Formato del microfono no compatible. Selecciona PCM 16 bits o float32 en Windows."
                    .into(),
            )
        }
    }
    .map_err(|e| format!("Error de microfono: {e}"))?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Capture {
        stream: Some(stream),
        rx,
        fault,
        file,
        dir,
        rate,
        samples: 0,
        synced: Instant::now(),
        paused,
    })
}
impl Capture {
    fn drain(&mut self, view: &Arc<Mutex<RecordingState>>) -> Result<(), String> {
        let mut level = 0f32;
        for _ in 0..128 {
            let Ok(data) = self.rx.try_recv() else {
                break;
            };
            let bytes: Vec<u8> = data.iter().flat_map(|s| s.to_le_bytes()).collect();
            self.file
                .write_all(&bytes)
                .map_err(|e| format!("No se pudo guardar el audio: {e}"))?;
            self.samples += data.len() as u64;
            level = level.max(
                data.iter()
                    .map(|v| (*v as f32).abs() / 32768.)
                    .fold(0f32, f32::max),
            );
        }
        if self.synced.elapsed() >= Duration::from_secs(1) {
            self.file.sync_data().map_err(|e| e.to_string())?;
            self.synced = Instant::now();
        }
        let mut state = view.lock().unwrap_or_else(|e| e.into_inner());
        state.seconds = self.samples as f64 / self.rate as f64;
        state.level = level;
        Ok(())
    }
    fn finish(mut self, view: &Arc<Mutex<RecordingState>>) -> Result<String, String> {
        drop(self.stream.take());
        self.drain(view)?;
        self.file.sync_all().map_err(|e| e.to_string())?;
        if self.samples == 0 {
            return Err("El microfono no entrego audio. Revisa los permisos de Windows.".into());
        }
        Ok(self.dir.file_name().unwrap().to_string_lossy().into_owned())
    }
}
fn worker(rx: mpsc::Receiver<Command>, view: Arc<Mutex<RecordingState>>, root: PathBuf) {
    let mut capture: Option<Capture> = None;
    loop {
        if let Some(c) = capture.as_mut() {
            let result=c.drain(&view).and_then(|_|if c.fault.load(Ordering::Relaxed){Err("Se interrumpio el microfono o el disco no pudo seguir el ritmo. El audio guardado esta en Recuperar.".into())}else{Ok(())});
            if let Err(error) = result {
                if let Some(c) = capture.take() {
                    let _ = c.finish(&view);
                }
                let mut s = view.lock().unwrap();
                s.phase = "error".into();
                s.error = error;
            }
        }
        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(Command::Start(reply)) => {
                if capture.is_some() {
                    let _ = reply.send(Err("Ya estas grabando".into()));
                    continue;
                }
                match begin(&root) {
                    Ok(c) => {
                        let id = c.dir.file_name().unwrap().to_string_lossy().into_owned();
                        *view.lock().unwrap() = RecordingState {
                            phase: "recording".into(),
                            session: id.clone(),
                            ..Default::default()
                        };
                        capture = Some(c);
                        let _ = reply.send(Ok(id));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e));
                    }
                }
            }
            Ok(Command::Pause(reply)) => {
                if let Some(c) = capture.as_ref() {
                    let paused = !c.paused.load(Ordering::Relaxed);
                    c.paused.store(paused, Ordering::Relaxed);
                    view.lock().unwrap().phase = if paused { "paused" } else { "recording" }.into();
                    let _ = reply.send(Ok(String::new()));
                } else {
                    let _ = reply.send(Err("No hay grabacion activa".into()));
                }
            }
            Ok(Command::Stop(reply)) => {
                let result = if let Some(c) = capture.take() {
                    c.finish(&view)
                } else {
                    Err("No hay grabacion activa".into())
                };
                {
                    let mut s = view.lock().unwrap();
                    match &result {
                        Ok(_) => s.phase = "ready".into(),
                        Err(e) => {
                            s.phase = "error".into();
                            s.error = e.clone();
                        }
                    }
                }
                let _ = reply.send(result);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Some(c) = capture.take() {
                    let _ = c.finish(&view);
                }
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = File::options()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())
}
pub fn session_dir(root: &Path, id: &str) -> Result<PathBuf, String> {
    uuid::Uuid::parse_str(id).map_err(|_| "Identificador de audio invalido")?;
    let path = root.join(id);
    if !path.is_dir() {
        return Err("El audio no existe".into());
    }
    Ok(path)
}
pub fn recoveries(root: &Path) -> Result<Vec<Recovery>, String> {
    let mut result = vec![];
    for entry in fs::read_dir(root).map_err(|e| e.to_string())?.flatten() {
        let dir = entry.path();
        if dir.join("completed.json").exists() || dir.join("cancelled.json").exists() {
            continue;
        }
        let Ok(bytes) = fs::read(dir.join("session.json")) else {
            continue;
        };
        let Ok(m) = serde_json::from_slice::<Manifest>(&bytes) else {
            continue;
        };
        let len = fs::metadata(dir.join("audio.pcm"))
            .map(|m| m.len())
            .unwrap_or(0);
        if len > 0 && m.sample_rate > 0 {
            result.push(Recovery {
                id: m.id,
                created_at: m.created_at,
                seconds: len as f64 / 2. / m.sample_rate as f64,
                path: dir.to_string_lossy().into_owned(),
            });
        }
    }
    result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(result)
}
// Work only on a derived WAV. Never rewrite the durable microphone PCM.
pub fn trimmed_copy(path: &Path) -> Result<PathBuf, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Ok(path.to_path_buf());
    }
    let samples: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let chunk = (spec.sample_rate as usize / 2).max(1);
    let audible: Vec<bool> = samples
        .chunks(chunk)
        .map(|s| {
            s.iter().map(|x| (*x as f64 / 32768.).powi(2)).sum::<f64>() / s.len() as f64
                > 0.005f64.powi(2)
        })
        .collect();
    if !audible.iter().any(|x| *x) {
        return Ok(path.to_path_buf());
    }
    let output = path.with_extension("trim.wav");
    let mut writer = hound::WavWriter::create(&output, spec).map_err(|e| e.to_string())?;
    for (i, block) in samples.chunks(chunk).enumerate() {
        // Keep neighbouring half-seconds so quiet consonants and word tails survive.
        if audible[i] || i > 0 && audible[i - 1] || i + 1 < audible.len() && audible[i + 1] {
            for s in block {
                writer.write_sample(*s).map_err(|e| e.to_string())?;
            }
        }
    }
    writer.finalize().map_err(|e| e.to_string())?;
    Ok(output)
}
// Raw PCM + immutable format metadata remain the recovery source, even if a WAV is interrupted.
pub fn materialize(dir: &Path, part_bytes: usize) -> Result<Vec<PathBuf>, String> {
    let m: Manifest =
        serde_json::from_slice(&fs::read(dir.join("session.json")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if m.sample_rate == 0 || part_bytes < 2 {
        return Err("Formato de recuperacion invalido".into());
    }
    let mut input = File::open(dir.join("audio.pcm")).map_err(|e| e.to_string())?;
    let mut parts = vec![];
    loop {
        let mut bytes = vec![];
        Read::by_ref(&mut input)
            .take((part_bytes / 2 * 2) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() < 2 {
            break;
        }
        let output = dir.join(format!("part-{:04}.wav", parts.len()));
        let mut wav = hound::WavWriter::create(
            &output,
            hound::WavSpec {
                channels: 1,
                sample_rate: m.sample_rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .map_err(|e| e.to_string())?;
        for pair in bytes.chunks_exact(2) {
            wav.write_sample(i16::from_le_bytes([pair[0], pair[1]]))
                .map_err(|e| e.to_string())?;
        }
        wav.finalize().map_err(|e| e.to_string())?;
        File::options()
            .write(true)
            .open(&output)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        parts.push(output);
    }
    if parts.is_empty() {
        return Err("El archivo de audio esta vacio".into());
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trim_preserves_source_and_guard_audio() {
        let dir = std::env::temp_dir().join(format!("whispera-trim-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("source.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..160000 {
            w.write_sample(if (72000..80000).contains(&i) {
                4000i16
            } else {
                0
            })
            .unwrap();
        }
        w.finalize().unwrap();
        let before = fs::read(&path).unwrap();
        let out = trimmed_copy(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_ne!(out, path);
        let trimmed = hound::WavReader::open(out).unwrap();
        assert_eq!(trimmed.duration(), 24000);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn long_audio_roundtrip_and_crash_recovery() {
        let root =
            std::env::temp_dir().join(format!("whispera-audio-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        write_new(
            &root.join("session.json"),
            br#"{"id":"test","sample_rate":16000,"created_at":"2026-01-01"}"#,
        )
        .unwrap();
        let samples = 16000 * 60 * 30;
        let mut raw = File::create(root.join("audio.pcm")).unwrap();
        let block = vec![42u8; 32000];
        for _ in 0..1800 {
            raw.write_all(&block).unwrap();
        }
        raw.sync_all().unwrap();
        drop(raw);
        let parts = materialize(&root, 16_000_000).unwrap();
        assert!(parts.len() > 1);
        let total: u32 = parts
            .iter()
            .map(|p| hound::WavReader::open(p).unwrap().duration())
            .sum();
        assert_eq!(total, samples);
        for p in &parts {
            assert!(p.metadata().unwrap().len() < 24_000_000);
        }
        assert_eq!(materialize(&root, 16_000_000).unwrap().len(), parts.len());
        for p in parts {
            fs::remove_file(p).unwrap();
        }
        fs::remove_file(root.join("session.json")).unwrap();
        fs::remove_file(root.join("audio.pcm")).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn rejects_traversal() {
        assert!(session_dir(Path::new("."), "../x").is_err());
    }
    #[test]
    fn stereo_downmix() {
        let (tx, rx) = mpsc::sync_channel(1);
        send_samples(
            &[1f32, -1., 0.5, 0.5],
            2,
            |x| x,
            &tx,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
        );
        assert_eq!(rx.recv().unwrap(), vec![0, 16384]);
    }
}
