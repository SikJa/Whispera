use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};

pub struct Store(pub Mutex<Connection>);

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub model: String,
    pub language: String,
    pub hotkey: String,
    pub color: String,
    pub pattern: String,
    pub placement: String,
    pub auto_copy: bool,
    pub auto_paste: bool,
    pub sound_theme: String,
    pub sounds: bool,
    pub recorder_scale: f64,
    pub trim_silence: bool,
    pub watchdog: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            model: "whisper-large-v3-turbo".into(),
            language: "es".into(),
            hotkey: "Control+Shift+Space".into(),
            color: "#9024DC".into(),
            pattern: "wave".into(),
            placement: "right".into(),
            auto_copy: true,
            auto_paste: true,
            sound_theme: "cristal".into(),
            sounds: true,
            recorder_scale: 0.85,
            trim_silence: true,
            watchdog: true,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if ![
            "pop", "marimba", "cristal", "gota", "madera", "seda", "pulso", "orbita", "tecla",
            "destello",
        ]
        .contains(&self.sound_theme.as_str())
            || !self.recorder_scale.is_finite()
            || !(0.6..=1.25).contains(&self.recorder_scale)
        {
            return Err("Sonido o tamano no valido".into());
        }
        if !["whisper-large-v3-turbo", "whisper-large-v3"].contains(&self.model.as_str()) {
            return Err("Modelo no admitido".into());
        }
        if !["es", "en", "pt", "auto"].contains(&self.language.as_str()) {
            return Err("Idioma no admitido".into());
        }
        if self.color.len() != 7
            || !self.color.starts_with('#')
            || !self.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Color HEX invalido".into());
        }
        if !["wave", "stairs"].contains(&self.pattern.as_str())
            || !["left", "right", "top", "bottom"].contains(&self.placement.as_str())
        {
            return Err("Apariencia no admitida".into());
        }
        if self.hotkey.trim().is_empty() || self.hotkey.len() > 80 {
            return Err("Atajo invalido".into());
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub source: String,
    pub target: String,
    pub enabled: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub id: String,
    pub timestamp: String,
    pub text: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub settings: Settings,
    pub rules: Vec<Rule>,
    pub history: Vec<Transcript>,
    pub key_configured: bool,
    pub logs: Vec<String>,
    pub native: bool,
}

impl Store {
    pub fn append_once(&self, id: &str, text: &str) -> Result<(), String> {
        self.0
            .lock()
            .map_err(|_| "Base de datos ocupada")?
            .execute(
                "INSERT OR IGNORE INTO history VALUES(?1,?2,?3)",
                params![id, chrono::Utc::now().to_rfc3339(), text],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS history(id TEXT PRIMARY KEY, timestamp TEXT NOT NULL, text TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS imports(path TEXT PRIMARY KEY);
            CREATE TABLE IF NOT EXISTS events(id INTEGER PRIMARY KEY, message TEXT NOT NULL);").map_err(|e| e.to_string())?;
        Ok(Self(Mutex::new(conn)))
    }
    pub fn get<T: serde::de::DeserializeOwned + Default>(&self, key: &str) -> Result<T, String> {
        let conn = self.0.lock().map_err(|_| "Base de datos ocupada")?;
        match conn.query_row("SELECT value FROM kv WHERE key=?1", [key], |r| {
            r.get::<_, String>(0)
        }) {
            Ok(value) => serde_json::from_str(&value).map_err(|e| e.to_string()),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(T::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<(), String> {
        let json = serde_json::to_string(value).map_err(|e| e.to_string())?;
        self.0
            .lock()
            .map_err(|_| "Base de datos ocupada")?
            .execute(
                "INSERT INTO kv VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![key, json],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn history(&self) -> Result<Vec<Transcript>, String> {
        let conn = self.0.lock().map_err(|_| "Base de datos ocupada")?;
        let mut stmt = conn
            .prepare("SELECT id,timestamp,text FROM history ORDER BY timestamp DESC LIMIT 2000")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Transcript {
                    id: r.get(0)?,
                    timestamp: r.get(1)?,
                    text: r.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    pub fn append(&self, text: &str) -> Result<(), String> {
        self.0
            .lock()
            .map_err(|_| "Base de datos ocupada")?
            .execute(
                "INSERT INTO history VALUES(?1,?2,?3)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    chrono::Utc::now().to_rfc3339(),
                    text
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn event(&self, message: &str) -> Result<(), String> {
        self.0
            .lock()
            .map_err(|_| "Base de datos ocupada")?
            .execute(
                "INSERT INTO events(message) VALUES(?1)",
                [format!(
                    "{} {}",
                    chrono::Local::now().format("%H:%M:%S"),
                    message
                )],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn logs(&self) -> Result<Vec<String>, String> {
        let conn = self.0.lock().map_err(|_| "Base de datos ocupada")?;
        let mut stmt = conn
            .prepare("SELECT message FROM events ORDER BY id DESC LIMIT 100")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_valid() {
        assert!(Settings::default().validate().is_ok());
    }
    #[test]
    fn malformed_color_rejected() {
        let mut s = Settings::default();
        s.color = "#nope!!".into();
        assert!(s.validate().is_err());
    }
    #[test]
    fn settings_roundtrip() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let mut cfg = Settings::default();
        cfg.color = "#FF0000".into();
        s.put("settings", &cfg).unwrap();
        assert_eq!(s.get::<Settings>("settings").unwrap().color, "#FF0000");
    }
    #[test]
    fn history_roundtrip() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        s.append("Texto de prueba").unwrap();
        assert_eq!(s.history().unwrap()[0].text, "Texto de prueba");
    }
}
