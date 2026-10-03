//! Import only capture preferences; dictation data and credentials stay in place.
use crate::storage::{Settings, Store};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::Path;

pub fn import_screen_settings(store: &Store) -> Result<(), String> {
    let Some(appdata) = std::env::var_os("APPDATA") else { return Ok(()); };
    import_from(store, &Path::new(&appdata).join("app.whispera.desktop/whispera.sqlite"))?;
    if !store.get::<bool>("setup_complete")?
        && crate::groq::entry().and_then(|e| e.get_password().map_err(|e| e.to_string())).is_ok()
    {
        store.put("setup_complete", &true)?;
    }
    Ok(())
}

fn import_from(store: &Store, source: &Path) -> Result<(), String> {
    if store.get::<bool>("unified_screen_import_v1")? || !source.is_file() { return Ok(()); }
    let source = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    let mut target = store.0.lock().map_err(|_| "Base de datos ocupada")?;
    let tx = target.transaction().map_err(|e| e.to_string())?;
    let raw: Option<String> = source.query_row("SELECT value FROM kv WHERE key='screen_preferences'", [], |r| r.get(0))
        .optional().map_err(|e| e.to_string())?;
    if let Some(raw) = raw {
        let prefs: crate::screen::Preferences = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let settings: String = tx.query_row("SELECT value FROM kv WHERE key='settings'", [], |r| r.get(0))
            .optional().map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::to_string(&Settings::default()).unwrap());
        let settings: Settings = serde_json::from_str(&settings).map_err(|e| e.to_string())?;
        let dictation = crate::shortcuts::parse(&settings.hotkey)?;
        let video = crate::shortcuts::parse(&prefs.hotkey)?;
        let image = crate::shortcuts::parse(&prefs.image_hotkey)?;
        if dictation == video || dictation == image || video == image {
            return Err("Los atajos de ambas Whispera se superponen; configura accesos distintos antes de unirlas".into());
        }
        tx.execute("INSERT OR IGNORE INTO kv VALUES('screen_preferences',?1)", [&raw]).map_err(|e| e.to_string())?;
    }
    let history: Option<String> = source.query_row("SELECT value FROM kv WHERE key='capture_history'", [], |r| r.get(0))
        .optional().map_err(|e| e.to_string())?;
    if let Some(history) = history {
        serde_json::from_str::<Vec<serde_json::Value>>(&history).map_err(|e| e.to_string())?;
        tx.execute("INSERT OR IGNORE INTO kv VALUES('capture_history',?1)", [&history]).map_err(|e| e.to_string())?;
    }
    tx.execute("INSERT INTO kv VALUES('unified_screen_import_v1','true')", []).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_dictation_and_existing_capture_preferences() {
        let root = std::env::temp_dir().join(format!("whispera-unification-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.sqlite");
        let old = Store::open(&source).unwrap();
        old.put("screen_preferences", &crate::screen::Preferences::default()).unwrap();
        let target = Store::open(Path::new(":memory:")).unwrap();
        let mut settings = Settings::default(); settings.hotkey = "|".into();
        target.put("settings", &settings).unwrap();
        import_from(&target, &source).unwrap();
        assert_eq!(target.get::<Settings>("settings").unwrap().hotkey, "|");
        let mut prefs: crate::screen::Preferences = target.get("screen_preferences").unwrap();
        prefs.frame_color = "#ff0000".into(); target.put("screen_preferences", &prefs).unwrap();
        import_from(&target, &source).unwrap();
        assert_eq!(target.get::<crate::screen::Preferences>("screen_preferences").unwrap().frame_color, "#ff0000");
        drop(old); std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn conflicting_shortcuts_roll_back_without_a_completion_marker() {
        let root = std::env::temp_dir().join(format!("whispera-unification-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.sqlite");
        let old = Store::open(&source).unwrap();
        let mut prefs = crate::screen::Preferences::default(); prefs.hotkey = "Control+Shift+Space".into();
        old.put("screen_preferences", &prefs).unwrap();
        let target = Store::open(Path::new(":memory:")).unwrap();
        target.put("settings", &Settings::default()).unwrap();
        assert!(import_from(&target, &source).is_err());
        assert!(!target.get::<bool>("unified_screen_import_v1").unwrap());
        assert_eq!(target.get::<Settings>("settings").unwrap().hotkey, "Control+Shift+Space");
        drop(old); std::fs::remove_dir_all(root).unwrap();
    }
}
