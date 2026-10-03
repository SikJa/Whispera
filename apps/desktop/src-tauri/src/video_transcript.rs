use crate::{
    groq, library,
    storage::{Rule, Settings, Store},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};
use tauri::Manager;

static GATE: Mutex<()> = Mutex::new(());
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Transcript {
    pub status: String,
    pub text: String,
    pub error: String,
    parts: Vec<PathBuf>,
    completed: Vec<String>,
    #[serde(default)]
    preparation_failed: bool,
}
fn key(path: &Path) -> String {
    format!("video_transcript:{}", path.to_string_lossy())
}
pub fn get(store: &Store, path: &Path) -> Result<Transcript, String> {
    store.get(&key(path))
}
fn enabled(store: &Store, name: &str) -> bool {
    library::settings(store)
        .ok()
        .and_then(|p| p[name].as_bool())
        .unwrap_or(true)
}
pub fn recording_enabled(store: &Store) -> bool {
    enabled(store, "transcribeVideo")
}

// The source is the microphone PCM, never the mixed desktop soundtrack.
pub fn prepare(binary: &Path, pcm: &Path, rate: u32, channels: u16) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let output = pcm
        .parent()
        .ok_or("Audio sin directorio")?
        .join("speech-%04d.wav");
    let mut child = std::process::Command::new(binary)
        .creation_flags(0x08000000)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "s16le",
            "-ar",
            &rate.to_string(),
            "-ac",
            &channels.to_string(),
            "-i",
        ])
        .arg(pcm)
        .args([
            "-ar",
            "16000",
            "-ac",
            "1",
            "-c:a",
            "pcm_s16le",
            "-f",
            "segment",
            "-segment_time",
            "300",
        ])
        .arg(output)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err("No se pudo preparar el microfono; PCM conservado".into())
            };
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Preparacion del microfono agotada; PCM conservado".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
pub fn enqueue(store: &Store, video: &Path) -> Result<(), String> {
    let _guard = GATE.lock().map_err(|_| "Transcripciones ocupadas")?;
    let mut directories: Vec<_> = fs::read_dir(video.parent().ok_or("Video sin directorio")?)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .starts_with("part-")
        })
        .collect();
    directories.sort();
    let mut transcript = Transcript {
        status: "pending".into(),
        ..Default::default()
    };
    for directory in directories {
        if let Ok(error) = fs::read_to_string(directory.join("speech-error.txt")) {
            transcript.status = "error".into();
            transcript.error = error;
            transcript.preparation_failed = true;
        }
        let mut parts: Vec<_> = fs::read_dir(directory)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|e| e == "wav")
                    && p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .starts_with("speech-")
            })
            .collect();
        parts.sort();
        transcript.parts.extend(parts);
    }
    if transcript.parts.is_empty() && transcript.error.is_empty() {
        return Ok(());
    }
    let mut queue: Vec<PathBuf> = store.get("video_transcript_queue")?;
    if !queue.contains(&video.to_path_buf()) {
        queue.push(video.into());
    }
    store.put_many(&[
        (
            "video_transcript_queue",
            serde_json::to_value(queue).map_err(|e| e.to_string())?,
        ),
        (
            &key(video),
            serde_json::to_value(transcript).map_err(|e| e.to_string())?,
        ),
    ])
}
fn process(store: &Store, video: &Path) -> Result<Transcript, String> {
    process_with(store, video, |part, settings, rules| {
        tauri::async_runtime::block_on(groq::transcribe(part, settings, rules))
    })
}
fn process_with(
    store: &Store,
    video: &Path,
    mut transcribe: impl FnMut(&Path, &Settings, &[Rule]) -> Result<String, String>,
) -> Result<Transcript, String> {
    let mut transcript = get(store, video)?;
    if transcript.preparation_failed {
        return Ok(transcript);
    }
    transcript.status = "processing".into();
    transcript.error.clear();
    store.put(&key(video), &transcript)?;
    let result = (|| -> Result<(), String> {
        let settings: Settings = store.get("settings")?;
        let rules: Vec<Rule> = store.get("rules")?;
        for part in transcript.parts.iter().skip(transcript.completed.len()) {
            if !recording_enabled(&store) {
                return Err(
                    "Transcripcion de videos desactivada. Se conservo el audio para reintentar."
                        .into(),
                );
            }
            let text = transcribe(part, &settings, &rules)?;
            transcript.completed.push(text);
            store.put(&key(video), &transcript)?;
        }
        transcript.text = transcript.completed.join("\n\n");
        if !transcript.text.trim().is_empty() {
            fs::write(video.with_extension("transcript.txt"), &transcript.text)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    transcript.text = transcript.completed.join("\n\n");
    transcript.status = if result.is_ok() { "ready" } else { "error" }.into();
    transcript.error = result.err().unwrap_or_default();
    store.put(&key(video), &transcript)?;
    Ok(transcript)
}
fn run(app: &tauri::AppHandle, video: &Path) -> Result<(), String> {
    let store = app.state::<Store>();
    let transcript = process(&store, video)?;
    // Never overwrite a clipboard which the user changed while Groq was working.
    if transcript.status == "ready" {
        // Sidecar and database IO must not hold the system clipboard open.
        let (files, text) = package(&store, &[video.to_path_buf()])?;
        let mut changed = false;
        if let Ok(_clipboard) = clipboard_win::Clipboard::new_attempts(10) {
            let mut current = Vec::<String>::new();
            use clipboard_win::Getter;
            if clipboard_win::formats::FileList
                .read_clipboard(&mut current)
                .is_ok()
                && current == vec![video.to_string_lossy().into_owned()]
            {
                write_open(&files, &text)?;
                changed = true;
            }
        }
        if changed {
            library::suppress_clipboard(app);
        }
    }
    Ok(())
}
pub fn start(app: &tauri::AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let store = app.state::<Store>();
        if let Ok(queue) = store.get::<Vec<PathBuf>>("video_transcript_queue") {
            for path in queue {
                if let Ok(mut t) = get(&store, &path) {
                    if t.status == "processing" {
                        t.status = "pending".into();
                        let _ = store.put(&key(&path), &t);
                    }
                }
            }
        }
        loop {
            std::thread::sleep(Duration::from_secs(1));
            if !recording_enabled(&store) {
                continue;
            }
            let pending = (|| -> Result<Vec<PathBuf>, String> {
                let _guard = GATE.lock().map_err(|_| "Transcripciones ocupadas")?;
                let previous: Vec<PathBuf> = store.get("video_transcript_queue")?;
                let queue: Vec<_> = previous
                    .iter()
                    .filter(|p| p.is_file() && get(&store, p).is_ok_and(|t| t.status == "pending"))
                    .cloned()
                    .collect();
                if queue != previous {
                    store.put("video_transcript_queue", &queue)?;
                }
                Ok(queue)
            })();
            if let Ok(queue) = pending {
                for path in queue {
                    if path.is_file() && get(&store, &path).is_ok_and(|t| t.status == "pending") {
                        if let Err(error) = run(&app, &path) {
                            let _ = store.event(&format!("Transcripcion de video: {error}"));
                        }
                    }
                }
            }
        }
    });
}
pub fn package(store: &Store, original: &[PathBuf]) -> Result<(Vec<String>, String), String> {
    let mut files = Vec::new();
    let mut texts = Vec::new();
    for path in original {
        if !path.is_file() {
            return Err("El archivo ya no existe".into());
        }
        files.push(path.to_string_lossy().into_owned());
        if enabled(store, "videoTranscriptAttachment") {
            let t = get(store, path)?;
            let sidecar = path.with_extension("transcript.txt");
            if t.status == "ready" && !t.text.trim().is_empty() {
                // Regenerate only our recorded transcript, never an arbitrary neighboring file.
                fs::write(&sidecar, &t.text).map_err(|e| e.to_string())?;
                files.push(sidecar.to_string_lossy().into_owned());
                texts.push(t.text);
            }
        }
    }
    Ok((files, texts.join("\n\n")))
}
fn write_open(files: &[String], text: &str) -> Result<(), String> {
    clipboard_win::raw::set_file_list(files).map_err(|e| e.to_string())?;
    if !text.is_empty() {
        clipboard_win::raw::set_string_with(text, clipboard_win::options::NoClear)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn copy_files(app: &tauri::AppHandle, paths: &[PathBuf]) -> Result<(), String> {
    let (files, text) = package(&app.state::<Store>(), paths)?;
    {
        let _clipboard = clipboard_win::Clipboard::new_attempts(20).map_err(|e| e.to_string())?;
        write_open(&files, &text)?;
    }
    library::suppress_clipboard(app);
    Ok(())
}
#[tauri::command]
pub fn video_transcript_action(
    app: tauri::AppHandle,
    path: String,
    action: String,
) -> Result<Transcript, String> {
    let store = app.state::<Store>();
    let path = PathBuf::from(path);
    let _guard = GATE.lock().map_err(|_| "Transcripciones ocupadas")?;
    let mut t = get(&store, &path)?;
    match action.as_str() {
        "status" => {}
        "copy" if t.status == "ready" => {
            use tauri_plugin_clipboard_manager::ClipboardExt;
            app.clipboard()
                .write_text(&t.text)
                .map_err(|e| e.to_string())?;
            library::suppress_clipboard(&app);
        }
        "retry" if t.status == "error" && !t.parts.is_empty() && !t.preparation_failed => {
            t.status = "pending".into();
            t.error.clear();
            let mut queue: Vec<PathBuf> = store.get("video_transcript_queue")?;
            if !queue.contains(&path) {
                queue.push(path.clone());
            }
            store.put_many(&[
                (
                    &key(&path),
                    serde_json::to_value(&t).map_err(|e| e.to_string())?,
                ),
                (
                    "video_transcript_queue",
                    serde_json::to_value(queue).map_err(|e| e.to_string())?,
                ),
            ])?;
        }
        _ => return Err("Transcripcion no disponible".into()),
    }
    // Internal audio paths and intermediate partial responses are not part of the UI contract.
    t.parts.clear();
    t.completed.clear();
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_segment_preserves_partial_text_and_retry_skips_completed_audio() {
        let root = root();
        let video = root.join("fixture.mp4");
        let store = Store::open(Path::new(":memory:")).unwrap();
        store.put(&key(&video), &Transcript {
            status: "pending".into(),
            parts: vec![root.join("first.wav"), root.join("second.wav")],
            ..Default::default()
        }).unwrap();
        let mut calls = 0;
        let failed = process_with(&store, &video, |_, _, _| {
            calls += 1;
            if calls == 1 { Ok("Primera parte".into()) } else { Err("Offline fixture failure".into()) }
        }).unwrap();
        assert_eq!(failed.status, "error");
        assert_eq!(failed.text, "Primera parte");
        assert_eq!(get(&store, &video).unwrap().text, failed.text);
        let mut retried = 0;
        let ready = process_with(&store, &video, |part, _, _| {
            retried += 1;
            assert_eq!(part, root.join("second.wav"));
            Ok("Segunda parte".into())
        }).unwrap();
        assert_eq!(retried, 1);
        assert_eq!(ready.status, "ready");
        assert!(ready.error.is_empty());
        assert_eq!(ready.text, "Primera parte\n\nSegunda parte");
        assert_eq!(fs::read_to_string(video.with_extension("transcript.txt")).unwrap(), ready.text);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn preparation_failure_never_calls_transcription_or_becomes_ready() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let video = Path::new("fixture.mp4");
        store.put(&key(video), &Transcript {
            status: "error".into(),
            error: "PCM conservado".into(),
            preparation_failed: true,
            parts: vec![PathBuf::from("partial.wav")],
            ..Default::default()
        }).unwrap();
        let result = process_with(&store, video, |_, _, _| panic!("must not upload partial preparation")).unwrap();
        assert_eq!(result.status, "error");
        assert_eq!(result.error, "PCM conservado");
    }
    #[test]
    #[ignore = "Opt-in: sends one short public MInDS-14 fixture to Groq; no desktop or user audio"]
    fn public_groq_video_transcript_pipeline() {
        let fixture=std::env::var("WHISPERA_PUBLIC_STT_FIXTURE").expect("Public fixture path required");
        let root=root();let video=root.join("public-qa.mp4");fs::write(&video,b"clipboard packaging fixture").unwrap();
        let part=root.join("part-0000");fs::create_dir_all(&part).unwrap();fs::copy(fixture,part.join("speech-0000.wav")).unwrap();
        let store=Store::open(Path::new(":memory:")).unwrap();
        let mut settings=Settings::default();settings.language="es".into();store.put("settings",&settings).unwrap();
        enqueue(&store,&video).unwrap();let started=std::time::Instant::now();let result=process(&store,&video).unwrap();
        assert_eq!(result.status,"ready","{}",result.error);assert!(!result.text.trim().is_empty());
        let (files,text)=package(&store,&[video]).unwrap();assert_eq!(files.len(),2);assert_eq!(text,result.text);
        println!("Live Groq + durable transcript + TXT attachment: {} words in {} ms",text.split_whitespace().count(),started.elapsed().as_millis());
        fs::remove_dir_all(root).unwrap();
    }
    fn root() -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("whispera-transcript-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn package_only_attaches_registered_ready_transcripts_and_respects_setting() {
        let root = root();
        let video = root.join("fixture.mp4");
        fs::write(&video, b"fixture").unwrap();
        let store = Store::open(Path::new(":memory:")).unwrap();
        fs::write(
            video.with_extension("transcript.txt"),
            "Untrusted neighboring text",
        )
        .unwrap();
        assert_eq!(package(&store, &[video.clone()]).unwrap().0.len(), 1);
        store
            .put(
                &key(&video),
                &Transcript {
                    status: "pending".into(),
                    text: "not ready".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(package(&store, &[video.clone()]).unwrap().0.len(), 1);
        store
            .put(
                &key(&video),
                &Transcript {
                    status: "ready".into(),
                    text: "Texto del microfono".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let (files, text) = package(&store, &[video.clone()]).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(text, "Texto del microfono");
        assert_eq!(fs::read_to_string(&files[1]).unwrap(), text);
        store
            .put(
                "library_settings",
                &serde_json::json!({"videoTranscriptAttachment":false}),
            )
            .unwrap();
        assert_eq!(package(&store, &[video]).unwrap().0.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn enqueue_orders_paused_segments_and_preserves_preparation_errors() {
        let root = root();
        let video = root.join("fixture.mp4");
        fs::write(&video, b"fixture").unwrap();
        for name in ["part-0001", "part-0000"] {
            let part = root.join(name);
            fs::create_dir_all(&part).unwrap();
            fs::write(part.join("speech-0000.wav"), b"fixture").unwrap();
        }
        let store = Store::open(Path::new(":memory:")).unwrap();
        enqueue(&store, &video).unwrap();
        let job = get(&store, &video).unwrap();
        assert_eq!(job.status, "pending");
        assert_eq!(job.parts.len(), 2);
        assert!(job.parts[0].to_string_lossy().contains("part-0000"));
        fs::write(root.join("part-0001/speech-error.txt"), "PCM conservado").unwrap();
        enqueue(&store, &video).unwrap();
        let job = get(&store, &video).unwrap();
        assert_eq!(job.status, "error");
        assert!(job.preparation_failed);
        assert_eq!(
            store
                .get::<Vec<PathBuf>>("video_transcript_queue")
                .unwrap()
                .len(),
            1
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn prepares_real_microphone_pcm_without_desktop_capture_or_network() {
        let root = root();
        let pcm = root.join("microphone.pcm");
        fs::write(&pcm, vec![0u8; 96000]).unwrap();
        let binary =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../node_modules/ffmpeg-static/ffmpeg.exe");
        prepare(&binary, &pcm, 48000, 1).unwrap();
        let wave = root.join("speech-0000.wav");
        let reader = hound::WavReader::open(wave).unwrap();
        assert_eq!(reader.spec().sample_rate, 16000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.duration(), 16000);
        assert!(pcm.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
