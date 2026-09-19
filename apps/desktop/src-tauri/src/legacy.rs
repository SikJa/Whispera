use crate::storage::{Rule, Settings, Store};
use rusqlite::params;
use serde_json::Value;
use std::path::Path;

fn read(path: &Path) -> Result<Option<Value>, String> {
    if !path.exists() {
        return Ok(None);
    }
    if path
        .metadata()
        .map_err(|_| "No se pudo leer el archivo anterior")?
        .len()
        > 32_000_000
    {
        return Err("Archivo de importacion demasiado grande".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "No se pudo leer el archivo anterior")?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| "Un archivo anterior contiene JSON invalido; no se importo nada".into())
}
pub fn import(store: &Store, path: &Path) -> Result<String, String> {
    let path = path.canonicalize().map_err(|_| "La carpeta no existe")?;
    let settings = read(&path.join("whispera.settings.json"))?;
    let history = read(&path.join("whispera.history.json"))?;
    let rules = read(&path.join("whispera.corrections.json"))?;
    if settings.is_none() && history.is_none() && rules.is_none() {
        return Err("No se encontraron archivos de Whispera en esa carpeta".into());
    }
    let mut conn = store.0.lock().map_err(|_| "Base de datos ocupada")?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let id = path.to_string_lossy().to_lowercase();
    let exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM imports WHERE path=?1)",
            [&id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if exists {
        return Ok("Esta carpeta ya fue importada".into());
    }
    if let Some(value) = settings {
        let mut s = Settings::default();
        if let Some(v) = value.get("hotkey").and_then(Value::as_str) {
            s.hotkey = v.to_owned();
        }
        if let Some(v) = value.get("groq_model").and_then(Value::as_str) {
            s.model = v.to_owned();
        }
        if let Some(v) = value.get("pill_color").and_then(Value::as_str) {
            s.color = match v {
                "red" => "#F51C28",
                "bordo" | "bordeaux" => "#800020",
                "purple" => "#9024DC",
                "gold" => "#E5B322",
                "white" => "#F3F3F3",
                _ => v,
            }
            .into();
        }
        s.validate()?;
        // Import only known preferences, never secrets or machine-specific paths.
        tx.execute(
            "INSERT OR IGNORE INTO kv VALUES('settings',?1)",
            [serde_json::to_string(&s).map_err(|e| e.to_string())?],
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(value) = history {
        let rows = value
            .as_array()
            .ok_or("El historial anterior no es una lista")?;
        for item in rows {
            let text = item
                .get("text")
                .and_then(Value::as_str)
                .ok_or("Transcripcion anterior invalida")?;
            let timestamp = item
                .get("timestamp")
                .and_then(Value::as_str)
                .ok_or("Fecha de transcripcion invalida")?;
            tx.execute("INSERT INTO history SELECT ?1,?2,?3 WHERE NOT EXISTS(SELECT 1 FROM history WHERE timestamp=?2 AND text=?3)",params![uuid::Uuid::new_v4().to_string(),timestamp,text]).map_err(|e|e.to_string())?;
        }
    }
    if let Some(value) = rules {
        let rows = value
            .as_array()
            .or_else(|| value.get("corrections").and_then(Value::as_array))
            .ok_or("Diccionario anterior invalido")?;
        let stored = tx.query_row("SELECT value FROM kv WHERE key='rules'", [], |r| {
            r.get::<_, String>(0)
        });
        let mut merged: Vec<Rule> = match stored {
            Ok(v) => serde_json::from_str(&v).map_err(|e| e.to_string())?,
            Err(rusqlite::Error::QueryReturnedNoRows) => vec![],
            Err(e) => return Err(e.to_string()),
        };
        for item in rows {
            let source = item
                .get("source")
                .and_then(Value::as_str)
                .ok_or("Palabra anterior invalida")?;
            let target = item
                .get("target")
                .and_then(Value::as_str)
                .ok_or("Correccion anterior invalida")?;
            if source.trim().is_empty() || target.trim().is_empty() {
                return Err("Regla anterior vacia".into());
            }
            if !merged
                .iter()
                .any(|r| r.source == source && r.target == target)
            {
                merged.push(Rule {
                    id: uuid::Uuid::new_v4().to_string(),
                    source: source.into(),
                    target: target.into(),
                    enabled: item.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                });
            }
        }
        tx.execute(
            "INSERT INTO kv VALUES('rules',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [serde_json::to_string(&merged).map_err(|e| e.to_string())?],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.execute("INSERT INTO imports VALUES(?1)", [id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok("Historial y diccionario importados. Preferencias importadas solo si no habia configuracion nueva. La clave API no se importa.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_dictionary_rolls_back_history() {
        let dir =
            std::env::temp_dir().join(format!("whispera-rollback-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let history = dir.join("whispera.history.json");
        let dictionary = dir.join("whispera.corrections.json");
        std::fs::write(&history, r#"[{"timestamp":"2026-01-01","text":"Test"}]"#).unwrap();
        std::fs::write(&dictionary, r#"[{"source":"","target":"Test"}]"#).unwrap();
        let store = Store::open(Path::new(":memory:")).unwrap();
        assert!(import(&store, &dir).is_err());
        assert!(store.history().unwrap().is_empty());
        std::fs::remove_file(history).unwrap();
        std::fs::remove_file(dictionary).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn import_does_not_store_api_key() {
        let dir = std::env::temp_dir().join(format!("whispera-key-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("whispera.settings.json");
        std::fs::write(
            &file,
            r#"{"groq_api_key":"synthetic-secret-for-test","hotkey":"F8"}"#,
        )
        .unwrap();
        let store = Store::open(Path::new(":memory:")).unwrap();
        import(&store, &dir).unwrap();
        let value: Value = store.get("settings").unwrap();
        assert_eq!(value.get("hotkey").unwrap(), "F8");
        assert!(value.get("groq_api_key").is_none());
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn import_preserves_source_and_is_idempotent() {
        let dir =
            std::env::temp_dir().join(format!("whispera-import-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("whispera.history.json");
        let original = r#"[{"timestamp":"2026-01-01T12:00:00","text":"Prueba"}]"#;
        std::fs::write(&file, original).unwrap();
        let store = Store::open(Path::new(":memory:")).unwrap();
        import(&store, &dir).unwrap();
        import(&store, &dir).unwrap();
        assert_eq!(store.history().unwrap().len(), 1);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
