use crate::storage::{Rule, Settings};
use reqwest::multipart;
use std::{path::Path, sync::OnceLock};

fn client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT.get_or_init(|| reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .pool_idle_timeout(std::time::Duration::from_secs(120))
        .build().map_err(|_| "No se pudo iniciar la conexion".to_owned()))
        .as_ref().map_err(Clone::clone)
}

pub fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new("Whispera.Desktop.Preview", "groq")
        .map_err(|_| "No se pudo abrir el almacen seguro de credenciales del sistema".into())
}
pub fn corrections(text: &str, rules: &[Rule]) -> String {
    corrections_many(&[text], rules).pop().unwrap_or_default()
}
// Compile each dictionary rule once for the full transcript and its segments.
// Recompiling the same regex for every phrase adds avoidable latency.
pub fn corrections_many(texts: &[&str], rules: &[Rule]) -> Vec<String> {
    let mut outputs: Vec<String> = texts.iter().map(|text|(*text).to_owned()).collect();
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
            static WORD: OnceLock<regex::Regex> = OnceLock::new();
            let word = WORD.get_or_init(|| regex::Regex::new(r"^\w$").unwrap());
            for output in &mut outputs {
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
            *output = next;
            }
        }
    }
    outputs
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
    let text = json
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or("Groq no devolvio texto")?;
    Ok(if apply {
        corrections(text, rules)
    } else {
        text.to_owned()
    })
}

pub async fn request(
    path: &Path,
    settings: &Settings,
    rules: &[Rule],
    word_times: bool,
) -> Result<serde_json::Value, String> {
    request_format(path, settings, rules, word_times.then_some("word")).await
}

pub async fn request_segments(path: &Path, settings: &Settings, rules: &[Rule]) -> Result<serde_json::Value, String> {
    request_format(path, settings, rules, Some("segment")).await
}

async fn request_format(path: &Path, settings: &Settings, rules: &[Rule], granularity: Option<&str>) -> Result<serde_json::Value, String> {
    let prepared = prepare_upload(path).await?;
    request_prepared(prepared, settings, rules, granularity).await
}

#[derive(Clone)]
struct PreparedUpload {
    bytes: Vec<u8>,
    extension: String,
    background: bool,
    silent: bool,
}

// Scoped to one immutable incremental part; never persists audio or credentials.
pub struct Upload {
    path: std::path::PathBuf,
    prepared: Option<PreparedUpload>,
}
impl Upload {
    pub fn new(path: &Path) -> Self {
        Self { path: path.to_path_buf(), prepared: None }
    }

    async fn prepare(&mut self) -> Result<&PreparedUpload, String> {
        if self.prepared.is_none() {
            self.prepared = Some(prepare_upload(&self.path).await?);
        }
        Ok(self.prepared.as_ref().unwrap())
    }

    pub async fn request(&mut self, settings: &Settings, rules: &[Rule], word_times: bool) -> Result<serde_json::Value, String> {
        let prepared = self.prepare().await?.clone();
        request_prepared(prepared, settings, rules, word_times.then_some("word")).await
    }
}

async fn prepare_upload(path: &Path) -> Result<PreparedUpload, String> {
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
    let mut bytes = tokio::fs::read(path)
        .await
        .map_err(|_| "No se pudo leer el audio")?;
    if ext == "wav" && digital_silence(&bytes) {
        return Ok(PreparedUpload { bytes: Vec::new(), extension: ext, background: false, silent: true });
    }
    // Keep every nonzero sample and its original time coordinate. Long digital
    // silence at the end can make Whisper invent a closing phrase. This only
    // shortens the upload copy; it never filters recognized words or the source.
    if ext == "wav" { crate::upload_tail::trim(&mut bytes); }
    // Quiet microphone noise is not digital zero. Only reject the known closing
    // hallucination when the PCM is stationary background; quiet or brief speech
    // keeps its modulation and is never rejected by volume alone. The retained
    // 200 ms zero tail also preserves the non-stationary classification after trim.
    let stationary_background = ext == "wav" && stationary_background(&bytes);
    let (bytes,ext)=crate::lossless_upload::prepare(bytes,ext).await;
    if bytes.is_empty() {return Err("No se pudo preparar el envío. El audio original sigue guardado.".into());}
    Ok(PreparedUpload { bytes, extension: ext, background: stationary_background, silent: false })
}

