use crate::storage::{Rule, Settings};
use reqwest::multipart;
use std::path::Path;

pub fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new("Whispera.Desktop", "groq")
        .map_err(|_| "No se pudo abrir el almacen de credenciales de Windows".into())
}
pub fn corrections(text: &str, rules: &[Rule]) -> String {
    let mut output = text.to_owned();
    let mut active: Vec<_> = rules
        .iter()
        .filter(|r| r.enabled && !r.source.trim().is_empty())
        .collect();
    active.sort_by_key(|r| std::cmp::Reverse(r.source.chars().count()));
    for rule in active {
        if let Ok(pattern) = regex::RegexBuilder::new(&regex::escape(&rule.source))
            .case_insensitive(true)
            .build()
        {
            let word = regex::Regex::new(r"^\w$").unwrap();
            let mut next = String::new();
            let mut previous = 0;
            for m in pattern.find_iter(&output) {
                let left = output[..m.start()].chars().next_back();
                let right = output[m.end()..].chars().next();
                if left.is_some_and(|c| word.is_match(&c.to_string()))
                    || right.is_some_and(|c| word.is_match(&c.to_string()))
                {
                    continue;
                }
                next.push_str(&output[previous..m.start()]);
                next.push_str(&rule.target);
                previous = m.end();
            }
            next.push_str(&output[previous..]);
            output = next;
        }
    }
    output
}
pub fn validate_rules(rules: &[Rule]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    let mut ids = std::collections::HashSet::new();
    if rules.len() > 5000 {
        return Err("Demasiadas correcciones".into());
    }
    for r in rules {
        let source = r.source.trim().to_lowercase();
        if source.is_empty()
            || r.target.trim().is_empty()
            || r.source.len() > 500
            || r.target.len() > 500
            || r.id.is_empty()
        {
            return Err("Correccion vacia o demasiado larga".into());
        }
        if source == r.target.trim().to_lowercase() {
            return Err("La correccion debe cambiar la palabra".into());
        }
        if !seen.insert(source) || !ids.insert(&r.id) {
            return Err("Ya existe una regla para esa palabra".into());
        }
    }
    Ok(())
}
pub fn prompt(rules: &[Rule]) -> String {
    let mut result =
        "Transcripcion fiel, con puntuacion. Vocabulario y ortografia preferida: ".to_owned();
    let mut active: Vec<_> = rules.iter().filter(|r| r.enabled).collect();
    active.sort_by_key(|r| std::cmp::Reverse(r.source.len()));
    for r in active {
        let hint = format!("{}; ", r.target.trim());
        if result.chars().count() + hint.chars().count() > 600 {
            continue;
        }
        result.push_str(&hint);
    }
    result
}
pub async fn transcribe(
    path: &Path,
    settings: &Settings,
    rules: &[Rule],
) -> Result<String, String> {
    transcribe_inner(path, settings, rules, true).await
}
pub async fn transcribe_raw(
    path: &Path,
    settings: &Settings,
    rules: &[Rule],
) -> Result<String, String> {
    transcribe_inner(path, settings, rules, false).await
}
async fn transcribe_inner(
    path: &Path,
    settings: &Settings,
    rules: &[Rule],
    apply: bool,
) -> Result<String, String> {
    let json = request(path, settings, rules, false).await?;
    let text = json.get("text").and_then(|v| v.as_str()).ok_or("Groq no devolvio texto")?;
    Ok(if apply { corrections(text, rules) } else { text.to_owned() })
}

pub async fn request(
    path: &Path,
    settings: &Settings,
    rules: &[Rule],
    word_times: bool,
) -> Result<serde_json::Value, String> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ![
        "wav", "mp3", "m4a", "ogg", "webm", "flac", "mp4", "mpeg", "mpga",
    ]
    .contains(&ext.as_str())
    {
        return Err("Formato de audio no admitido".into());
    }
    let meta = tokio::fs::metadata(path)
        .await
        .map_err(|_| "No se pudo abrir el audio")?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > 24_000_000 {
        return Err(
            "Selecciona un archivo de audio de hasta 24 MB. El original no se modifica.".into(),
        );
    }
    let key = entry()?
        .get_password()
        .map_err(|_| "Configura la clave de Groq en Transcripcion")?;
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|_| "No se pudo leer el audio")?;
    let file = multipart::Part::bytes(bytes).file_name(format!("audio.{ext}"));
    let mut form = multipart::Form::new()
        .part("file", file)
        .text("model", settings.model.clone())
        .text("response_format", if word_times { "verbose_json" } else { "json" })
        .text("temperature", "0");
    if word_times {
        form = form.text("timestamp_granularities[]", "word");
    }
    if !rules.is_empty() {
        form = form.text("prompt", prompt(rules));
    }
    if settings.language != "auto" {
        form = form.text("language", settings.language.clone());
    }
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|_| "No se pudo iniciar la conexion")?
        .post("https://api.groq.com/openai/v1/audio/transcriptions")
        .bearer_auth(key)
        .multipart(form)
        .send()
        .await
        .map_err(|_| "No se pudo conectar con Groq. El audio original sigue disponible.")?;
    if !response.status().is_success() {
        return Err(format!(
            "Groq respondio HTTP {}. El audio original sigue disponible para reintentar.",
            response.status().as_u16()
        ));
    }
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|_| "Groq devolvio una respuesta no valida")?;
    Ok(json)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rule(source: &str, target: &str) -> Rule {
        Rule {
            id: source.into(),
            source: source.into(),
            target: target.into(),
            enabled: true,
        }
    }
    #[test]
    fn longest_rules_and_punctuation_keep_original_boundaries() {
        let rules = vec![
            rule("foo", "short"),
            rule("foo bar", "long"),
            rule("C++", "C plus plus"),
        ];
        assert_eq!(
            corrections("foo bar, foo! C++ XC++ C++x", &rules),
            "long, short! C plus plus XC++ C++x"
        );
    }
    #[test]
    fn rules_reject_duplicates_and_noops() {
        assert!(validate_rules(&[rule("Groq", "Groq")]).is_err());
        assert!(validate_rules(&[rule("grok", "Groq"), rule(" GROK ", "Otro")]).is_err());
        assert!(validate_rules(&[rule("grok", "Groq")]).is_ok());
    }
    #[test]
    fn prompt_contains_active_dictionary_with_bounded_unicode() {
        let mut disabled = rule("wrong", "Excluded");
        disabled.enabled = false;
        let rules = vec![
            rule("grok", "Groq"),
            disabled,
            rule("huge", &"ñ".repeat(900)),
        ];
        let p = prompt(&rules);
        assert!(p.contains("Groq"));
        assert!(!p.contains("Excluded"));
        assert!(p.chars().count() <= 600);
    }
    #[test]
    fn corrections_respect_words_and_literals() {
        let r = Rule {
            id: "1".into(),
            source: "grok".into(),
            target: "Groq $1".into(),
            enabled: true,
        };
        assert_eq!(corrections("GROK grokking", &[r]), "Groq $1 grokking");
    }
}
