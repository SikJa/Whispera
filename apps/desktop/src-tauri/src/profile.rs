//! One settings database per Windows user, independent of the launching app.
use crate::storage::Store;
use rusqlite::{Connection, DatabaseName, OpenFlags};
use std::path::{Path, PathBuf};

pub fn database_path() -> Result<PathBuf, String> {
    use windows::Win32::{System::Com::CoTaskMemFree, UI::Shell::{SHGetKnownFolderPath, FOLDERID_Profile, KF_FLAG_NO_PACKAGE_REDIRECTION}};
    unsafe {
        let value = SHGetKnownFolderPath(&FOLDERID_Profile, KF_FLAG_NO_PACKAGE_REDIRECTION, None)
            .map_err(|e| format!("No se pudo localizar el perfil de Windows: {e}"))?;
        let decoded = value.to_string().map_err(|e| e.to_string());
        CoTaskMemFree(Some(value.0 as _));
        Ok(PathBuf::from(decoded?).join(".whispera").join("desktop").join("whispera.sqlite"))
    }
}

pub fn open(legacy: &Path) -> Result<Store, String> {
    open_at(&database_path()?, legacy)
}

fn open_at(destination: &Path, legacy: &Path) -> Result<Store, String> {
    let parent = destination.parent().ok_or("Perfil sin directorio")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    // Never re-import an older profile on a subsequent launch.
    if !destination.exists() && legacy.is_file() {
        let source = Connection::open_with_flags(legacy, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("No se pudo leer la configuracion existente: {e}"))?;
        let integrity: String = source.query_row("PRAGMA integrity_check", [], |row| row.get(0)).map_err(|e| e.to_string())?;
        if integrity != "ok" { return Err("La configuracion existente necesita recuperacion; no se reemplazo por valores predeterminados".into()); }
        source.prepare("SELECT key,value FROM kv").map_err(|e| e.to_string())?;
        let temporary = parent.join(format!("migration-{}.sqlite", uuid::Uuid::new_v4()));
        // SQLite backup includes committed WAL pages. A raw file copy does not.
        let result = source.backup(DatabaseName::Main, &temporary, None).map_err(|e| e.to_string())
            .and_then(|_| std::fs::rename(&temporary, destination).map_err(|e| e.to_string()));
        if result.is_err() { let _ = std::fs::remove_file(&temporary); }
        result?;
    }
    Store::open(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_survive_reopen_and_a_different_launching_profile() {
        let root = std::env::temp_dir().join(format!("whispera-profile-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let legacy = root.join("packaged.sqlite");
        let original = Store::open(&legacy).unwrap();
        original.put("settings", &serde_json::json!({"hotkey":"Alt+KeyZ","language":"es","autoPaste":true})).unwrap();
        original.put("screen_preferences", &serde_json::json!({"hotkey":"Alt+KeyX","image_hotkey":"Alt+KeyA","audio":"none"})).unwrap();
        original.put("library_settings", &serde_json::json!({"toggleHotkey":"Alt+KeyC"})).unwrap();
        original.put("setup_complete", &true).unwrap();
        let destination = root.join("shared/whispera.sqlite");
        // Keep the source open so its committed WAL must be included.
        let migrated = open_at(&destination, &legacy).unwrap();
        for key in ["settings", "screen_preferences", "library_settings", "setup_complete"] {
            assert_eq!(migrated.get::<serde_json::Value>(key).unwrap(), original.get::<serde_json::Value>(key).unwrap());
        }
        migrated.put("settings", &serde_json::json!({"hotkey":"Control+Alt+KeyD","language":"en"})).unwrap();
        drop(migrated);
        original.put("settings", &serde_json::json!({"hotkey":"Alt+KeyQ"})).unwrap();
        let reopened = open_at(&destination, &root.join("another-launch.sqlite")).unwrap();
        assert_eq!(reopened.get::<serde_json::Value>("settings").unwrap()["hotkey"], "Control+Alt+KeyD");
        drop(reopened);
        let reopened = open_at(&destination, &legacy).unwrap();
        assert_eq!(reopened.get::<serde_json::Value>("settings").unwrap()["hotkey"], "Control+Alt+KeyD");
        drop(reopened); drop(original);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn broken_source_is_reported_without_creating_default_settings() {
        let root = std::env::temp_dir().join(format!("whispera-profile-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("bad.sqlite");
        std::fs::write(&source, b"damaged database").unwrap();
        let destination = root.join("shared/whispera.sqlite");
        assert!(open_at(&destination, &source).is_err());
        assert!(!destination.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn native_settings_location_is_outside_redirected_appdata() {
        let path = database_path().unwrap();
        assert!(path.is_absolute());
        assert!(path.ends_with(Path::new(".whispera/desktop/whispera.sqlite")));
        assert!(!path.to_string_lossy().contains("LocalCache"));
    }
}