async fn request_prepared(prepared: PreparedUpload, settings: &Settings, rules: &[Rule], granularity: Option<&str>) -> Result<serde_json::Value, String> {
    send_prepared(prepared, settings, rules, granularity,
        "https://api.groq.com/openai/v1/audio/transcriptions", || entry()?
        .get_password()
        .map_err(|_| "Configura la clave de Groq en Transcripcion".into())).await
}

async fn send_prepared(prepared: PreparedUpload, settings: &Settings, rules: &[Rule], granularity: Option<&str>, url: &str,
    read_key: impl FnOnce() -> Result<String, String>) -> Result<serde_json::Value, String> {
    if prepared.silent { return Ok(serde_json::json!({"text":"","words":[],"segments":[]})); }
    let key = read_key()?;
    let file = multipart::Part::bytes(prepared.bytes).file_name(format!("audio.{}", prepared.extension));
    let mut form = multipart::Form::new()
        .part("file", file)
        .text("model", settings.model.clone())
        .text(
            "response_format",
            if granularity.is_some() { "verbose_json" } else { "json" },
        )
        .text("temperature", "0");
    if let Some(value) = granularity {
        form = form.text("timestamp_granularities[]", value.to_owned());
    }
    if !rules.is_empty() {
        form = form.text("prompt", prompt(rules));
    }
    if settings.language != "auto" {
        form = form.text("language", settings.language.clone());
    }
    let response = client()?
        .post(url)
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
    let mut json: serde_json::Value = response
        .json()
        .await
        .map_err(|_| "Groq devolvio una respuesta no valida")?;
    suppress_background_closing(&mut json, prepared.background);
    Ok(json)
}

fn suppress_background_closing(json: &mut serde_json::Value, background: bool) {
    let normalized: String=json["text"].as_str().unwrap_or("").chars().filter(|c|c.is_alphanumeric()).flat_map(char::to_lowercase).collect();
    if background && normalized == "gracias" {
        json["text"]=serde_json::json!("");
        for key in ["segments","words"] {if json.get(key).is_some(){json[key]=serde_json::json!([]);}}
    }
}

fn stationary_background(bytes: &[u8]) -> bool {
    let Ok(mut wav)=hound::WavReader::new(std::io::Cursor::new(bytes)) else{return false};
    let spec=wav.spec();
    if spec.channels!=1 || spec.bits_per_sample!=16 || spec.sample_format!=hound::SampleFormat::Int || spec.sample_rate<8000 {return false;}
    let size=(spec.sample_rate/50) as usize; // 20ms windows, no resampling or model.
    let (mut count,mut sum,mut squares,mut windows)=(0usize,0f64,0f64,0usize);
    let (mut low,mut high)=(f64::INFINITY,0f64);
    let mut boundary_zero = false;
    for sample in wav.samples::<i16>() {
        let Ok(sample)=sample else{return false};let sample=f64::from(sample);
        sum+=sample;squares+=sample*sample;count+=1;
        if count==size {
            // Ignore exact digital padding only at the boundaries. An internal
            // silent window is modulation, so it must still preserve speech.
            if squares == 0. {
                boundary_zero |= windows > 0;
                (count,sum,squares)=(0,0.,0.);
                continue;
            }
            if boundary_zero { return false; }
            let power=(squares/count as f64-(sum/count as f64).powi(2)).max(0.);
            low=low.min(power);high=high.max(power);windows+=1;
            (count,sum,squares)=(0,0.,0.);
        }
    }
    // Any quiet/short/unsupported/ambiguous signal is kept. This is intentionally
    // narrower than VAD: it does not remove words or trim the user's recording.
    if count>0 && ((boundary_zero && squares>0.) || squares/count as f64-(sum/count as f64).powi(2)>high*1.8){return false;}
    windows>=5 && high>0. && low>0. && high/low<1.8
}

