//! Durable background transcription with conservative full-audio fallback.
use crate::{
    audio, groq,
    storage::{Rule, Settings},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const SECONDS: u64 = 60;
const CONTEXT: u64 = 1;

#[derive(Serialize, Deserialize)]
pub struct Plan {
    version: u32,
    pub settings: Settings,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Window {
    start: u64,
    end: u64,
    from: u64,
    to: u64,
}

#[derive(Serialize, Deserialize)]
struct Cached {
    window: Window,
    text: String,
    elapsed_ms: u128,
    #[serde(default)]
    alignment: Option<serde_json::Value>,
}

pub struct Live {
    pub stop: Arc<AtomicBool>,
    pub done: Arc<AtomicBool>,
    pub task: tauri::async_runtime::JoinHandle<Result<(), String>>,
}

pub fn enabled(dir: &Path) -> bool {
    dir.join("incremental-plan.json").exists()
}

pub fn prepare(dir: &Path, settings: Settings, rules: Vec<Rule>) -> Result<(), String> {
    atomic_json(
        &dir.join("incremental-plan.json"),
        &Plan {
            version: 2,
            settings,
            rules,
        },
    )
}

fn plan(dir: &Path) -> Result<Plan, String> {
    let p: Plan = serde_json::from_slice(
        &fs::read(dir.join("incremental-plan.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if ![1, 2].contains(&p.version) {
        return Err("Version de segmentos no admitida".into());
    }
    Ok(p)
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut file = File::create(&tmp).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn window(index: u64, samples: u64, rate: u32, finalizing: bool) -> Option<Window> {
    if rate == 0 {
        return None;
    }
    let span = SECONDS * u64::from(rate);
    let pad = CONTEXT * u64::from(rate);
    let start = index.checked_mul(span)?;
    if start >= samples {
        return None;
    }
    let end = start.checked_add(span)?;
    if !finalizing && samples < end + pad {
        return None;
    }
    Some(Window {
        start,
        end: end.min(samples),
        from: start.saturating_sub(pad),
        to: (end + pad).min(samples),
    })
}

fn info(dir: &Path) -> Result<(u64, u32), String> {
    let m: audio::Manifest =
        serde_json::from_slice(&fs::read(dir.join("session.json")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if m.sample_rate == 0 || m.sample_rate > 192_000 {
        return Err("Frecuencia de audio no admitida".into());
    }
    Ok((
        fs::metadata(dir.join("audio.pcm"))
            .map_err(|e| e.to_string())?
            .len()
            / 2,
        m.sample_rate,
    ))
}

fn wav(dir: &Path, index: u64, w: &Window, rate: u32) -> Result<PathBuf, String> {
    let path = dir.join(format!("live-{index:04}.wav"));
    let mut input = File::open(dir.join("audio.pcm")).map_err(|e| e.to_string())?;
    input
        .seek(SeekFrom::Start(w.from * 2))
        .map_err(|e| e.to_string())?;
    let mut bytes = vec![0; ((w.to - w.from) * 2) as usize];
    input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .map_err(|e| e.to_string())?;
    for pair in bytes.chunks_exact(2) {
        writer
            .write_sample(i16::from_le_bytes([pair[0], pair[1]]))
            .map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;
    Ok(path)
}

// Assign each word to one core interval; the context is never pasted twice.
// Silence compression is deliberately disabled here to preserve timestamp coordinates.
fn owned_text(json: &serde_json::Value, w: &Window, rate: u32) -> Result<String, String> {
    let words = json
        .get("words")
        .and_then(|v| v.as_array())
        .ok_or("Groq no devolvio tiempos de palabras; reintenta el audio guardado")?;
    let mut owned = vec![];
    for word in words {
        let start = word["start"].as_f64().ok_or("Tiempo de palabra invalido")?;
        let end = word["end"].as_f64().ok_or("Tiempo de palabra invalido")?;
        if !start.is_finite() || !end.is_finite() || start < 0. || end < start {
            return Err("Tiempo de palabra invalido".into());
        }
        let midpoint = w.from as f64 + (start + end) * 0.5 * f64::from(rate);
        if midpoint >= w.start as f64 && midpoint < w.end as f64 {
            owned.push(word["word"].as_str().ok_or("Palabra invalida")?.trim());
        }
    }
    if words.is_empty() && json["text"].as_str().is_some_and(|s| !s.trim().is_empty()) {
        return Err("Respuesta sin tiempos; texto conservado pendiente de reintento".into());
    }
    Ok(owned.join(" "))
}

async fn part(
    dir: &Path,
    index: u64,
    w: Window,
    rate: u32,
    p: &Plan,
    stop: &AtomicBool,
) -> Result<String, String> {
    let cache = dir.join(format!("live-{index:04}.json"));
    if cache.exists() {
        let c: Cached = serde_json::from_slice(&fs::read(&cache).map_err(|e| e.to_string())?)
            .map_err(|_| "Cache de segmento invalida".to_string())?;
        // At stop, the right context can be longer than at the original snapshot,
        // but the owned core must remain exactly identical.
        if c.window.start != w.start || c.window.end != w.end || c.window.from != w.from {
            return Err("Segmento guardado incompatible".into());
        }
        return Ok(c.text);
    }
    let dir_copy = dir.to_path_buf();
    let copy = w.clone();
    let path = tauri::async_runtime::spawn_blocking(move || wav(&dir_copy, index, &copy, rate))
        .await
        .map_err(|e| e.to_string())??;
    let began = Instant::now();
    let mut last = String::new();
    for attempt in 0..3 {
        if stop.load(Ordering::SeqCst) {
            return Err("Envio anticipado detenido".into());
        }
        match groq::request(&path, &p.settings, &p.rules, true).await {
            Ok(json) => {
                let text = owned_text(&json, &w, rate).map_err(|e| format!("Alineacion: {e}"))?;
                atomic_json(
                    &cache,
                    &Cached {
                        window: w,
                        text: text.clone(),
                        elapsed_ms: began.elapsed().as_millis(),
                        alignment: json.get("words").cloned(),
                    },
                )?;
                return Ok(text);
            }
            Err(e) => {
                // Do not repeatedly submit invalid credentials/files. A 429 is left
                // for explicit retry rather than guessing the provider's reset time.
                let retry = e.contains("conectar") || e.contains("HTTP 5");
                last = e;
                if !retry || attempt == 2 {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(2 << attempt)).await;
            }
        }
    }
    Err(last)
}

pub async fn run(dir: PathBuf, stop: Arc<AtomicBool>) -> Result<(), String> {
    let p = plan(&dir)?;
    let mut index = 0;
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        let (samples, rate) = info(&dir)?;
        if let Some(w) = window(index, samples, rate, false) {
            part(&dir, index, w, rate, &p, &stop).await?;
            index += 1;
        } else {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
}

pub async fn finish(dir: &Path) -> Result<String, String> {
    let p = plan(dir)?;
    if dir.join("full-audio-fallback.json").exists() {
        return original(dir, &p).await;
    }
    let (samples, rate) = info(dir)?;
    if p.version >= 2 && samples < (SECONDS + CONTEXT) * u64::from(rate) {
        return original(dir, &p).await;
    }
    match finish_parts(dir, &p).await {
        Ok(text) => Ok(text),
        Err(e) if fallback_eligible(&e) => {
            atomic_json(
                &dir.join("full-audio-fallback.json"),
                &serde_json::json!({"reason":e}),
            )?;
            original(dir, &p).await
        }
        Err(e) => Err(e),
    }
}

fn fallback_eligible(error: &str) -> bool {
    error.starts_with("Alineacion:")
        || error == "Cache de segmento invalida"
        || error == "Segmento guardado incompatible"
}

// Compare the nearest word on each side of the cut, using both independent
// recognitions. Disagreement includes timestamp drift: never delete text by guess.
fn signature(c: &Cached, cut: u64, rate: u32) -> Result<(Option<String>, Option<String>), String> {
    let words = c
        .alignment
        .as_ref()
        .and_then(|v| v.as_array())
        .ok_or("Alineacion: faltan tiempos guardados")?;
    let mut before = None;
    let mut after = None;
    for word in words {
        let start = word["start"]
            .as_f64()
            .ok_or("Alineacion: tiempo invalido")?;
        let end = word["end"].as_f64().ok_or("Alineacion: tiempo invalido")?;
        let mid = c.window.from as f64 + (start + end) * 0.5 * f64::from(rate);
        if (mid - cut as f64).abs() > 0.8 * f64::from(rate) {
            continue;
        }
        let token: String = word["word"]
            .as_str()
            .ok_or("Alineacion: palabra invalida")?
            .chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect();
        if token.is_empty() {
            continue;
        }
        if mid < cut as f64 {
            before = Some(token);
        } else if after.is_none() {
            after = Some(token);
        }
    }
    Ok((before, after))
}

fn validate_join(left: &Cached, right: &Cached, rate: u32) -> Result<(), String> {
    if signature(left, right.window.start, rate)? != signature(right, right.window.start, rate)? {
        return Err("Alineacion: union ambigua; usando audio completo".into());
    }
    Ok(())
}

async fn original(dir: &Path, p: &Plan) -> Result<String, String> {
    let root = dir.to_path_buf();
    let files = tauri::async_runtime::spawn_blocking(move || audio::materialize(&root, 16_000_000))
        .await
        .map_err(|e| e.to_string())??;
    let mut texts = vec![];
    for (index, file) in files.into_iter().enumerate() {
        let cache = dir.join(format!("full-{index:04}.json"));
        let text: String = if cache.exists() {
            serde_json::from_slice(&fs::read(&cache).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
        } else {
            let trim = p.settings.trim_silence;
            let prepared = tauri::async_runtime::spawn_blocking(move || {
                if trim {
                    audio::trimmed_copy(&file)
                } else {
                    Ok(file)
                }
            })
            .await
            .map_err(|e| e.to_string())??;
            let text = groq::transcribe_raw(&prepared, &p.settings, &p.rules).await?;
            atomic_json(&cache, &text)?;
            text
        };
        texts.push(text);
    }
    Ok(groq::corrections(&texts.join("\n\n"), &p.rules))
}

async fn finish_parts(dir: &Path, p: &Plan) -> Result<String, String> {
    let (samples, rate) = info(dir)?;
    if samples == 0 {
        return Err("Audio vacio".into());
    }
    let mut texts = vec![];
    let mut index = 0;
    let stop = AtomicBool::new(false);
    let mut previous = None;
    while let Some(w) = window(index, samples, rate, true) {
        texts.push(part(dir, index, w, rate, &p, &stop).await?);
        if p.version >= 2 {
            let c: Cached = serde_json::from_slice(
                &fs::read(dir.join(format!("live-{index:04}.json"))).map_err(|e| e.to_string())?,
            )
            .map_err(|_| "Cache de segmento invalida")?;
            if let Some(left) = previous.as_ref() {
                validate_join(left, &c, rate)?;
            }
            previous = Some(c);
        }
        index += 1;
    }
    Ok(groq::corrections(&texts.join(" "), &p.rules))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ambiguous_boundaries_fall_back_without_guessing_repeated_words() {
        let mut left = Cached {
            window: window(0, 121_000, 1000, false).unwrap(),
            text: "".into(),
            elapsed_ms: 0,
            alignment: Some(serde_json::json!([
                {"word":"hola,","start":59.5,"end":59.7}, {"word":"mundo","start":60.1,"end":60.5}
            ])),
        };
        let right = Cached {
            window: window(1, 121_000, 1000, false).unwrap(),
            text: "".into(),
            elapsed_ms: 0,
            alignment: Some(serde_json::json!([
                {"word":"Hola","start":0.5,"end":0.7}, {"word":"mundo.","start":1.1,"end":1.5}
            ])),
        };
        assert!(validate_join(&left, &right, 1000).is_ok());
        left.alignment = Some(
            serde_json::json!([{"word":"hola","start":59.9,"end":60.3},{"word":"mundo","start":60.4,"end":60.5}]),
        );
        assert!(fallback_eligible(
            &validate_join(&left, &right, 1000).unwrap_err()
        ));
        assert!(!fallback_eligible("Groq respondio HTTP 429"));
        assert!(!fallback_eligible("No se pudo conectar con Groq"));
    }
    #[test]
    fn short_recordings_and_fallback_recover_full_audio_cache() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&dir).unwrap();
        prepare(&dir, Settings::default(), vec![]).unwrap();
        atomic_json(
            &dir.join("session.json"),
            &audio::Manifest {
                id: "test".into(),
                sample_rate: 1000,
                created_at: "test".into(),
            },
        )
        .unwrap();
        fs::write(dir.join("audio.pcm"), vec![0; 2000]).unwrap();
        atomic_json(&dir.join("full-0000.json"), &"Texto completo.").unwrap();
        assert_eq!(
            tauri::async_runtime::block_on(finish(&dir)).unwrap(),
            "Texto completo."
        );
        atomic_json(&dir.join("full-audio-fallback.json"), &true).unwrap();
        assert_eq!(
            tauri::async_runtime::block_on(finish(&dir)).unwrap(),
            "Texto completo."
        );
        assert!(dir.join("audio.pcm").exists());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn recovery_uses_cache_in_order_and_applies_dictionary_once() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&dir).unwrap();
        prepare(
            &dir,
            Settings::default(),
            vec![Rule {
                id: "1".into(),
                source: "hola".into(),
                target: "hola mundo".into(),
                enabled: true,
            }],
        )
        .unwrap();
        atomic_json(
            &dir.join("session.json"),
            &audio::Manifest {
                id: "test".into(),
                sample_rate: 1000,
                created_at: "test".into(),
            },
        )
        .unwrap();
        fs::write(dir.join("audio.pcm"), vec![0; 120_100 * 2]).unwrap();
        for (i, text) in ["hola", "segunda", "final"].iter().enumerate().rev() {
            atomic_json(
                &dir.join(format!("live-{i:04}.json")),
                &Cached {
                    window: window(i as u64, 120_100, 1000, true).unwrap(),
                    text: (*text).into(),
                    elapsed_ms: 1,
                    alignment: Some(serde_json::json!([])),
                },
            )
            .unwrap();
        }
        tauri::async_runtime::block_on(async {
            assert_eq!(finish(&dir).await.unwrap(), "hola mundo segunda final");
            assert_eq!(finish(&dir).await.unwrap(), "hola mundo segunda final");
        });
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn stopped_worker_does_not_create_or_submit_parts() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&dir).unwrap();
        prepare(&dir, Settings::default(), vec![]).unwrap();
        // No PCM exists: a stopped worker must return before reading or uploading.
        tauri::async_runtime::block_on(run(dir.clone(), Arc::new(AtomicBool::new(true)))).unwrap();
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn malformed_cache_does_not_silently_drop_audio() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("live-0000.json"), b"{").unwrap();
        let p = Plan {
            version: 1,
            settings: Settings::default(),
            rules: vec![],
        };
        let w = window(0, 100, 1000, true).unwrap();
        assert!(tauri::async_runtime::block_on(part(
            &dir,
            0,
            w,
            1000,
            &p,
            &AtomicBool::new(false)
        ))
        .is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    #[ignore = "Explicit opt-in: sends only the existing public MInDS-14 fixture to Groq; takes 3 minutes"]
    fn benchmark_three_minutes_public() {
        assert_eq!(
            std::env::var("WHISPERA_RUN_PUBLIC_BENCHMARK").as_deref(),
            Ok("1")
        );
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap();
        let fixtures = root.join(".local/benchmarks/groq-public-es-20260915");
        let out = root.join(".local/benchmarks").join(format!(
            "incremental-{}",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        ));
        fs::create_dir_all(&out).unwrap();
        let mut base = vec![];
        let mut rate = 0;
        for index in (0..100).step_by(9) {
            let converted = out.join(format!("source-{index:03}.wav"));
            assert!(std::process::Command::new("ffmpeg")
                .args(["-v", "error", "-i"])
                .arg(fixtures.join(format!("sample-{index:03}.wav")))
                .args(["-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"])
                .arg(&converted)
                .status()
                .unwrap()
                .success());
            let mut reader = hound::WavReader::open(converted).unwrap();
            let spec = reader.spec();
            assert_eq!((spec.channels, spec.bits_per_sample), (1, 16));
            if rate == 0 {
                rate = spec.sample_rate;
            }
            assert_eq!(rate, spec.sample_rate);
            base.extend(reader.samples::<i16>().map(Result::unwrap));
        }
        let samples: Vec<i16> = base
            .iter()
            .copied()
            .cycle()
            .take(rate as usize * 180)
            .collect();
        let pcm: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let baseline = out.join("baseline");
        let live = out.join("live");
        for dir in [&baseline, &live] {
            fs::create_dir(dir).unwrap();
            atomic_json(
                &dir.join("session.json"),
                &audio::Manifest {
                    id: "public-benchmark".into(),
                    sample_rate: rate,
                    created_at: chrono::Utc::now().to_rfc3339(),
                },
            )
            .unwrap();
        }
        fs::write(baseline.join("audio.pcm"), &pcm).unwrap();
        fs::write(live.join("audio.pcm"), []).unwrap();
        let settings = Settings::default();
        prepare(&live, settings.clone(), vec![]).unwrap();
        let mut report = serde_json::json!({
            "source":"https://huggingface.co/datasets/PolyAI/minds14",
            "license":"CC-BY-4.0; PolyAI, Gerz et al. 2021",
            "fixture":"12 public es-ES clips concatenated and repeated to exactly 180 s; NOT continuous natural dictation",
            "method":"Real-time PCM replay; same Rust segmentation/queue/Groq/merge code as app. No microphone, user audio, dictionary or clipboard. Normal path includes existing silence trimming; live preserves timestamps. Latencies include preparation/upload/API/merge, exclude desktop paste. Single paced trial, baseline repeated before/after.",
            "duration_seconds":180,"sample_rate":rate,"model":settings.model,"baseline":[]
        });
        tauri::async_runtime::block_on(async {
            for round in 0..2 {
                let began = Instant::now();
                let mut texts = vec![];
                for path in audio::materialize(&baseline, 16_000_000).unwrap() {
                    let path = audio::trimmed_copy(&path).unwrap();
                    texts.push(groq::transcribe_raw(&path, &settings, &[]).await.unwrap());
                }
                let ms = began.elapsed().as_secs_f64() * 1000.;
                report["baseline"].as_array_mut().unwrap().push(
                    serde_json::json!({"round":round,"wait_ms":ms,"text":texts.join("\n\n")}),
                );
                fs::write(
                    out.join("results.json"),
                    serde_json::to_vec_pretty(&report).unwrap(),
                )
                .unwrap();
                println!("Baseline {round}: {ms:.1} ms");
            }
            let stopped = Arc::new(AtomicBool::new(false));
            let worker = tauri::async_runtime::spawn(run(live.clone(), stopped.clone()));
            let dir = live.clone();
            let began = Instant::now();
            let producer = std::thread::spawn(move || {
                let mut file = File::options()
                    .append(true)
                    .open(dir.join("audio.pcm"))
                    .unwrap();
                let bytes_per_tick = rate as usize * 2 / 10;
                for (index, block) in pcm.chunks(bytes_per_tick).enumerate() {
                    let due = began + Duration::from_millis((index as u64 + 1) * 100);
                    std::thread::sleep(due.saturating_duration_since(Instant::now()));
                    file.write_all(block).unwrap();
                    if index % 10 == 9 {
                        file.sync_data().unwrap();
                    }
                }
                file.sync_all().unwrap();
                Instant::now()
            });
            let stopped_at = tauri::async_runtime::spawn_blocking(move || producer.join().unwrap())
                .await
                .unwrap();
            let ready_before_stop = fs::read_dir(&live)
                .unwrap()
                .flatten()
                .filter(|e| {
                    e.file_name().to_string_lossy().starts_with("live-")
                        && e.path().extension().is_some_and(|x| x == "json")
                })
                .count();
            stopped.store(true, Ordering::SeqCst);
            worker.await.unwrap().unwrap();
            let text = finish(&live).await.unwrap();
            let wait_ms = stopped_at.elapsed().as_secs_f64() * 1000.;
            report["incremental"] = serde_json::json!({"wait_ms":wait_ms,"ready_before_stop":ready_before_stop,"replay_wall_ms":stopped_at.duration_since(began).as_secs_f64()*1000.,"text":text});
            let parts: Vec<serde_json::Value> = (0..3)
                .filter_map(|i| {
                    serde_json::from_slice(&fs::read(live.join(format!("live-{i:04}.json"))).ok()?)
                        .ok()
                })
                .collect();
            report["parts"] = serde_json::json!(parts);
            report["fallback"] = serde_json::json!(live.join("full-audio-fallback.json").exists());
            fs::write(
                out.join("results.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
            println!("Incremental: {wait_ms:.1} ms after stop, {ready_before_stop} parts ready");
            let started = Instant::now();
            let mut texts = vec![];
            for path in audio::materialize(&baseline, 16_000_000).unwrap() {
                texts.push(
                    groq::transcribe_raw(&audio::trimmed_copy(&path).unwrap(), &settings, &[])
                        .await
                        .unwrap(),
                );
            }
            let ms = started.elapsed().as_secs_f64() * 1000.;
            report["baseline"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"round":2,"wait_ms":ms,"text":texts.join("\n\n")}));
            fs::write(
                out.join("results.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
            println!(
                "Baseline after replay: {ms:.1} ms; report: {}",
                out.display()
            );
            // Re-running finish must use cached results and work without a new request.
            let cached = Instant::now();
            assert_eq!(finish(&live).await.unwrap(), text);
            println!(
                "Cached recovery: {:.1} ms",
                cached.elapsed().as_secs_f64() * 1000.
            );
        });
    }
    #[test]
    fn intervals_cover_tail_and_wait_for_context() {
        assert!(window(0, 60_000, 1000, false).is_none());
        assert_eq!(
            window(0, 61_000, 1000, false).unwrap(),
            Window {
                start: 0,
                end: 60_000,
                from: 0,
                to: 61_000
            }
        );
        let second = window(1, 120_200, 1000, true).unwrap();
        assert_eq!(
            (second.start, second.end, second.from, second.to),
            (60_000, 120_000, 59_000, 120_200)
        );
        assert_eq!(window(2, 120_200, 1000, true).unwrap().end, 120_200);
        assert!(window(3, 180_000, 1000, true).is_none());
        assert!(window(0, 0, 1000, true).is_none());
        assert!(window(0, 20, 0, true).is_none());
    }
    #[test]
    fn boundary_words_belong_to_only_one_interval() {
        let a = window(0, 121_000, 1000, false).unwrap();
        let b = window(1, 121_000, 1000, false).unwrap();
        let first = serde_json::json!({"words":[{"word":"antes","start":59.0,"end":59.5},{"word":"cruce","start":59.8,"end":60.2}]});
        let second = serde_json::json!({"words":[{"word":"antes","start":0.0,"end":0.5},{"word":"cruce","start":0.8,"end":1.2},{"word":"despues","start":1.3,"end":1.8}]});
        assert_eq!(owned_text(&first, &a, 1000).unwrap(), "antes");
        assert_eq!(owned_text(&second, &b, 1000).unwrap(), "cruce despues");
        assert!(owned_text(&serde_json::json!({"text":"hola"}), &a, 1000).is_err());
    }
    #[test]
    fn snapshots_preserve_source_and_include_exact_samples() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&dir).unwrap();
        let pcm: Vec<u8> = (0..100i16).flat_map(i16::to_le_bytes).collect();
        fs::write(dir.join("audio.pcm"), &pcm).unwrap();
        let path = wav(
            &dir,
            0,
            &Window {
                start: 10,
                end: 20,
                from: 9,
                to: 21,
            },
            1000,
        )
        .unwrap();
        let mut reader = hound::WavReader::open(path).unwrap();
        assert_eq!(
            reader
                .samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            (9..21i16).collect::<Vec<_>>()
        );
        assert_eq!(fs::read(dir.join("audio.pcm")).unwrap(), pcm);
        fs::remove_dir_all(dir).unwrap();
    }
}
