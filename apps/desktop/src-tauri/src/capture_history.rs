use crate::storage::Store;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, process::Command, sync::Mutex};
use tauri::{Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[derive(Clone, Serialize, Deserialize)]
pub struct Recent {
    id: String,
    kind: String,
    path: String,
    created_at: String,
}
static UPDATE:Mutex<()>=Mutex::new(());
pub fn remember(store: &Store, kind: &str, path: &Path) -> Result<(), String> {
    // Image exports and video finalization can finish on separate worker threads.
    let _update=UPDATE.lock().map_err(|_|"Historial ocupado")?;
    let mut entries: Vec<Recent> = store.get("capture_history")?;
    let path = path.to_string_lossy().into_owned();
    entries.retain(|entry| entry.path != path);
    entries.insert(
        0,
        Recent {
            id: uuid::Uuid::new_v4().to_string(),
            kind: kind.into(),
            path: path.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
        },
    );
    entries.truncate(12);
    store.put("capture_history", &entries)?;
    crate::library::enqueue(Path::new(&path));
    Ok(())
}
#[tauri::command]
pub fn screen_recent(store: State<Store>) -> Result<Vec<Recent>, String> {
    let mut entries: Vec<Recent> = store.get("capture_history")?;
    entries.retain(|entry| Path::new(&entry.path).is_file());
    Ok(entries)
}
fn find(store: &Store, id: &str) -> Result<Recent, String> {
    store
        .get::<Vec<Recent>>("capture_history")?
        .into_iter()
        .find(|entry| entry.id == id && Path::new(&entry.path).is_file())
        .ok_or_else(|| "El archivo ya no esta disponible".into())
}
#[tauri::command]
pub async fn screen_recent_copy(app: tauri::AppHandle, id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let entry = find(&app.state::<Store>(), &id)?;
        if entry.kind == "image" {
            let bytes = fs::read(&entry.path).map_err(|e| e.to_string())?;
            let image = tauri::image::Image::from_bytes(&bytes).map_err(|e| e.to_string())?;
            app.clipboard()
                .write_image(&image)
                .map_err(|e| e.to_string())
        } else {
            crate::video_transcript::copy_files(&app, &[std::path::PathBuf::from(&entry.path)])
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn screen_recent_reveal(store: State<Store>, id: String) -> Result<(), String> {
    let entry = find(&store, &id)?;
    Command::new("explorer.exe")
        .arg(format!("/select,{}", entry.path))
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simultaneous_images_and_videos_do_not_lose_history_entries() {
        let store=std::sync::Arc::new(Store::open(Path::new(":memory:")).unwrap());
        let barrier=std::sync::Arc::new(std::sync::Barrier::new(12));
        let threads:Vec<_>=(0..12).map(|i|{
            let store=store.clone();let barrier=barrier.clone();
            std::thread::spawn(move||{
                barrier.wait();
                remember(&store,if i%2==0 {"image"}else{"video"},Path::new(&format!("concurrent-{i}"))).unwrap();
            })
        }).collect();
        for thread in threads {thread.join().unwrap();}
        let entries:Vec<Recent>=store.get("capture_history").unwrap();
        assert_eq!(entries.len(),12);
        for i in 0..12 {assert!(entries.iter().any(|e|e.path==format!("concurrent-{i}")));}
    }
    #[test]
    fn recent_history_is_bounded_and_deduplicated() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        for i in 0..20 {
            remember(&store, "video", Path::new(&format!("video-{i}.mp4"))).unwrap();
        }
        let entries: Vec<Recent> = store.get("capture_history").unwrap();
        assert_eq!(entries.len(), 12);
        assert_eq!(entries[0].path, "video-19.mp4");
        remember(&store, "video", Path::new("video-18.mp4")).unwrap();
        let entries: Vec<Recent> = store.get("capture_history").unwrap();
        assert_eq!(entries.len(), 12);
        assert_eq!(entries[0].path, "video-18.mp4");
        assert!(find(&store, "missing").is_err());
    }
}
