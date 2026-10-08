use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
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
    input: Arc<InputBuffer>,
    faults: mpsc::Receiver<String>,
    sync: DiskSync,
    file: File,
    dir: PathBuf,
    rate: u32,
    samples: u64,
    synced: Instant,
    paused: Arc<AtomicBool>,
}

// Bound by audio duration, not by the device's variable callback block size.
struct InputBuffer {
    tx: mpsc::Sender<Vec<i16>>,
    queued: AtomicUsize,
    limit: usize,
    failed: AtomicBool,
    faults: mpsc::SyncSender<String>,
}
impl InputBuffer {
    fn fail(&self, message: String) {
        if !self.failed.swap(true, Ordering::Relaxed) {
            let _ = self.faults.try_send(message);
        }
    }
    fn send(&self, samples: Vec<i16>) {
        if samples.is_empty() || self.failed.load(Ordering::Relaxed) {
            return;
        }
        let count = samples.len();
        if self
            .queued
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                n.checked_add(count).filter(|total| *total <= self.limit)
            })
            .is_err()
        {
            self.fail("El guardado se atraso mas de 30 segundos. Se detuvo la grabacion para no ocultar perdida de audio.".into());
            return;
        }
        if self.tx.send(samples).is_err() {
            self.queued.fetch_sub(count, Ordering::Relaxed);
            self.fail("Se cerro el canal interno de audio.".into());
        }
    }
}

