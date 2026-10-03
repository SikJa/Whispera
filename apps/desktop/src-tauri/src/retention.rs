//! Native 48-hour retention, independent of Codex or Python.
use crate::{audio::Manifest, engine::Engine, storage::Store};
use chrono::{DateTime, Duration, FixedOffset, NaiveDateTime, TimeZone, Utc};
use std::{collections::HashMap, fs, path::Path, sync::atomic::Ordering};
use tauri::Manager;

fn date(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value).map(|d| d.with_timezone(&Utc)).ok().or_else(|| {
        // Legacy history used Argentina local time without an offset.
        let local = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f").ok()?;
        FixedOffset::west_opt(3 * 3600)?.from_local_datetime(&local).single().map(|d| d.with_timezone(&Utc))
    })
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    { metadata.file_type().is_symlink() }
}

fn newest(path: &Path, latest: &mut DateTime<Utc>) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if is_link(&metadata) { return Err("Enlace inesperado en una grabacion".into()); }
    if let Ok(modified) = metadata.modified() { *latest = (*latest).max(modified.into()); }
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            newest(&entry.map_err(|e| e.to_string())?.path(), latest)?;
        }
    }
    Ok(())
}

pub fn clean(store: &Store, recordings: &Path, now: DateTime<Utc>) -> Result<(usize, usize), String> {
    if is_link(&fs::symlink_metadata(recordings).map_err(|e| e.to_string())?) {
        return Err("Enlace inesperado en recordings".into());
    }
    let cutoff = now - Duration::hours(48);
    // Migration restores historical text without resurrecting expired audio.
    // Keep those original rows until the user explicitly removes them.
    let preserved: Vec<String> = store.get("unified_preserved_history")?;
    let conn = store.0.lock().map_err(|_| "Base de datos ocupada")?;
    let dates: HashMap<String, Option<DateTime<Utc>>> = {
        let mut stmt = conn.prepare("SELECT id,timestamp FROM history").map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(|e| e.to_string())?;
        rows.map(|row| row.map(|(id, timestamp)| (id, date(&timestamp)))).collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    let canonical_root = recordings.canonicalize().map_err(|e| e.to_string())?;
    let mut removed = 0;
    for entry in fs::read_dir(recordings).map_err(|e| e.to_string())? {
        let folder = entry.map_err(|e| e.to_string())?.path();
        let id = folder.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if uuid::Uuid::parse_str(id).map(|u| u.to_string() != id).unwrap_or(true) { continue; }
        let metadata = fs::symlink_metadata(&folder).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || is_link(&metadata) { continue; }
        if !folder.join("completed.json").exists() && !folder.join("cancelled.json").exists() { continue; }
        let manifest: Manifest = match fs::read(folder.join("session.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()) { Some(m) => m, None => continue };
        if manifest.id != id { continue; }
        let Some(mut latest) = date(&manifest.created_at) else { continue; };
        match dates.get(id) {
            Some(Some(d)) => latest = latest.max(*d),
            Some(None) => continue,
            None => {}
        }
        // Completed recently, even if recording started earlier: preserve it.
        if newest(&folder, &mut latest).is_err() || latest >= cutoff { continue; }
        if folder.canonicalize().map_err(|e| e.to_string())?.parent() != Some(canonical_root.as_path()) { return Err("Grabacion fuera de la carpeta esperada".into()); }
        fs::remove_dir_all(&folder).map_err(|e| e.to_string())?;
        removed += 1;
    }
    let mut history_removed = 0;
    for (id, timestamp) in dates {
        if preserved.contains(&id) { continue; }
        // Pending/recent audio still present: leave its history intact.
        if uuid::Uuid::parse_str(&id).is_err() || recordings.join(&id).exists() { continue; }
        if timestamp.map(|d| d < cutoff).unwrap_or(false) {
            history_removed += conn.execute("DELETE FROM history WHERE id=?1", [&id]).map_err(|e| e.to_string())?;
        }
    }
    Ok((removed, history_removed))
}

pub fn start(app: &tauri::AppHandle) {
    let _ = app.state::<Store>().event("Retencion automatica interna: 48 horas; revision cada hora");
    let app = app.clone();
    std::thread::spawn(move || loop {
        {
            let engine = app.state::<Engine>();
            if let Ok(_guard) = engine.gate.try_lock() {
                if !crate::health::busy(&engine.recorder.snapshot().phase) && !engine.processing.load(Ordering::SeqCst) {
                    let store = app.state::<Store>();
                    match clean(&store, &engine.recorder.root, Utc::now()) {
                        Ok((sessions, history)) if sessions + history > 0 => { let _ = store.event(&format!("Retencion 48h: {sessions} sesiones y {history} transcripciones eliminadas")); }
                        Err(error) => { let _ = store.event(&format!("Retencion 48h no completada: {error}")); }
                        _ => {}
                    }
                }
            };
        }
        std::thread::sleep(std::time::Duration::from_secs(3600));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_preserves_restored_history_with_original_dates() {
        let root = std::env::temp_dir().join(format!("whispera-retention-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let store = Store::open(Path::new(":memory:")).unwrap();
        let keep = uuid::Uuid::new_v4().to_string();
        let expire = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        for id in [&keep, &expire] {
            store.0.lock().unwrap().execute("INSERT INTO history VALUES(?1,?2,'test')", rusqlite::params![id,(now - Duration::hours(72)).to_rfc3339()]).unwrap();
        }
        store.put("unified_preserved_history", &vec![keep.clone()]).unwrap();
        assert_eq!(clean(&store,&root,now).unwrap(),(0,1));
        assert_eq!(store.history().unwrap()[0].id,keep);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn removes_only_expired_completed_data_and_preserves_settings() {
        let root = std::env::temp_dir().join(format!("whispera-retention-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let store = Store::open(Path::new(":memory:")).unwrap();
        store.put("retention-test", &"keep").unwrap();
        let now = Utc::now() + Duration::days(3);
        let old = now - Duration::days(4);
        let recent = now - Duration::hours(1);
        let ids: Vec<_> = (0..4).map(|_| uuid::Uuid::new_v4().to_string()).collect();
        for (index, id) in ids.iter().enumerate() {
            let timestamp = if index == 2 { recent } else { old };
            store.0.lock().unwrap().execute("INSERT INTO history VALUES(?1,?2,'test')", rusqlite::params![id, timestamp.to_rfc3339()]).unwrap();
            if index == 3 { continue; }
            let folder = root.join(id);
            fs::create_dir(&folder).unwrap();
            fs::write(folder.join("session.json"), serde_json::json!({"id": id,"sample_rate":48000,"created_at":timestamp.to_rfc3339()}).to_string()).unwrap();
            fs::write(folder.join("audio.pcm"), b"audio").unwrap();
            if index != 1 { fs::write(folder.join("completed.json"), b"{}").unwrap(); }
        }
        assert_eq!(clean(&store, &root, now).unwrap(), (1, 2));
        assert!(!root.join(&ids[0]).exists());
        assert!(root.join(&ids[1]).exists());
        assert!(root.join(&ids[2]).exists());
        assert_eq!(store.history().unwrap().len(), 2);
        assert_eq!(store.get::<String>("retention-test").unwrap(), "keep");
        assert_eq!(clean(&store, &root, now).unwrap(), (0, 0));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn legacy_dates_and_bad_dates() {
        assert_eq!(date("2026-09-29T21:00:00"), date("2026-09-30T00:00:00Z"));
        assert!(date("not-a-date").is_none());
    }
    #[test]
    fn old_recording_just_completed_is_preserved() {
        let root = std::env::temp_dir().join(format!("whispera-retention-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        let folder = root.join(&id);
        fs::create_dir_all(&folder).unwrap();
        let now = Utc::now();
        fs::write(folder.join("session.json"), serde_json::json!({"id":id,"sample_rate":48000,"created_at":(now-Duration::days(4)).to_rfc3339()}).to_string()).unwrap();
        fs::write(folder.join("completed.json"), b"{}").unwrap();
        let store = Store::open(Path::new(":memory:")).unwrap();
        assert_eq!(clean(&store, &root, now).unwrap(), (0,0));
        assert!(folder.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