// Exact digital zero only: do not classify quiet speech or noise as silence.
fn digital_silence(bytes:&[u8])->bool {
    let Ok(mut reader)=hound::WavReader::new(std::io::Cursor::new(bytes)) else{return false;};
    if reader.duration()==0{return false;}
    match reader.spec().sample_format {
        hound::SampleFormat::Int=>reader.samples::<i32>().all(|s|matches!(s,Ok(0))),
        hound::SampleFormat::Float=>reader.samples::<f32>().all(|s|s.is_ok_and(|v|v==0.)),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn performance_incremental_upload_is_prepared_once_and_retains_original_bytes() {
        let path = std::env::temp_dir().join(format!("whispera-upload-reuse-{}.wav", uuid::Uuid::new_v4()));
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, hound::WavSpec {
                channels: 1, sample_rate: 16000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int,
            }).unwrap();
            for i in 0..16000 { writer.write_sample(if (i / 1600) % 2 == 0 { 0i16 } else { 1000 }).unwrap(); }
            writer.finalize().unwrap();
        }
        let original = cursor.into_inner();
        std::fs::write(&path, &original).unwrap();
        let retained = tauri::async_runtime::block_on(async {
            let mut upload = Upload::new(&path);
            let prepared = upload.prepare().await.unwrap();
            assert_eq!(prepared.bytes, original);
            assert!(!prepared.silent && !prepared.background);
            std::fs::remove_file(&path).unwrap();
            let mut outcomes = Vec::new();
            for _ in 0..3 { outcomes.push(upload.prepare().await.map(|next| next.bytes == original)); }
            outcomes
        });
        assert_eq!(retained, vec![Ok(true); 3], "Retry preparation must not reopen or change immutable audio");
    }

    #[test]
    fn prepared_attempts_preserve_payload_options_sanitization_and_fresh_credentials() {
        use std::io::{BufRead, BufReader, Read, Write};
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/fixture", server.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for attempt in 0..3 {
                let (mut stream, _) = server.accept().unwrap();
                stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
                let mut input = BufReader::new(stream.try_clone().unwrap());
                let mut headers = String::new();
                let mut length = None;
                loop {
                    let mut line = String::new();
                    assert!(input.read_line(&mut line).unwrap() > 0);
                    if line == "\r\n" { break; }
                    if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                        length = Some(value.trim().parse::<usize>().unwrap());
                    }
                    headers.push_str(&line);
                }
                let mut body = vec![0; length.unwrap()];
                input.read_exact(&mut body).unwrap();
                requests.push((headers, body));
                let status = if attempt < 2 { "500 Internal Server Error" } else { "200 OK" };
                let body = r#"{"text":"Gracias.","words":[{"word":"Gracias.","start":0,"end":1}]}"#;
                write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
            requests
        });
        let settings = Settings { language: "es".into(), ..Default::default() };
        let rules = vec![rule("whisper", "Whispera")];
        let payload = b"synthetic immutable payload".to_vec();
        let mut reads = 0;
        tauri::async_runtime::block_on(async {
            for attempt in 0..3 {
                let prepared = PreparedUpload { bytes: payload.clone(), extension: "wav".into(), background: true, silent: false };
                let result = send_prepared(prepared, &settings, &rules, Some("word"), &url, || {
                    reads += 1;
                    Ok(format!("synthetic-key-{reads}"))
                }).await;
                if attempt < 2 { assert!(result.unwrap_err().contains("HTTP 500")); }
                else { let result = result.unwrap(); assert_eq!(result["text"], ""); assert_eq!(result["words"], serde_json::json!([])); }
            }
        });
        assert_eq!(reads, 3);
        for (index, (headers, body)) in worker.join().unwrap().iter().enumerate() {
            assert!(headers.to_lowercase().contains(&format!("authorization: bearer synthetic-key-{}", index + 1)));
            assert!(body.windows(payload.len()).any(|window| window == payload));
            let body = String::from_utf8_lossy(body);
            for value in ["audio.wav", &settings.model, "verbose_json", "timestamp_granularities[]", "word", "temperature", "language", "es", "Whispera"] {
                assert!(body.contains(value), "Missing request field: {value}");
            }
        }
    }
    #[test]
    fn stationary_noise_with_digital_boundary_padding_is_still_background() {
        let mut bytes=std::io::Cursor::new(Vec::new());
        let spec=hound::WavSpec{channels:1,sample_rate:16000,bits_per_sample:16,sample_format:hound::SampleFormat::Int};
        let mut writer=hound::WavWriter::new(&mut bytes,spec).unwrap();
        for _ in 0..3200 {writer.write_sample(0i16).unwrap();}
        let mut seed=13u32;
        for _ in 0..32000 {seed=seed.wrapping_mul(1664525).wrapping_add(1013904223);writer.write_sample(((seed>>16)%161) as i16-80).unwrap();}
        for _ in 0..3200 {writer.write_sample(0i16).unwrap();}
        writer.finalize().unwrap();
        assert!(stationary_background(bytes.get_ref()));
    }
    #[test]
    #[ignore = "Explicit synthetic-only probe for repeated speech and genuine thanks; no microphone or profile writes"]
    fn synthetic_thanks_probe() {
        let root=std::path::PathBuf::from(std::env::var_os("WHISPERA_THANKS_FIXTURES").expect("Synthetic fixtures"));
        let manifest:serde_json::Value=serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["syntheticOnly"],true);
        crate::upload_tail::BENCHMARK_BYPASS.store(std::env::var("WHISPERA_THANKS_BASELINE").as_deref()==Ok("1"),std::sync::atomic::Ordering::SeqCst);
        crate::lossless_upload::configure(Path::new(env!("CARGO_MANIFEST_DIR")).join(if cfg!(windows){"../node_modules/ffmpeg-static/ffmpeg.exe"}else{"../node_modules/ffmpeg-static/ffmpeg"}));
        let files=std::env::var("WHISPERA_THANKS_FILES").expect("Explicit fixture names");
        let output=std::path::PathBuf::from(std::env::var_os("WHISPERA_THANKS_RESULTS").expect("Output"));
        let mut rows=vec![];
        tauri::async_runtime::block_on(async {
            for file in files.split(',') {
                assert!(file.ends_with(".wav")&&!file.contains(['/', '\\']));
                let mut settings=Settings::default();
                if let Ok(model)=std::env::var("WHISPERA_THANKS_MODEL") {settings.model=model;}
                let granularity=std::env::var("WHISPERA_THANKS_GRANULARITY").unwrap_or_else(|_|"word".into());
                let began=std::time::Instant::now();
                let response=request_format(&root.join(file),&settings,&[],if granularity=="json"{None}else{Some(&granularity)}).await.unwrap();
                rows.push(serde_json::json!({"file":file,"model":settings.model,"granularity":granularity,"elapsedMs":began.elapsed().as_secs_f64()*1000.,"response":response}));
                std::fs::write(&output,serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
                eprintln!("Synthetic {file}: {:.1} ms",rows.last().unwrap()["elapsedMs"].as_f64().unwrap());
            }
        });
        if std::env::var("WHISPERA_THANKS_VALIDATE").as_deref()==Ok("1") {
            for row in &rows {
                let case=manifest["cases"].as_array().unwrap().iter().find(|case|case["file"]==row["file"]).expect("Reference case");
                let expected=case["expectedThanksCount"].as_u64().unwrap() as usize;
                let actual=row["response"]["text"].as_str().unwrap().split_whitespace().filter(|word|word.chars().filter(|c|c.is_alphanumeric()).collect::<String>().to_lowercase()=="gracias").count();
                assert_eq!(actual,expected,"Wrong thanks count: {}",row["file"]);
            }
        }
    }
    #[test]
    fn stationary_background_filter_preserves_voice_and_real_thanks() {
        let mut noise=serde_json::json!({"text":" Gracias.","segments":[{"text":" Gracias."}],"duration":3});
        suppress_background_closing(&mut noise,true);assert_eq!(noise["text"],"");assert_eq!(noise["segments"],serde_json::json!([]));assert_eq!(noise["duration"],3);
        for (text,background) in [("Gracias.",false),("Gracias por ayudarme.",true),("Esta es mi voz",true)] {
            let mut result=serde_json::json!({"text":text});suppress_background_closing(&mut result,background);assert_eq!(result["text"],text);
        }
        for rate in [8000,16000,44100,48000,96000] {
            let mut bytes=std::io::Cursor::new(Vec::new());let mut seed=71u32;
            {let mut writer=hound::WavWriter::new(&mut bytes,hound::WavSpec{channels:1,sample_rate:rate,bits_per_sample:16,sample_format:hound::SampleFormat::Int}).unwrap();
            for _ in 0..rate*2{seed=seed.wrapping_mul(1664525).wrapping_add(1013904223);writer.write_sample(((seed>>16)%161) as i16-80).unwrap();}writer.finalize().unwrap();}
            assert!(stationary_background(bytes.get_ref()),"stationary microphone noise at {rate}");
        }
        assert!(!stationary_background(b"invalid"));
    }
    #[test]
    fn quiet_modulated_signal_is_not_stationary_noise() {
        for level in [2.,8.,32.,1000.] {
            let mut bytes=std::io::Cursor::new(Vec::new());
            {let mut writer=hound::WavWriter::new(&mut bytes,hound::WavSpec{channels:1,sample_rate:16000,bits_per_sample:16,sample_format:hound::SampleFormat::Int}).unwrap();
            for i in 0..32000{let envelope=if (i/1600)%3==0{0.}else{1.};let sample=(level*envelope*(i as f64*0.1).sin()).round() as i16;writer.write_sample(sample).unwrap();}writer.finalize().unwrap();}
            assert!(!stationary_background(bytes.get_ref()),"quiet modulation at amplitude {level}");
        }
    }
    #[test]
    #[ignore = "Local CPU benchmark over synthetic 20-second PCM; no network or devices"]
    fn background_guard_latency() {
        let mut bytes=std::io::Cursor::new(Vec::new());let mut seed=13u32;
        {let mut writer=hound::WavWriter::new(&mut bytes,hound::WavSpec{channels:1,sample_rate:48000,bits_per_sample:16,sample_format:hound::SampleFormat::Int}).unwrap();for _ in 0..960000{seed=seed.wrapping_mul(1664525).wrapping_add(1013904223);writer.write_sample(((seed>>16)%161) as i16-80).unwrap();}writer.finalize().unwrap();}
        let mut times=vec![];for _ in 0..21{let start=std::time::Instant::now();assert!(stationary_background(std::hint::black_box(bytes.get_ref())));times.push(start.elapsed().as_secs_f64()*1000.);}
        times.sort_by(f64::total_cmp);eprintln!("Background guard: synthetic 20s 48000Hz mono PCM, 21 trials; median={:.3}ms min={:.3}ms max={:.3}ms",times[10],times[0],times[20]);
    }
    #[test]
    #[ignore = "Explicit authorization: sends only locally generated synthetic voice/noise to Groq"]
    fn synthetic_silence_and_quiet_voice() {
        let root=std::path::PathBuf::from(std::env::var("WHISPERA_SILENCE_QA").expect("Synthetic fixture directory required"));
        let phase=std::env::var("WHISPERA_SILENCE_PHASE").unwrap_or_else(|_|"probe".into());
        let mut report=vec![];
        tauri::async_runtime::block_on(async {
            for model in ["whisper-large-v3-turbo","whisper-large-v3"] {
                for file in ["noise-low.wav","noise-room.wav","hum.wav","voice.wav","voice-quiet.wav","voice-thanks.wav"] {
                    let mut settings=Settings::default();settings.model=model.into();settings.language="es".into();
                    let start=std::time::Instant::now();
                    let result=request_segments(&root.join(file),&settings,&[]).await.unwrap();
                    report.push(serde_json::json!({"model":model,"file":file,"ms":start.elapsed().as_secs_f64()*1000.,"response":result}));
                }
            }
        });
        std::fs::write(root.join(format!("{phase}.json")),serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        if phase=="validated" {
            for row in &report {
                let text=row["response"]["text"].as_str().unwrap().to_lowercase();
                if row["file"].as_str().unwrap().starts_with("voice") {assert!(text.contains("gracias")&&(row["file"]=="voice-thanks.wav"||text.contains("palabras")),"Quiet speech was lost: {row}");}
                else {assert!(text.trim().is_empty(),"Non-speech hallucination: {row}");}
            }
        }
    }
    #[test]
    fn exact_silence_never_suppresses_even_one_quiet_sample() {
        for value in [0i16,1,-1,100] {
            let mut bytes=std::io::Cursor::new(Vec::new());
            {let mut writer=hound::WavWriter::new(&mut bytes,hound::WavSpec{channels:1,sample_rate:16000,bits_per_sample:16,sample_format:hound::SampleFormat::Int}).unwrap();
            for _ in 0..16000{writer.write_sample(0i16).unwrap();}writer.write_sample(value).unwrap();writer.finalize().unwrap();}
            assert_eq!(digital_silence(bytes.get_ref()),value==0);
        }
        assert!(!digital_silence(b"bad wav"));
    }
    #[test]
    fn transcription_transport_reuses_connection_without_reusing_credentials() {
        use std::{io::{BufRead, BufReader, Write}, net::TcpListener,
            sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}}, time::{Duration, Instant}};
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();
        let url=format!("http://{}/fixture",listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let completed=Arc::new(AtomicUsize::new(0));
        let headers=Arc::new(Mutex::new(Vec::new()));
        let done=completed.clone();let observed=headers.clone();
        let server=std::thread::spawn(move|| {
            let deadline=Instant::now()+Duration::from_secs(5);
            let mut connections=0;let mut workers=vec![];
            while done.load(Ordering::SeqCst)<2 && Instant::now()<deadline {
                match listener.accept() {
                    Ok((mut socket,_))=> {
                        connections+=1;let done=done.clone();let observed=observed.clone();
                        workers.push(std::thread::spawn(move|| {
                            // Accepted sockets inherit nonblocking mode on Windows.
                            socket.set_nonblocking(false).unwrap();
                            socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                            let mut reader=BufReader::new(socket.try_clone().unwrap());
                            while done.load(Ordering::SeqCst)<2 {
                                let mut request=String::new();
                                loop {
                                    let mut line=String::new();
                                    if reader.read_line(&mut line).unwrap_or(0)==0 {return;}
                                    if line=="\r\n" {break;}
                                    request.push_str(&line);
                                }
                                observed.lock().unwrap().push(request.to_lowercase());
                                socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\nok").unwrap();
                                done.fetch_add(1,Ordering::SeqCst);
                            }
                        }));
                    }
                    Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>std::thread::sleep(Duration::from_millis(5)),
                    Err(e)=>panic!("Local fixture: {e}"),
                }
            }
            for worker in workers {worker.join().unwrap();}connections
        });
        tauri::async_runtime::block_on(async {
            for token in ["first","second"] {
                let body=client().unwrap().get(&url).bearer_auth(token).timeout(Duration::from_secs(3))
                    .send().await.unwrap().text().await.unwrap();
                assert_eq!(body,"ok");
                // Hyper returns the consumed connection to its pool asynchronously.
                // Real dictations are separated by microphone recording time.
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });
        let connections=server.join().unwrap();
        let requests=headers.lock().unwrap();
        assert_eq!(connections,1,"successive requests should share the local TCP connection: {requests:?}");
        assert!(requests[0].contains("authorization: bearer first"));
        assert!(requests[1].contains("authorization: bearer second"));
    }
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