// Slow disk flushes must not block the audio consumer. Only one flush is queued.
struct DiskSync {
    tx: Option<mpsc::SyncSender<()>>,
    worker: Option<std::thread::JoinHandle<Result<(), String>>>,
    errors: mpsc::Receiver<String>,
}
impl DiskSync {
    fn new(file: File) -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel(1);
        let (errors_tx, errors) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("whispera-audio-sync".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    if let Err(e) = file.sync_data() {
                        let message = format!("No se pudo sincronizar el audio con el disco: {e}");
                        let _ = errors_tx.send(message.clone());
                        return Err(message);
                    }
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            tx: Some(tx),
            worker: Some(worker),
            errors,
        })
    }
    fn finish(&mut self) -> Result<(), String> {
        self.tx.take();
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| "El hilo de sincronizacion de audio fallo".to_string())?,
            None => Ok(()),
        }
    }
}
impl Drop for DiskSync {
    fn drop(&mut self) {
        let _ = self.finish();
    }
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
    input: &InputBuffer,
    paused: &AtomicBool,
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
    input.send(mono);
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
    let sync = DiskSync::new(file.try_clone().map_err(|e| e.to_string())?)?;
    let (tx, rx) = mpsc::channel();
    let (fault_tx, faults) = mpsc::sync_channel(1);
    let input = Arc::new(InputBuffer {
        tx,
        queued: AtomicUsize::new(0),
        limit: rate as usize * 30,
        failed: AtomicBool::new(false),
        faults: fault_tx,
    });
    let paused = Arc::new(AtomicBool::new(false));
    let p = paused.clone();
    let f = input.clone();
    let err = input.clone();
    let error = move |e: cpal::StreamError| {
        err.fail(format!("El microfono interrumpio la captura: {e}"));
    };
    let cfg: cpal::StreamConfig = config.clone().into();
    let stream =
        match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &cfg,
                move |data: &[f32], _| send_samples(data, channels, |x| x, &f, &p),
                error,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_input_stream(
                &cfg,
                move |data: &[i16], _| send_samples(data, channels, |x| x as f32 / 32768., &f, &p),
                error,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_input_stream(
                &cfg,
                move |data: &[u16], _| {
                    send_samples(data, channels, |x| (x as f32 - 32768.) / 32768., &f, &p)
                },
                error,
                None,
            ),
            _ => return Err(
                "Formato del microfono no compatible. Selecciona PCM 16 bits o float32 en Windows."
                    .into(),
            ),
        }
        .map_err(|e| format!("Error de microfono: {e}"))?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Capture {
        stream: Some(stream),
        rx,
        input,
        faults,
        sync,
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
        self.drain_blocks(view, 128)
    }
    fn drain_blocks(
        &mut self,
        view: &Arc<Mutex<RecordingState>>,
        blocks: usize,
    ) -> Result<(), String> {
        let mut level = 0f32;
        for _ in 0..blocks {
            let Ok(data) = self.rx.try_recv() else {
                break;
            };
            self.input.queued.fetch_sub(data.len(), Ordering::Relaxed);
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
            if let Some(tx) = &self.sync.tx {
                let _ = tx.try_send(());
            }
            self.synced = Instant::now();
        }
        let mut state = view.lock().unwrap_or_else(|e| e.into_inner());
        state.seconds = self.samples as f64 / self.rate as f64;
        state.level = level;
        Ok(())
    }
    fn finish(mut self, view: &Arc<Mutex<RecordingState>>) -> Result<String, String> {
        drop(self.stream.take());
        let result = (|| {
            self.drain_blocks(view, usize::MAX)?;
            self.sync.finish()?;
            self.file
                .sync_all()
                .map_err(|e| format!("No se pudo finalizar el audio en disco: {e}"))?;
            if let Ok(error) = self.faults.try_recv() {
                return Err(error);
            }
            if self.samples == 0 {
                return Err(
                    "El microfono no entrego audio. Revisa los permisos de Windows.".into(),
                );
            }
            Ok(self.dir.file_name().unwrap().to_string_lossy().into_owned())
        })();
        if let Err(error) = &result {
            record_capture_error(&self.dir, error);
        }
        result
    }
}
fn record_capture_error(dir: &Path, error: &str) {
    let diagnostic = serde_json::json!({ "at": chrono::Utc::now().to_rfc3339(), "error": error });
    if let Ok(bytes) = serde_json::to_vec(&diagnostic) {
        let _ = write_new(&dir.join("capture-error.json"), &bytes);
    }
}
fn worker(rx: mpsc::Receiver<Command>, view: Arc<Mutex<RecordingState>>, root: PathBuf) {
    let mut capture: Option<Capture> = None;
    loop {
        if let Some(c) = capture.as_mut() {
            let result = c.drain(&view).and_then(|_| {
                if let Ok(error) = c.faults.try_recv() {
                    return Err(error);
                }
                if let Ok(error) = c.sync.errors.try_recv() {
                    return Err(error);
                }
                Ok(())
            });
            if let Err(mut error) = result {
                if let Some(c) = capture.take() {
                    let dir = c.dir.clone();
                    if let Err(finish_error) = c.finish(&view) {
                        error = format!("{error} Error al finalizar: {finish_error}");
                    }
                    record_capture_error(&dir, &error);
                }
                let mut s = view.lock().unwrap();
                s.phase = "error".into();
                s.error = format!("{error} El audio guardado esta en Recuperar.");
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
        let (input, rx, _) = test_input(16000);
        send_samples(
            &[1f32, -1., 0.5, 0.5],
            2,
            |x| x,
            &input,
            &AtomicBool::new(false),
        );
        assert_eq!(rx.recv().unwrap(), vec![0, 16384]);
    }
    fn test_input(
        limit: usize,
    ) -> (
        Arc<InputBuffer>,
        mpsc::Receiver<Vec<i16>>,
        mpsc::Receiver<String>,
    ) {
        let (tx, rx) = mpsc::channel();
        let (faults, errors) = mpsc::sync_channel(1);
        (
            Arc::new(InputBuffer {
                tx,
                queued: AtomicUsize::new(0),
                limit,
                failed: AtomicBool::new(false),
                faults,
            }),
            rx,
            errors,
        )
    }
    #[test]
    fn buffering_handles_more_than_128_device_callbacks_without_loss() {
        let (input, rx, errors) = test_input(48000 * 30);
        for _ in 0..300 {
            input.send(vec![123; 480]);
        }
        assert!(errors.try_recv().is_err());
        assert_eq!(input.queued.load(Ordering::Relaxed), 144000);
        assert_eq!(rx.try_iter().map(|b| b.len()).sum::<usize>(), 144000);
    }
    #[test]
    fn buffer_limit_reports_loss_and_preserves_only_contiguous_prefix() {
        let (input, rx, errors) = test_input(4);
        input.send(vec![1, 2, 3, 4]);
        input.send(vec![5]);
        input.queued.store(0, Ordering::Relaxed);
        input.send(vec![6]);
        assert!(errors.recv().unwrap().contains("30 segundos"));
        assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![vec![1, 2, 3, 4]]);
    }
    #[test]
    fn microphone_and_disconnected_consumer_have_distinct_errors() {
        let (input, rx, errors) = test_input(4);
        drop(rx);
        input.send(vec![1]);
        assert!(errors.recv().unwrap().contains("canal interno"));
        assert_eq!(input.queued.load(Ordering::Relaxed), 0);
        let (input, _, errors) = test_input(4);
        input.fail("El microfono interrumpio la captura: device disconnected".into());
        assert!(errors.recv().unwrap().contains("device disconnected"));
    }
    #[test]
    fn finish_drains_entire_backlog_and_keeps_recovery_pcm() {
        let dir = std::env::temp_dir().join(format!("whispera-drain-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let file = File::create(dir.join("audio.pcm")).unwrap();
        let sync = DiskSync::new(file.try_clone().unwrap()).unwrap();
        let (input, rx, faults) = test_input(48000 * 30);
        for _ in 0..300 {
            input.send(vec![123; 480]);
        }
        let capture = Capture {
            stream: None,
            rx,
            input,
            faults,
            sync,
            file,
            dir: dir.clone(),
            rate: 48000,
            samples: 0,
            synced: Instant::now(),
            paused: Arc::new(AtomicBool::new(false)),
        };
        let view = Arc::new(Mutex::new(RecordingState::default()));
        capture.finish(&view).unwrap();
        assert_eq!(view.lock().unwrap().seconds, 3.0);
        let bytes = fs::read(dir.join("audio.pcm")).unwrap();
        assert_eq!(bytes.len(), 288000);
        assert!(bytes
            .chunks_exact(2)
            .all(|b| i16::from_le_bytes([b[0], b[1]]) == 123));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn stalled_flush_does_not_block_audio_drain() {
        let dir = std::env::temp_dir().join(format!("whispera-sync-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let file = File::create(dir.join("audio.pcm")).unwrap();
        let (flush_tx, _flush_rx) = mpsc::sync_channel(1);
        flush_tx.send(()).unwrap();
        let (_error_tx, errors) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            release_rx.recv().unwrap();
            Ok(())
        });
        let sync = DiskSync {
            tx: Some(flush_tx),
            worker: Some(worker),
            errors,
        };
        let (input, rx, faults) = test_input(48000 * 30);
        input.send(vec![123; 480]);
        let mut capture = Capture {
            stream: None,
            rx,
            input,
            faults,
            sync,
            file,
            dir: dir.clone(),
            rate: 48000,
            samples: 0,
            synced: Instant::now() - Duration::from_secs(2),
            paused: Arc::new(AtomicBool::new(false)),
        };
        let (done_tx, done_rx) = mpsc::channel();
        let drain = std::thread::spawn(move || {
            let result = capture.drain(&Arc::new(Mutex::new(RecordingState::default())));
            done_tx.send(result.map(|_| capture.samples)).unwrap();
            capture
        });
        let result = done_rx.recv_timeout(Duration::from_secs(2));
        release_tx.send(()).unwrap();
        drop(drain.join().unwrap());
        assert_eq!(result.unwrap().unwrap(), 480);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn stop_does_not_report_success_after_input_failure() {
        let dir =
            std::env::temp_dir().join(format!("whispera-fault-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let file = File::create(dir.join("audio.pcm")).unwrap();
        let sync = DiskSync::new(file.try_clone().unwrap()).unwrap();
        let (input, rx, faults) = test_input(48000 * 30);
        input.send(vec![123; 480]);
        input.fail("El microfono interrumpio la captura: test failure".into());
        let capture = Capture {
            stream: None,
            rx,
            input,
            faults,
            sync,
            file,
            dir: dir.clone(),
            rate: 48000,
            samples: 0,
            synced: Instant::now(),
            paused: Arc::new(AtomicBool::new(false)),
        };
        assert!(capture
            .finish(&Arc::new(Mutex::new(RecordingState::default())))
            .unwrap_err()
            .contains("test failure"));
        assert_eq!(fs::metadata(dir.join("audio.pcm")).unwrap().len(), 960);
        let diagnostic: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("capture-error.json")).unwrap()).unwrap();
        assert!(diagnostic["error"]
            .as_str()
            .unwrap()
            .contains("test failure"));
        fs::remove_dir_all(dir).unwrap();
    }
}
