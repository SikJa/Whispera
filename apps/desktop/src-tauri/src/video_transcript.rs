use crate::{
    groq, library, video_timing::{self, Cue, TimedText},
    storage::{Rule, Settings, Store},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Condvar, Mutex},
    time::Duration,
};
use tauri::Manager;

static GATE: Mutex<()> = Mutex::new(());
// Retain the one-second recovery poll, but start newly queued work immediately.
struct QueueWake { pending: Mutex<bool>, ready: Condvar }
impl QueueWake {
    const fn new() -> Self { Self { pending: Mutex::new(false), ready: Condvar::new() } }
    fn notify(&self) { let mut pending=self.pending.lock().unwrap_or_else(|e|e.into_inner()); *pending=true; self.ready.notify_one(); }
    fn wait(&self) {
        let pending=self.pending.lock().unwrap_or_else(|e|e.into_inner());
        let (mut pending,_)=self.ready.wait_timeout_while(pending,Duration::from_secs(1),|p|!*p).unwrap_or_else(|e|e.into_inner());
        *pending=false;
    }
}
static WORK_WAKE: QueueWake = QueueWake::new();
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Transcript {
    pub status: String,
    pub text: String,
    pub error: String,
    parts: Vec<PathBuf>,
    completed: Vec<String>,
    #[serde(default)]
    preparation_failed: bool,
    #[serde(default)]
    pub cues: Vec<Cue>,
    #[serde(default)]
    pub timing_complete: bool,
    #[serde(default)]
    part_offsets: Vec<Option<f64>>,
    #[serde(default)]
    completed_cues: Vec<Vec<Cue>>,
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
        .unwrap_or(false)
}
pub fn recording_enabled(store: &Store) -> bool {
    enabled(store, "transcribeVideo")
}

// Replay trimming transcribes the final MP4 soundtrack, never discarded intervals.
pub fn transcribe_selection(app:&tauri::AppHandle,video:&Path)->Result<(),String>{
    if !recording_enabled(&app.state::<Store>()) { return Ok(()); }
    let _guard=GATE.lock().map_err(|_|"Transcripciones ocupadas")?;
    let store=app.state::<Store>();
    let root=app.path().app_cache_dir().map_err(|e|e.to_string())?.join("trim-speech").join(uuid::Uuid::new_v4().to_string());
    let binary=crate::screen::ffmpeg(app)?;
    transcribe_track(&store,&root,&binary,video)
}
fn transcribe_track(store:&Store,root:&Path,binary:&Path,video:&Path)->Result<(),String>{
    let settings:Settings=store.get("settings")?;let rules:Vec<Rule>=store.get("rules")?;
    fs::create_dir_all(root).map_err(|e|e.to_string())?;
    let result=(||{
        let mut command=crate::replay::command(binary);
        command.args(["-hide_banner","-loglevel","error","-y","-i"]).arg(video).args(["-map","0:a:0","-vn","-ar","16000","-ac","1","-c:a","pcm_s16le","-f","segment","-segment_time","300"]).arg(root.join("speech-%04d.wav")).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        crate::replay::run_command(&mut command,Duration::from_secs(60))?;
        let mut parts:Vec<_>=fs::read_dir(&root).map_err(|e|e.to_string())?.filter_map(Result::ok).map(|entry|entry.path()).filter(|path|path.extension().is_some_and(|ext|ext=="wav")).collect();parts.sort();
        let mut texts=Vec::new();let mut cues=Vec::new();let mut offset=Some(0.);let mut complete=true;
        for part in parts {
            let value=tauri::async_runtime::block_on(groq::request_segments(&part,&settings,&rules))?;
            let timed=video_timing::parse(&value,&rules,video_timing::wav_seconds(&part))?;
            let aligned=offset_cues(&timed.cues,offset);
            if !timed.text.trim().is_empty() && aligned.is_empty() {complete=false;}
            cues.extend(aligned);texts.push(timed.text);
            offset=offset.zip(video_timing::wav_seconds(&part)).map(|(at,duration)|at+duration);
        }
        let text=texts.join("\n\n");fs::write(video.with_extension("transcript.txt"),video_timing::render(&text,&cues,complete)).map_err(|e|e.to_string())?;
        store.put(&key(video),&Transcript{status:"ready".into(),text,cues,timing_complete:complete,..Default::default()})
    })();
    let _=fs::remove_dir_all(root);
    if let Err(error)=&result{let _=store.put(&key(video),&Transcript{status:"error".into(),error:error.clone(),..Default::default()});}
    result
}

