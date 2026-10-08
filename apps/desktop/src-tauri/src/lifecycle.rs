use std::{fs::{self, OpenOptions}, io::Write, path::{Path, PathBuf}, sync::{Mutex, OnceLock}};

static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
static GATE: Mutex<()> = Mutex::new(());
const LIMIT: u64 = 128 * 1024;

pub fn init(directory: PathBuf) {
    if fs::create_dir_all(&directory).is_err() || DIRECTORY.set(directory.clone()).is_err() { return; }
    if directory.join("running.json").exists() { record("Previous process ended without a clean-exit marker (crash, termination or shutdown)"); }
    let marker = serde_json::json!({"pid":std::process::id(),"started":chrono::Utc::now().to_rfc3339()});
    let _ = fs::write(directory.join("running.json"), marker.to_string());
    record("Started");
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Panic payloads can include private text; record only the source location.
        let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        record(&format!("Rust panic at {location}"));
        previous(info);
    }));
}
fn append(directory: &Path, message: &str) -> std::io::Result<()> {
    let path = directory.join("lifecycle.log");
    if fs::metadata(&path).is_ok_and(|m| m.len() >= LIMIT) {
        let previous = directory.join("lifecycle.previous.log");
        if previous.exists() { fs::remove_file(&previous)?; }
        fs::rename(&path, previous)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{} pid={} {}", chrono::Utc::now().to_rfc3339(), std::process::id(), message)?;
    file.sync_data()
}
pub fn record(message: &str) {
    if let (Some(directory), Ok(_guard)) = (DIRECTORY.get(), GATE.try_lock()) { let _ = append(directory, message); }
}
pub fn clean_exit() {
    record("Clean exit");
    if let Some(directory) = DIRECTORY.get() { let _ = fs::remove_file(directory.join("running.json")); }
}
pub fn keep_on_close(label: &str) -> bool { !label.starts_with("media-") }
pub fn prevent_implicit_exit(code: Option<i32>) -> bool { code.is_none() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transient_editors_are_released_but_tray_windows_survive() {
        for label in ["media-canvas-one", "media-canvas-two"] { assert!(!keep_on_close(label)); }
        for label in ["main", "recorder", "library", "screen-select-0", "screen-hud"] { assert!(keep_on_close(label)); }
        assert!(prevent_implicit_exit(None));
        assert!(!prevent_implicit_exit(Some(0)));
        assert!(!prevent_implicit_exit(Some(1)));
    }
    #[test]
    fn logs_rotate_to_one_bounded_backup() {
        let directory = std::env::temp_dir().join(format!("whispera-lifecycle-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("lifecycle.log"), vec![b'x'; LIMIT as usize]).unwrap();
        append(&directory, "test").unwrap();
        assert_eq!(fs::metadata(directory.join("lifecycle.previous.log")).unwrap().len(), LIMIT);
        assert!(fs::metadata(directory.join("lifecycle.log")).unwrap().len() < 1024);
        fs::remove_dir_all(directory).unwrap();
    }
}
