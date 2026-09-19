use crate::{
    engine::Engine,
    storage::{Settings, Store},
};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Manager, State};
pub struct Health {
    beats: Mutex<HashMap<String, Instant>>,
    restarts: Mutex<Vec<Instant>>,
}
#[tauri::command]
pub fn ui_heartbeat(window: tauri::WebviewWindow, health: State<Health>) {
    if let Ok(mut beats) = health.beats.lock() {
        beats.insert(window.label().into(), Instant::now());
    }
}
pub fn busy(phase: &str) -> bool {
    ["recording", "paused", "processing"].contains(&phase)
}
#[cfg(test)]
mod tests {
    #[test]
    fn never_reload_during_audio_work() {
        for phase in ["recording", "paused", "processing"] {
            assert!(super::busy(phase));
        }
        assert!(!super::busy("idle"));
    }
}
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) -> Result<(), String> {
    if busy(&app.state::<Engine>().recorder.snapshot().phase)
        || app
            .state::<Engine>()
            .processing
            .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err("Termina o guarda la grabacion antes de reiniciar".into());
    }
    crate::engine::shutdown(&app);
    app.restart();
}
pub fn start(app: &tauri::AppHandle) {
    app.manage(Health {
        beats: Mutex::new(HashMap::new()),
        restarts: Mutex::new(vec![]),
    });
    let app = app.clone();
    std::thread::spawn(move || {
        let mut visible = std::collections::HashSet::new();
        loop {
            std::thread::sleep(Duration::from_secs(3));
            let enabled = app
                .state::<Store>()
                .get::<Settings>("settings")
                .map(|s| s.watchdog)
                .unwrap_or(false);
            let occupied = busy(&app.state::<Engine>().recorder.snapshot().phase)
                || app
                    .state::<Engine>()
                    .processing
                    .load(std::sync::atomic::Ordering::SeqCst);
            let health = app.state::<Health>();
            for (label, window) in app.webview_windows() {
                if !window.is_visible().unwrap_or(false) {
                    visible.remove(&label);
                    continue;
                }
                let first = visible.insert(label.clone());
                let stale = {
                    let mut beats = match health.beats.lock() {
                        Ok(b) => b,
                        Err(_) => continue,
                    };
                    let beat = beats.entry(label.clone()).or_insert_with(Instant::now);
                    if first || occupied || !enabled {
                        *beat = Instant::now();
                    }
                    beat.elapsed() > Duration::from_secs(30)
                };
                if stale {
                    let mut attempts = match health.restarts.lock() {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    attempts.retain(|a| a.elapsed() < Duration::from_secs(600));
                    if attempts.len() >= 2 {
                        continue;
                    }
                    attempts.push(Instant::now());
                    // Reload only the unresponsive renderer, never stop the audio engine.
                    let _ = app
                        .state::<Store>()
                        .event("UI sin respuesta: recargando ventana (audio intacto)");
                    if let Ok(url) = window.url() {
                        let _ = window.navigate(url);
                    }
                    if let Ok(mut b) = health.beats.lock() {
                        b.insert(label, Instant::now());
                    }
                }
            }
        }
    });
}