// The source is the microphone PCM, never the mixed desktop soundtrack.
pub fn prepare(binary: &Path, pcm: &Path, rate: u32, channels: u16) -> Result<(), String> {
    let output = pcm
        .parent()
        .ok_or("Audio sin directorio")?
        .join("speech-%04d.wav");
    let mut command = std::process::Command::new(binary);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
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
    let mut base=Some(0.);
    for directory in directories {
        if let Ok(error) = fs::read_to_string(directory.join("speech-error.txt")) {
            transcript.status = "error".into();
            transcript.error = error;
            transcript.preparation_failed = true;
        }
        let mut parts: Vec<_> = fs::read_dir(&directory)
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
        let mut offset=base;
        for part in &parts {
            transcript.part_offsets.push(offset);
            offset=offset.zip(video_timing::wav_seconds(part)).map(|(at,duration)|at+duration);
        }
        base=base.zip(video_timing::part_seconds(&directory)).map(|(at,duration)|at+duration);
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
    ])?;
    WORK_WAKE.notify();
    Ok(())
}
fn process(store: &Store, video: &Path) -> Result<Transcript, String> {
    process_timed_with(store, video, |part, settings, rules| {
        let value=tauri::async_runtime::block_on(groq::request_segments(part, settings, rules))?;
        video_timing::parse(&value,rules,video_timing::wav_seconds(part))
    })
}
#[cfg(test)]
fn process_with(
    store: &Store,
    video: &Path,
    mut transcribe: impl FnMut(&Path, &Settings, &[Rule]) -> Result<String, String>,
) -> Result<Transcript, String> {
    process_timed_with(store,video,|part,settings,rules|transcribe(part,settings,rules).map(|text|TimedText{text,cues:Vec::new()}))
}
fn offset_cues(cues:&[Cue],offset:Option<f64>)->Vec<Cue> {
    let Some(offset)=offset.filter(|at|at.is_finite() && *at>=0.) else{return Vec::new();};
    cues.iter().map(|cue|Cue{start:cue.start+offset,end:cue.end+offset,text:cue.text.clone()}).collect()
}
fn collect_timing(transcript:&mut Transcript) {
    transcript.cues=transcript.completed_cues.iter().flatten().cloned().collect();
    transcript.timing_complete=transcript.completed.len()==transcript.parts.len()
        && transcript.completed.iter().enumerate().all(|(index,text)|text.trim().is_empty() || transcript.completed_cues.get(index).is_some_and(|c|!c.is_empty()));
}
fn process_timed_with(
    store: &Store,
    video: &Path,
    mut transcribe: impl FnMut(&Path, &Settings, &[Rule]) -> Result<TimedText, String>,
) -> Result<Transcript, String> {
    let mut transcript = get(store, video)?;
    if transcript.preparation_failed {
        return Ok(transcript);
    }
    transcript.status = "processing".into();
    transcript.error.clear();
    transcript.completed_cues.resize(transcript.completed.len(),Vec::new());
    store.put(&key(video), &transcript)?;
    let result = (|| -> Result<(), String> {
        let settings: Settings = store.get("settings")?;
        let rules: Vec<Rule> = store.get("rules")?;
        for index in transcript.completed.len()..transcript.parts.len() {
            let part=&transcript.parts[index];
            if !recording_enabled(&store) {
                return Err(
                    "Transcripcion de videos desactivada. Se conservo el audio para reintentar."
                        .into(),
                );
            }
            let timed = transcribe(part, &settings, &rules)?;
            transcript.completed_cues.push(offset_cues(&timed.cues,transcript.part_offsets.get(index).copied().flatten()));
            transcript.completed.push(timed.text);
            store.put(&key(video), &transcript)?;
        }
        transcript.text = transcript.completed.join("\n\n");
        collect_timing(&mut transcript);
        if !transcript.text.trim().is_empty() {
            fs::write(video.with_extension("transcript.txt"), video_timing::render(&transcript.text,&transcript.cues,transcript.timing_complete))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    transcript.text = transcript.completed.join("\n\n");
    collect_timing(&mut transcript);
    transcript.status = if result.is_ok() { "ready" } else { "error" }.into();
    transcript.error = result.err().unwrap_or_default();
    store.put(&key(video), &transcript)?;
    Ok(transcript)
}
fn run(app: &tauri::AppHandle, video: &Path) -> Result<(), String> {
    let _updating=crate::updates::work(app)?;
    let store = app.state::<Store>();
    let transcript = process(&store, video)?;
    // Never overwrite a clipboard which the user changed while Groq was working.
    if transcript.status == "ready" {
        // Sidecar and database IO must not hold the system clipboard open.
        let (files, text) = package(&store, &[video.to_path_buf()])?;
        let shared_original = share_files(app, &[video.to_string_lossy().into_owned()])?;
        let files = share_files(app, &files)?;
        #[cfg(windows)]
        {
            let mut changed = false;
            if let Ok(_clipboard) = clipboard_win::Clipboard::new_attempts(10) {
                let mut current = Vec::<String>::new();
                use clipboard_win::Getter;
                if clipboard_win::formats::FileList
                    .read_clipboard(&mut current)
                    .is_ok()
                    && (current == vec![video.to_string_lossy().into_owned()] || current == shared_original)
                {
                    write_open(&files, &text)?;
                    changed = true;
                }
            }
            if changed {
                library::suppress_clipboard(app);
            }
        }
        #[cfg(target_os = "macos")]
        {
            validate_files(&files)?;
            let expected = vec![vec![video.to_string_lossy().into_owned()], shared_original];
            if let Some(sequence) = library::clipboard::write_files(&files, Some(&expected))? {
                library::suppress_clipboard_sequence(app, Some(sequence));
            }
            let _ = text; // Text stays in the TXT attachment, not a competing pasteboard type.
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
        WORK_WAKE.notify();
        loop {
            WORK_WAKE.wait();
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
                let content=video_timing::render(&t.text,&t.cues,t.timing_complete);
                if fs::read_to_string(&sidecar).ok().as_deref() != Some(content.as_str()) {
                    fs::write(&sidecar, &content).map_err(|e| e.to_string())?;
                }
                files.push(sidecar.to_string_lossy().into_owned());
                texts.push(t.text);
            }
        }
    }
    Ok((files, texts.join("\n\n")))
}
pub(crate) fn write_open(files: &[String], _text: &str) -> Result<(), String> {
    validate_files(files)?;
    // set_file_list preserves existing formats by default. Replace the previous
    // clipboard item first so paste targets cannot pick an unrelated PNG or HTML.
    #[cfg(windows)]
    {
        clipboard_win::raw::empty().map_err(|e| e.to_string())?;
        clipboard_win::raw::set_file_list(files).map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    { library::clipboard::write_files(files, None)?; }
    // File paste targets may prefer CF_UNICODETEXT over CF_HDROP. Keep the
    // transcript in the TXT attachment; explicit "copy text" remains separate.
    Ok(())
}
fn validate_files(files: &[String]) -> Result<(), String> {
    if files.is_empty() || files.iter().any(|file| !Path::new(file).is_file()) {
        return Err("El archivo ya no existe. No se modifico el portapapeles.".into());
    }
    Ok(())
}
pub fn copy_files(app: &tauri::AppHandle, paths: &[PathBuf]) -> Result<(), String> {
    let (files, text) = package(&app.state::<Store>(), paths)?;
    let files = share_files(app, &files)?;
    copy_shared_files(app, &files, &text)
}
fn copy_shared_files(app: &tauri::AppHandle, files: &[String], text: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let _clipboard = clipboard_win::Clipboard::new_attempts(20).map_err(|e| e.to_string())?;
        write_open(files, text)?;
    }
    #[cfg(windows)]
    library::suppress_clipboard(app);
    #[cfg(target_os = "macos")]
    {
        validate_files(files)?;
        let sequence = library::clipboard::write_files(files, None)?;
        library::suppress_clipboard_sequence(app, sequence);
        let _ = text;
    }
    Ok(())
}
// Export durable, physical files for clipboard and external drag recipients.
// This also avoids MSIX AppData virtualization on Windows. macOS uses Movies.
pub(crate) fn share_files(app: &tauri::AppHandle, files: &[String]) -> Result<Vec<String>, String> {
    let root = app.path().video_dir().map_err(|e| e.to_string())?
        .join("Whispera").join("Compartir");
    share_files_at(&root, files)
}
fn share_files_at(root: &Path, files: &[String]) -> Result<Vec<String>, String> {
    use std::hash::{Hash, Hasher};
    static EXPORT_GATE: Mutex<()> = Mutex::new(());
    let _guard = EXPORT_GATE.lock().map_err(|_| "Exportacion ocupada")?;
    validate_files(files)?;
    files.iter().map(|file| {
        let source = Path::new(file);
        if source.starts_with(root) { return Ok(file.clone()); }
        let metadata = fs::metadata(source).map_err(|e| e.to_string())?;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut hash);
        metadata.len().hash(&mut hash);
        metadata.modified().map_err(|e| e.to_string())?.hash(&mut hash);
        let directory = root.join(format!("{:016x}", hash.finish()));
        let target = directory.join(source.file_name().ok_or("Archivo sin nombre")?);
        if !fs::metadata(&target).is_ok_and(|m| m.is_file() && m.len() == metadata.len()) {
            fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
            let pending = directory.join(format!("{}.partial", uuid::Uuid::new_v4()));
            let result = (|| -> Result<(), String> {
                let copied = fs::copy(source, &pending).map_err(|e| e.to_string())?;
                if copied != metadata.len() { return Err("El archivo cambio mientras se copiaba. Reintenta.".into()); }
                fs::rename(&pending, &target).map_err(|e| e.to_string())
            })();
            if result.is_err() { let _ = fs::remove_file(&pending); }
            result?;
        }
        Ok(target.to_string_lossy().into_owned())
    }).collect()
}
// A selected replay explicitly includes its own transcription, independently of
// the normal recording attachment preference. Only our stored result is trusted.
pub fn copy_selection(app:&tauri::AppHandle,path:&Path)->Result<(),String>{
    let mut files=vec![path.to_string_lossy().into_owned()];let mut text=String::new();
    if let Ok(result)=get(&app.state::<Store>(),path){if result.status=="ready"{
        let sidecar=path.with_extension("transcript.txt");
        if sidecar.is_file(){files.push(sidecar.to_string_lossy().into_owned());text=result.text;}
    }}
    let files=share_files(app,&files)?;
    copy_shared_files(app,&files,&text)
}
#[tauri::command]
pub async fn video_transcript_action(
    app: tauri::AppHandle,
    path: String,
    action: String,
) -> Result<Transcript, String> {
    tauri::async_runtime::spawn_blocking(move || transcript_action(app, path, action)).await.map_err(|e| e.to_string())?
}
fn transcript_action(app: tauri::AppHandle, path: String, action: String) -> Result<Transcript, String> {
    let store = app.state::<Store>();
    let path = PathBuf::from(path);
    let _guard = GATE.lock().map_err(|_| "Transcripciones ocupadas")?;
    let mut t = get(&store, &path)?;
    match action.as_str() {
        "status" => {}
        "copy" if t.status == "ready" => {
            use tauri_plugin_clipboard_manager::ClipboardExt;
            app.clipboard()
                .write_text(video_timing::render(&t.text,&t.cues,t.timing_complete))
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
            WORK_WAKE.notify();
        }
        _ => return Err("Transcripcion no disponible".into()),
    }
    // Internal audio paths and intermediate partial responses are not part of the UI contract.
    t.parts.clear();
    t.completed.clear();
    t.part_offsets.clear();
    t.completed_cues.clear();
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wave(path:&Path,seconds:f64) {
        let mut out=hound::WavWriter::create(path,hound::WavSpec{channels:1,sample_rate:16000,bits_per_sample:16,sample_format:hound::SampleFormat::Int}).unwrap();
        for _ in 0..(seconds*16000.) as usize {out.write_sample(0i16).unwrap();}out.finalize().unwrap();
    }
    fn timed(text:&str)->TimedText {TimedText{text:text.into(),cues:vec![Cue{start:0.2,end:0.8,text:text.into()}]}}
    #[test]
    fn paused_parts_use_encoded_duration_and_chunks_use_actual_audio_duration() {
        let root=root();let video=root.join("fixture.mp4");fs::write(&video,b"fixture").unwrap();
        let first=root.join("part-0000");let second=root.join("part-0001");fs::create_dir(&first).unwrap();fs::create_dir(&second).unwrap();
        wave(&first.join("speech-0000.wav"),1.25);wave(&first.join("speech-0001.wav"),0.75);wave(&second.join("speech-0000.wav"),1.);
        fs::write(first.join("speech-timing.json"),r#"{"videoSeconds":2.345}"#).unwrap();
        let store=Store::open(Path::new(":memory:")).unwrap();
        store.put("library_settings", &serde_json::json!({"transcribeVideo":true,"videoTranscriptAttachment":true})).unwrap();enqueue(&store,&video).unwrap();
        assert_eq!(get(&store,&video).unwrap().part_offsets,vec![Some(0.),Some(1.25),Some(2.345)]);
        let result=process_timed_with(&store,&video,|_,_,_|Ok(timed("Frase."))).unwrap();
        assert!(result.timing_complete);assert_eq!(result.cues.len(),3);assert!((result.cues[2].start-2.545).abs()<0.000001);
        let (files,_)=package(&store,&[video.clone()]).unwrap();let text=fs::read_to_string(&files[1]).unwrap();assert!(text.contains("00:00:02.545"));
        // Re-packaging must not strip the timestamps back to plain text.
        assert_eq!(text,fs::read_to_string(package(&store,&[video]).unwrap().0[1].clone()).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn timed_retry_preserves_completed_cues_without_uploading_audio_twice() {
        let root=root();let video=root.join("retry.mp4");fs::write(&video,b"fixture").unwrap();let store=Store::open(Path::new(":memory:")).unwrap();
        store.put("library_settings", &serde_json::json!({"transcribeVideo":true})).unwrap();
        store.put(&key(&video),&Transcript{status:"pending".into(),parts:vec![root.join("first.wav"),root.join("second.wav")],part_offsets:vec![Some(0.),Some(7.5)],..Default::default()}).unwrap();
        let mut calls=0;let failed=process_timed_with(&store,&video,|_,_,_|{calls+=1;if calls==1 {Ok(timed("Primera."))}else{Err("offline".into())}}).unwrap();
        assert_eq!(failed.status,"error");assert_eq!(failed.cues.len(),1);assert!(!failed.timing_complete);
        let mut retries=0;let result=process_timed_with(&store,&video,|path,_,_|{retries+=1;assert!(path.ends_with("second.wav"));Ok(timed("Segunda."))}).unwrap();
        assert_eq!(retries,1);assert!(result.timing_complete);assert_eq!(result.cues[0],failed.cues[0]);assert_eq!(result.cues[1].start,7.7);
        assert!(fs::read_to_string(video.with_extension("transcript.txt")).unwrap().contains("00:00:07.700"));fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn legacy_cache_and_missing_part_alignment_keep_complete_plain_text() {
        let value=serde_json::json!({"status":"ready","text":"Anterior.","error":"","parts":[],"completed":[]});
        let old:Transcript=serde_json::from_value(value).unwrap();assert!(!old.timing_complete);assert!(old.cues.is_empty());
        let root=root();let video=root.join("legacy.mp4");let store=Store::open(Path::new(":memory:")).unwrap();
        store.put("library_settings", &serde_json::json!({"transcribeVideo":true})).unwrap();
        store.put(&key(&video),&Transcript{parts:vec![root.join("first.wav")],..Default::default()}).unwrap();
        let result=process_timed_with(&store,&video,|_,_,_|Ok(timed("Texto completo."))).unwrap();assert_eq!(result.status,"ready");assert!(!result.timing_complete);
        assert_eq!(fs::read_to_string(video.with_extension("transcript.txt")).unwrap(),"Texto completo.");fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn queue_notifications_survive_before_wait_and_during_processing() {
        let signal=std::sync::Arc::new(QueueWake::new());signal.notify();let at=std::time::Instant::now();signal.wait();assert!(at.elapsed()<Duration::from_millis(800));
        let other=signal.clone();let (tx,rx)=std::sync::mpsc::channel();let worker=std::thread::spawn(move||{other.wait();tx.send(()).unwrap();});
        signal.notify();rx.recv_timeout(Duration::from_secs(2)).unwrap();worker.join().unwrap();
        signal.notify();signal.notify();let at=std::time::Instant::now();signal.wait();assert!(at.elapsed()<Duration::from_millis(800));
    }
    #[test]
    #[ignore = "Measured scheduler comparison; no recording, credentials or network"]
    fn measure_polling_and_notification_delay() {
        let mut rows=Vec::new();
        for phase in [0u64,250,500,750] {
            let signal=std::sync::Arc::new(QueueWake::new());let other=signal.clone();let (tx,rx)=std::sync::mpsc::channel();
            let worker=std::thread::spawn(move||{other.wait();tx.send(std::time::Instant::now()).unwrap();});
            std::thread::sleep(Duration::from_millis(phase));let start=std::time::Instant::now();signal.notify();let elapsed=rx.recv_timeout(Duration::from_secs(2)).unwrap().saturating_duration_since(start);worker.join().unwrap();
            let start=std::time::Instant::now();std::thread::sleep(Duration::from_millis(1000-phase));let previous=start.elapsed();
            rows.push(serde_json::json!({"phaseMs":phase,"oldPollMs":previous.as_secs_f64()*1000.,"notificationMs":elapsed.as_secs_f64()*1000.}));
        }
        let path=std::env::var_os("WHISPERA_WAKE_RESULTS").expect("Result file required");fs::write(path,serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
    }
    #[test]
    #[ignore = "Opt-in: real Groq, explicitly supplied synthetic 20s fixture only; no desktop, microphone or clipboard"]
    fn synthetic_timed_video_and_trim_pipeline() {
        crate::lossless_upload::configure(Path::new(env!("CARGO_MANIFEST_DIR")).join("../node_modules/ffmpeg-static/ffmpeg.exe"));
        let fixture=PathBuf::from(std::env::var_os("WHISPERA_TIMING_FIXTURES").expect("Synthetic fixture directory"));
        let reference:serde_json::Value=serde_json::from_slice(&fs::read(fixture.join("reference.json")).unwrap()).unwrap();assert_eq!(reference["syntheticOnly"],true);
        let root=fixture.join(format!("pipeline-{}",uuid::Uuid::new_v4()));fs::create_dir(&root).unwrap();let video=root.join("synthetic.mp4");
        let binary=Path::new(env!("CARGO_MANIFEST_DIR")).join("../node_modules/ffmpeg-static/ffmpeg.exe");let mut command=crate::replay::command(&binary);
        command.args(["-hide_banner","-loglevel","error","-y","-f","lavfi","-i","color=size=320x180:rate=30:color=navy","-i"]).arg(fixture.join("synthetic-20s.wav")).args(["-t","20","-c:v","libx264","-preset","ultrafast","-c:a","aac"]).arg(&video).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        crate::replay::run_command(&mut command,Duration::from_secs(30)).unwrap();
        let part=root.join("part-0000");fs::create_dir(&part).unwrap();fs::copy(fixture.join("synthetic-20s.wav"),part.join("speech-0000.wav")).unwrap();video_timing::record_part_duration(&video,&part);
        let store=Store::open(Path::new(":memory:")).unwrap();
        store.put("library_settings", &serde_json::json!({"transcribeVideo":true,"videoTranscriptAttachment":true})).unwrap();let mut settings=Settings::default();settings.language="es".into();store.put("settings",&settings).unwrap();
        let mut rows=Vec::new();
        for trial in 0..3 {
            let at=std::time::Instant::now();enqueue(&store,&video).unwrap();let result=process(&store,&video).unwrap();let (files,_)=package(&store,&[video.clone()]).unwrap();
            let elapsed=at.elapsed().as_secs_f64()*1000.;assert_eq!(result.status,"ready","{}",result.error);assert!(result.timing_complete);assert_eq!(files.len(),2);assert!(!result.cues.is_empty());
            let saved=fs::read_to_string(&files[1]).unwrap();assert!(saved.starts_with('['));
            rows.push(serde_json::json!({"trial":trial,"enqueueAsrExportMs":elapsed,"cues":result.cues,"text":result.text}));
        }
        let trimmed=root.join("selected.mp4");crate::video_trim::trim(&binary,&video,&trimmed,2.,12.).unwrap();let at=std::time::Instant::now();
        transcribe_track(&store,&root.join("trim-audio"),&binary,&trimmed).unwrap();let result=get(&store,&trimmed).unwrap();assert_eq!(result.status,"ready");assert!(result.timing_complete);assert!(result.cues.iter().all(|cue|cue.start>=0. && cue.end<=10.25));
        rows.push(serde_json::json!({"trimmed":true,"extractAsrExportMs":at.elapsed().as_secs_f64()*1000.,"cues":result.cues,"text":result.text}));
        fs::write(fixture.join("pipeline-results.json"),serde_json::to_vec_pretty(&serde_json::json!({"directory":root,"measurements":rows})).unwrap()).unwrap();
    }
    #[test]
    fn sharing_uses_external_files_reuses_exports_and_preserves_sources() {
        let root = root();
        let source = root.join("private");
        fs::create_dir_all(&source).unwrap();
        let video = source.join("video.mp4");
        let text = source.join("video.transcript.txt");
        fs::write(&video, b"video fixture").unwrap();
        fs::write(&text, b"text fixture").unwrap();
        let originals = vec![video.to_string_lossy().into_owned(), text.to_string_lossy().into_owned()];
        let shared = root.join("Videos/Whispera/Compartir");
        let exports = share_files_at(&shared, &originals).unwrap();
        assert_eq!(exports, share_files_at(&shared, &originals).unwrap());
        assert_eq!(exports, share_files_at(&shared, &exports).unwrap());
        for (original, export) in originals.iter().zip(&exports) {
            assert!(Path::new(export).starts_with(&shared));
            assert_eq!(fs::read(original).unwrap(), fs::read(export).unwrap());
        }
        fs::write(&text, b"updated transcript fixture").unwrap();
        let changed = share_files_at(&shared, &originals).unwrap();
        assert_eq!(changed[0], exports[0]);
        assert_ne!(changed[1], exports[1]);
        assert!(share_files_at(&shared, &[source.join("missing.mp4").to_string_lossy().into_owned()]).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[cfg(windows)]
    #[ignore = "Run alone: uses a private Windows window station, never the interactive clipboard"]
    fn private_station_clipboard_replaces_stale_formats() {
        use std::ffi::c_void;
        #[link(name = "user32")]
        extern "system" {
            fn CreateWindowStationW(name: *const u16, flags: u32, access: u32, security: *const c_void) -> *mut c_void;
            fn GetProcessWindowStation() -> *mut c_void;
            fn SetProcessWindowStation(station: *mut c_void) -> i32;
            fn CloseWindowStation(station: *mut c_void) -> i32;
        }
        struct Station(*mut c_void, *mut c_void);
        impl Drop for Station {
            fn drop(&mut self) {
                unsafe { SetProcessWindowStation(self.0); CloseWindowStation(self.1); }
            }
        }
        let _station = unsafe {
            let original = GetProcessWindowStation();
            let private = CreateWindowStationW(std::ptr::null(), 0, 0x37f, std::ptr::null());
            assert!(!private.is_null(), "Cannot create isolated clipboard station");
            if SetProcessWindowStation(private) == 0 {
                CloseWindowStation(private);
                panic!("Cannot select isolated station; no clipboard access attempted");
            }
            Station(original, private)
        };
        let root = root();
        let video = root.join("fixture.mp4");
        fs::write(&video, b"fixture").unwrap();
        let files = vec![video.to_string_lossy().into_owned()];
        {
            let _clipboard = clipboard_win::Clipboard::new_attempts(5).unwrap();
            let png = clipboard_win::raw::register_format("PNG").unwrap().get();
            clipboard_win::raw::set(png, b"old image format").unwrap();
            clipboard_win::raw::set_string_with("old text", clipboard_win::options::NoClear).unwrap();
            write_open(&files, "").unwrap();
            assert!(!clipboard_win::raw::is_format_avail(png));
            assert!(!clipboard_win::raw::is_format_avail(13));
            let mut actual = Vec::new();
            clipboard_win::raw::get_file_list(&mut actual).unwrap();
            assert_eq!(actual, files);
            write_open(&files, "new transcript").unwrap();
            assert!(!clipboard_win::raw::is_format_avail(13));
            let mut actual = Vec::new();
            clipboard_win::raw::get_file_list(&mut actual).unwrap();
            assert_eq!(actual, files);
            assert!(!clipboard_win::raw::is_format_avail(png));
            assert!(write_open(&[root.join("missing.mp4").to_string_lossy().into_owned()], "").is_err());
            let mut preserved = Vec::new();
            clipboard_win::raw::get_file_list(&mut preserved).unwrap();
            assert_eq!(preserved, files);
        }
        // Exercise the real image publisher on this private station too.
        let pixels=image::RgbaImage::from_raw(2,2,vec![10,20,30,255,40,50,60,128,70,80,90,0,100,110,120,64]).unwrap();
        let png=crate::screen_capture::rgba_png(&pixels).unwrap();let dib=crate::image_clipboard::bitmap(&pixels).unwrap();
        let (ready,held)=std::sync::mpsc::channel();
        let holder=std::thread::spawn(move||{
            let clipboard=clipboard_win::Clipboard::new().unwrap();ready.send(()).unwrap();
            std::thread::sleep(Duration::from_millis(15));drop(clipboard);
        });
        held.recv().unwrap();
        crate::image_clipboard::write(&png,&dib).unwrap();
        holder.join().unwrap();
        {
            let _clipboard=clipboard_win::Clipboard::new_attempts(5).unwrap();
            let format=clipboard_win::register_format("PNG").unwrap().get();
            let mut actual=vec![0;png.len()];assert_eq!(clipboard_win::raw::get(format,&mut actual).unwrap(),png.len());
            assert_eq!(image::load_from_memory(&actual).unwrap().into_rgba8(),pixels);
            let mut actual=vec![0;dib.len()];assert_eq!(clipboard_win::raw::get(17,&mut actual).unwrap(),dib.len());assert_eq!(actual,dib);
            assert!(!clipboard_win::raw::is_format_avail(15));assert!(!clipboard_win::raw::is_format_avail(13));
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn clipboard_payload_rejects_missing_files_before_replacing_clipboard() {
        let root = root();
        let video = root.join("fixture.mp4");
        fs::write(&video, b"fixture").unwrap();
        let file = video.to_string_lossy().into_owned();
        assert!(validate_files(&[file.clone()]).is_ok());
        assert!(validate_files(&[]).is_err());
        assert!(validate_files(&[root.to_string_lossy().into_owned()]).is_err());
        assert!(validate_files(&[file.clone(), root.join("missing.txt").to_string_lossy().into_owned()]).is_err());
        fs::remove_file(video).unwrap();
        // This path fails before any system clipboard API is called.
        assert!(write_open(&[file], "text").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_segment_preserves_partial_text_and_retry_skips_completed_audio() {
        let root = root();
        let video = root.join("fixture.mp4");
        let store = Store::open(Path::new(":memory:")).unwrap();
        store.put("library_settings", &serde_json::json!({"transcribeVideo":true})).unwrap();
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
        store.put("library_settings", &serde_json::json!({"transcribeVideo":true,"videoTranscriptAttachment":true})).unwrap();
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
        store.put("library_settings", &serde_json::json!({"videoTranscriptAttachment":true})).unwrap();
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
        let binary = Path::new(env!("CARGO_MANIFEST_DIR")).join(if cfg!(windows) {
            "../node_modules/ffmpeg-static/ffmpeg.exe"
        } else { "../node_modules/ffmpeg-static/ffmpeg" });
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

#[cfg(all(test, windows))]
mod trim_live_tests {
    use super::*;
    #[test]
    #[ignore = "Opt-in: transcribes only local synthetic speech with Groq and tests the real Windows clipboard"]
    fn selected_clip_transcribes_only_retained_speech_and_copies_both_files(){
        let fixture=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.local/replay-qa");
        let before=fixture.join("before.wav");let selected=fixture.join("selected.wav");assert!(before.is_file()&&selected.is_file());
        let a=hound::WavReader::open(&before).unwrap();let start=a.duration() as f64/a.spec().sample_rate as f64;
        let b=hound::WavReader::open(&selected).unwrap();let end=start+b.duration() as f64/b.spec().sample_rate as f64;
        let root=std::env::temp_dir().join(format!("whispera-speech-qa-{}",uuid::Uuid::new_v4()));fs::create_dir(&root).unwrap();let source=root.join("source.mp4");let output=root.join("selected.mp4");
        let binary=Path::new(env!("CARGO_MANIFEST_DIR")).join("../node_modules/ffmpeg-static/ffmpeg.exe");let mut command=crate::replay::command(&binary);
        command.args(["-hide_banner","-loglevel","error","-y","-f","lavfi","-i","color=size=320x180:rate=30","-i"]).arg(&before).arg("-i").arg(&selected).args(["-filter_complex","[1:a][2:a]concat=n=2:v=0:a=1[a]","-map","0:v:0","-map","[a]","-t"]).arg(end.to_string()).args(["-c:v","libx264","-preset","ultrafast","-c:a","aac"]).arg(&source).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());crate::replay::run_command(&mut command,Duration::from_secs(30)).unwrap();
        crate::video_trim::trim(&binary,&source,&output,start,end).unwrap();
        let store=Store::open(Path::new(":memory:")).unwrap();let mut settings=Settings::default();settings.language="en".into();store.put("settings",&settings).unwrap();store.put("rules",&Vec::<Rule>::new()).unwrap();
        transcribe_track(&store,&root.join("speech"),&binary,&output).unwrap();let text=get(&store,&output).unwrap().text.to_lowercase();assert!(text.contains("camera"),"Selected message missing: {text}");assert!(!text.contains("banana")&&!text.contains("discarded"),"Discarded words survived: {text}");
        let files=vec![output.to_string_lossy().into_owned(),output.with_extension("transcript.txt").to_string_lossy().into_owned()];
        {let _clipboard=clipboard_win::Clipboard::new_attempts(20).unwrap();let mut old=Vec::new();let _=clipboard_win::raw::get_string(&mut old);let mut old_files=Vec::<String>::new();let _=clipboard_win::raw::get_file_list(&mut old_files);write_open(&files,&text).unwrap();let mut actual=Vec::<String>::new();clipboard_win::raw::get_file_list(&mut actual).unwrap();assert_eq!(actual,files);if !old_files.is_empty(){write_open(&old_files,&String::from_utf8_lossy(&old)).unwrap();}else{clipboard_win::raw::set_string(&String::from_utf8_lossy(&old)).unwrap();}}
        eprintln!("LIVE QA passed: discarded speech excluded, selected speech transcribed, MP4 plus TXT on Windows clipboard");fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod consent_tests {
    use super::*;
    #[test] fn video_speech_and_attachments_require_explicit_opt_in() {
        let store=Store::open(Path::new(":memory:")).unwrap();
        assert!(!recording_enabled(&store)); assert!(!enabled(&store,"videoTranscriptAttachment"));
        store.put("library_settings", &serde_json::json!({"transcribeVideo":true})).unwrap();
        assert!(recording_enabled(&store));
        store.put("library_settings", &serde_json::json!({"transcribeVideo":false})).unwrap();
        assert!(!recording_enabled(&store));
    }
}
