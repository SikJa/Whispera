use crate::storage::Store;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, process::Command};
use tauri::{Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[derive(Clone, Serialize, Deserialize)]
pub struct Recent {
    id: String,
    kind: String,
    path: String,
    created_at: String,
}
pub fn remember(store: &Store, kind: &str, path: &Path) -> Result<(), String> {
    let mut entries: Vec<Recent> = store.get("capture_history")?;
    let path = path.to_string_lossy().into_owned();
    entries.retain(|entry| entry.path != path);
    entries.insert(
        0,
        Recent {
            id: uuid::Uuid::new_v4().to_string(),
            kind: kind.into(),
            path,
            created_at: chrono::Utc::now().to_rfc3339(),
        },
    );
    entries.truncate(12);
    store.put("capture_history", &entries)
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
            crate::screen::copy_file(Path::new(&entry.path))
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
