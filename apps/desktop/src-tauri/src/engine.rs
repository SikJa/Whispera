use crate::{
    audio::{self, Recorder, RecordingState},
    groq,
    storage::{Rule, Settings, Store},
    system_audio,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

pub struct Engine {
    pub recorder: Recorder,
    pub gate: Arc<Mutex<()>>,
    pub processing: AtomicBool,
    pub mute_journal: PathBuf,
    pub paste_target: Mutex<Option<crate::paste::Target>>,
    pub live: Mutex<std::collections::HashMap<String, crate::incremental::Live>>,
}
impl Engine {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let mute_journal = root.join("sound-restore.json");
        let recorder = Recorder::new(root.join("recordings"))?;
        if let Err(e) = system_audio::restore(&mute_journal) {
            recorder.change(|s| s.error = e);
        }
        let gate = Arc::new(Mutex::new(()));
        let weak_gate = Arc::downgrade(&gate);
        let view = Arc::downgrade(&recorder.state);
        let journal = mute_journal.clone();
        std::thread::Builder::new()
            .name("whispera-sound-recovery".into())
            .spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let (Some(gate), Some(view)) = (weak_gate.upgrade(), view.upgrade()) else {
                    break;
                };
                let Ok(_guard) = gate.lock() else {
                    break;
                };
                let Ok(mut state) = view.lock() else {
                    break;
                };
                if state.phase == "error" && state.muted && system_audio::restore(&journal).is_ok()
                {
                    state.muted = false;
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            recorder,
            gate,
            processing: AtomicBool::new(false),
            mute_journal,
            paste_target: Mutex::new(None),
            live: Mutex::new(std::collections::HashMap::new()),
        })
    }
    pub fn restore(&self) {
        match system_audio::restore(&self.mute_journal) {
            Ok(()) => self.recorder.change(|s| s.muted = false),
            Err(e) => self.recorder.change(|s| s.error = e),
        }
    }
}
#[tauri::command]
pub fn recording_state(engine: State<Engine>) -> RecordingState {
    engine.recorder.snapshot()
}
#[tauri::command]
pub fn pending_recordings(engine: State<Engine>) -> Result<Vec<audio::Recovery>, String> {
    let current = engine.recorder.snapshot();
    Ok(audio::recoveries(&engine.recorder.root)?
        .into_iter()
        .filter(|r| {
            r.id != current.session
                || !["recording", "paused", "processing"].contains(&current.phase.as_str())
        })
        .collect())
}
#[tauri::command]
pub async fn recording_action(action: String, app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || control(&app, &action))
        .await
        .map_err(|e| e.to_string())?
}
pub fn control(app: &tauri::AppHandle, action: &str) -> Result<(), String> {
    let engine = app.state::<Engine>();
    let _guard = engine.gate.lock().map_err(|_| "El motor esta ocupado")?;
    let state = engine.recorder.snapshot();
    if engine.processing.load(Ordering::SeqCst) {
        return Err("La transcripcion sigue en curso".into());
    }
    match action {
        "start" => {
            let settings: Settings = app.state::<Store>().get("settings")?;
            let rules: Vec<Rule> = app.state::<Store>().get("rules")?;
            if groq::entry()?.get_password().is_err() {
                return Err("Configura o importa la clave Groq antes de grabar".into());
            }
            if !["recording", "paused"].contains(&state.phase.as_str()) {
                system_audio::restore(&engine.mute_journal)?;
            }
            let id = engine.recorder.start()?;
            if settings.incremental_transcription {
                let dir = audio::session_dir(&engine.recorder.root, &id)?;
                if let Err(e) = crate::incremental::prepare(&dir, settings.clone(), rules) {
                    let _ = engine.recorder.stop();
                    return Err(e);
                }
                let stop = Arc::new(AtomicBool::new(false));
                let signal = stop.clone();
                let done = Arc::new(AtomicBool::new(false));
                let completed = done.clone();
                let view = engine.recorder.clone();
                let session = id.clone();
                let app_copy = app.clone();
                let task = tauri::async_runtime::spawn(async move {
                    let work = crate::incremental::run(dir, signal.clone());
                    tokio::pin!(work);
                    let result = tokio::select! {
                        result = &mut work => result,
                        _ = async {
                            loop {
                                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                let state = view.snapshot();
                                if state.session != session || state.phase == "error" {
                                    signal.store(true, Ordering::SeqCst);
                                    break;
                                }
                            }
                        } => work.await,
                    };
                    if result.is_err() && !signal.load(Ordering::SeqCst) {
                        let _ = app_copy.state::<Store>().event("Envio anticipado pendiente; el audio sigue guardado y se reintentara al detener.");
                    }
                    completed.store(true, Ordering::SeqCst);
                    result
                });
                let mut jobs = engine.live.lock().map_err(|_| "Cola ocupada")?;
                jobs.retain(|_, job| !job.done.load(Ordering::SeqCst));
                jobs.insert(id, crate::incremental::Live { stop, done, task });
            }
            *engine.paste_target.lock().map_err(|_| "Destino ocupado")? = crate::paste::capture();
            crate::sounds::play(&app.state::<Store>().get::<Settings>("settings")?, false);
        }
        "pause" => {
            engine.recorder.pause()?;
        }
        "mute" => {
            if !["recording", "paused"].contains(&state.phase.as_str()) {
                return Err("No hay grabacion activa".into());
            }
            if state.muted {
                system_audio::restore(&engine.mute_journal)?;
            } else {
                system_audio::mute(&engine.mute_journal)?;
            }
            engine.recorder.change(|s| s.muted = !state.muted);
        }
        "stop" | "save" | "cancel" => {
            let result = engine.recorder.stop();
            {
                let live = engine.live.lock().map_err(|_| "Cola ocupada")?;
                if let Some(job) = live.get(&state.session) {
                    job.stop.store(true, Ordering::SeqCst);
                }
                // An already-sent request may finish caching, but never publishes
                // or pastes anything after save/cancel.
            }
            engine.restore();
            let id = result?;
            crate::sounds::play(&app.state::<Store>().get::<Settings>("settings")?, true);
            if action == "stop" {
                schedule(app.clone(), &engine, id, true)?;
            } else {
                *engine.paste_target.lock().map_err(|_| "Destino ocupado")? = None;
                if action == "cancel" {
                    let dir = audio::session_dir(&engine.recorder.root, &id)?;
                    audio::write_new(&dir.join("cancelled.json"), b"{\"cancelled\":true}")?;
                    engine.recorder.change(|s| {
                        s.phase = "idle".into();
                        s.seconds = 0.;
                        s.text.clear();
                    });
                    if let Some(w) = app.get_webview_window("recorder") {
                        let _ = w.hide();
                    }
                }
            }
        }
        _ => return Err("Accion no valida".into()),
    }
    Ok(())
}
fn hide_after_completion(live_dictation: bool, state: &RecordingState) -> bool {
    live_dictation && state.phase == "done" && state.error.is_empty()
}
fn schedule(
    app: tauri::AppHandle,
    engine: &Engine,
    id: String,
    live_dictation: bool,
) -> Result<(), String> {
    if engine.processing.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una transcripcion en curso".into());
    }
    engine.recorder.change(|s| {
        s.phase = "processing".into();
        s.session = id.clone();
        s.error.clear();
        s.text.clear();
        s.progress = "Preparando audio".into();
    });
    tauri::async_runtime::spawn(async move {
        let result = process(&app, &id).await;
        let engine = app.state::<Engine>();
        match result {
            Ok(text) => {
                let settings = app
                    .state::<Store>()
                    .get::<Settings>("settings")
                    .unwrap_or_default();
                let target = engine.paste_target.lock().ok().and_then(|mut p| p.take());
                if live_dictation && settings.auto_paste {
                    let app_copy = app.clone();
                    let value = text.clone();
                    let pasted = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
                        let mut copied = false;
                        for _ in 0..3 {
                            if app_copy.clipboard().write_text(&value).is_ok() {
                                std::thread::sleep(std::time::Duration::from_millis(100));
                                if app_copy.clipboard().read_text().ok().as_deref() == Some(&value) { copied = true; break; }
                            }
                        }
                        if !copied { return Err("No se pudo verificar el portapapeles. El texto esta guardado.".into()); }
                        match target { Some(target) => crate::paste::restore_and_paste(target), None => Err("Texto copiado. No habia un campo de destino externo al iniciar.".into()) }
                    }).await.map_err(|e| e.to_string()).and_then(|r| r);
                    if let Err(e) = pasted {
                        engine.recorder.change(|s| s.error = e);
                    }
                }
                engine.recorder.change(|s| {
                    s.phase = "done".into();
                    s.text = text;
                    s.progress.clear();
                });
                if hide_after_completion(live_dictation, &engine.recorder.snapshot()) {
                    if let Some(window) = app.get_webview_window("recorder") {
                        let _ = window.hide();
                    }
                }
            }
            Err(e) => {
                let _ = app.state::<Store>().event(&e);
                engine.recorder.change(|s| {
                    s.phase = "error".into();
                    s.error = e;
                    s.progress.clear();
                });
            }
        }
        engine.processing.store(false, Ordering::SeqCst);
    });
    Ok(())
}
async fn process(app: &tauri::AppHandle, id: &str) -> Result<String, String> {
    let engine = app.state::<Engine>();
    let dir = audio::session_dir(&engine.recorder.root, id)?;
    let settings: Settings = app.state::<Store>().get("settings")?;
    let rules: Vec<Rule> = app.state::<Store>().get("rules")?;
    let live = {
        let mut pending = engine.live.lock().map_err(|_| "Cola ocupada")?;
        pending.remove(id)
    };
    if let Some(job) = live {
        job.stop.store(true, Ordering::SeqCst);
        // Missing/failed parts are retried below from the durable PCM.
        let _ = job.task.await;
    }
    let text = if crate::incremental::enabled(&dir) {
        crate::incremental::finish(&dir).await?
    } else {
        let target = dir.clone();
        let parts =
            tauri::async_runtime::spawn_blocking(move || audio::materialize(&target, 16_000_000))
                .await
                .map_err(|e| e.to_string())??;
        let mut texts = vec![];
        for (index, path) in parts.iter().enumerate() {
            engine
                .recorder
                .change(|s| s.progress = format!("Parte {} de {}", index + 1, parts.len()));
            let cache = dir.join(format!("part-{index:04}.txt"));
            let text = if cache.exists() {
                std::fs::read_to_string(&cache).map_err(|e| e.to_string())?
            } else {
                let original = path.clone();
                let trim = settings.trim_silence;
                let prepared = tauri::async_runtime::spawn_blocking(move || {
                    if trim {
                        audio::trimmed_copy(&original)
                    } else {
                        Ok(original)
                    }
                })
                .await
                .map_err(|e| e.to_string())??;
                let text = groq::transcribe_raw(&prepared, &settings, &rules).await?;
                audio::write_new(&cache, text.as_bytes())?;
                text
            };
            texts.push(text);
        }
        groq::corrections(&texts.join("\n\n"), &rules)
    };
    let result = dir.join("result.txt");
    if !result.exists() {
        audio::write_new(&result, text.as_bytes())?;
    }
    app.state::<Store>().append_once(id, &text)?;
    if settings.auto_copy {
        if app.clipboard().write_text(&text).is_err() {
            engine
                .recorder
                .change(|s| s.error = "Texto guardado; no se pudo copiar automaticamente.".into());
        }
    }
    if !dir.join("completed.json").exists() {
        audio::write_new(&dir.join("completed.json"), b"{\"done\":true}")?;
    }
    app.state::<Store>()
        .event("Grabacion transcrita y guardada")?;
    Ok(text)
}
#[tauri::command]
pub fn retry_recording(id: String, app: tauri::AppHandle) -> Result<(), String> {
    let engine = app.state::<Engine>();
    let _guard = engine.gate.lock().map_err(|_| "Motor ocupado")?;
    if ["recording", "paused", "processing"].contains(&engine.recorder.snapshot().phase.as_str()) {
        return Err("Finaliza la grabacion actual primero".into());
    }
    audio::session_dir(&engine.recorder.root, &id)?;
    schedule(app.clone(), &engine, id, false)
}
#[tauri::command]
pub fn copy_recording(app: tauri::AppHandle) -> Result<(), String> {
    let text = app.state::<Engine>().recorder.snapshot().text;
    if text.is_empty() {
        return Err("No hay texto para copiar".into());
    }
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn reveal_recording(id: String, app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let engine = app.state::<Engine>();
        let _guard = engine.gate.lock().map_err(|_| "Motor ocupado")?;
        if ["recording", "paused", "processing"]
            .contains(&engine.recorder.snapshot().phase.as_str())
        {
            return Err("Espera a que termine el proceso actual".into());
        }
        let dir = audio::session_dir(&engine.recorder.root, &id)?;
        audio::materialize(&dir, 16_000_000)?;
        std::process::Command::new("explorer.exe")
            .arg(dir)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
pub fn shutdown(app: &tauri::AppHandle) {
    let engine = app.state::<Engine>();
    let _guard = engine.gate.lock().unwrap();
    if ["recording", "paused"].contains(&engine.recorder.snapshot().phase.as_str()) {
        let _ = engine.recorder.stop();
    }
    engine.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_successful_live_dictation_auto_hides() {
        let mut state = RecordingState {
            phase: "done".into(),
            ..Default::default()
        };
        assert!(hide_after_completion(true, &state));
        assert!(!hide_after_completion(false, &state));
        state.error = "Clipboard failed".into();
        assert!(!hide_after_completion(true, &state));
        state.error.clear();
        for phase in ["idle", "recording", "paused", "processing", "error"] {
            state.phase = phase.into();
            assert!(!hide_after_completion(true, &state));
        }
    }
}
