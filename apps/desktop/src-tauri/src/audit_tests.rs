//! Audit fixtures use temporary stores and generated speech, never the user's history.
use crate::{groq,storage::{Rule,Settings,Store}};
use std::{fs,path::Path,time::Instant};

#[test]
#[ignore="Isolated native audio worker idle CPU and command latency measurement, no microphone capture"]
fn audit_audio_worker_idle_cpu() {
    use windows::Win32::{Foundation::FILETIME,System::Threading::{GetCurrentProcess,GetProcessTimes}};
    fn cpu_ms()->f64 {unsafe{let(mut created,mut exited,mut kernel,mut user)=(FILETIME::default(),FILETIME::default(),FILETIME::default(),FILETIME::default());GetProcessTimes(GetCurrentProcess(),&mut created,&mut exited,&mut kernel,&mut user).unwrap();let ticks=|v:FILETIME|((v.dwHighDateTime as u64)<<32)|v.dwLowDateTime as u64;(ticks(kernel)+ticks(user)) as f64/10000.}}
    let root=std::env::temp_dir().join(format!("Whispera-audit-idle-{}",uuid::Uuid::new_v4()));let recorder=crate::audio::Recorder::new(root.clone()).unwrap();
    std::thread::sleep(std::time::Duration::from_secs(1));let mut rows=vec![];
    for trial in 1..=3 {let start=Instant::now();let before=cpu_ms();std::thread::sleep(std::time::Duration::from_secs(5));let cpu=cpu_ms()-before;
        let command=Instant::now();assert!(recorder.pause().is_err());let latency=command.elapsed().as_secs_f64()*1000.;
        rows.push(serde_json::json!({"trial":trial,"wallMs":start.elapsed().as_secs_f64()*1000.,"cpuMs":cpu,"idleCommandMs":latency}));}
    let phase=std::env::var("WHISPERA_AUDIT_PHASE").unwrap_or_else(|_|"before".into());
    fs::write(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../../outputs/auditoria-2026-10-06/evidencias/audio-idle-{phase}.json")),serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
    drop(recorder);std::thread::sleep(std::time::Duration::from_millis(50));fs::remove_dir_all(root).unwrap();
}

#[test]
fn audit_settings_all_discrete_values_and_numeric_boundaries() {
    for model in ["whisper-large-v3-turbo","whisper-large-v3"] {
        for language in ["es","en","pt","auto"] {
            for sound in ["pop","marimba","cristal","gota","madera","seda","pulso","orbita","tecla","destello"] {
                for placement in ["left","right","top","bottom"] {
                    for pattern in ["wave","stairs"] {
                        for artwork in ["original","metallic"] {
                            let mut s=Settings::default();s.model=model.into();s.language=language.into();s.sound_theme=sound.into();s.placement=placement.into();s.pattern=pattern.into();s.dictation_artwork=artwork.into();
                            assert!(s.validate().is_ok());
                        }
                    }
                }
            }
        }
    }
    for scale in [0.6,0.85,1.25] {let mut s=Settings::default();s.recorder_scale=scale;assert!(s.validate().is_ok());}
    for scale in [-1.,0.59,1.26,f64::NAN,f64::INFINITY] {let mut s=Settings::default();s.recorder_scale=scale;assert!(s.validate().is_err());}
    for color in ["#ffffff","#000000","#9024DC"] {let mut s=Settings::default();s.color=color.into();s.metallic_color=color.into();assert!(s.validate().is_ok());}
    for color in ["","#123","#GGGGGG","1234567","#12345678"] {let mut s=Settings::default();s.color=color.into();assert!(s.validate().is_err());}
}

#[test]
fn audit_synthetic_database_reopen_corruption_and_unicode_paths() {
    let root=std::env::temp_dir().join(format!("Whispera prueba á ñ 日本 {}",uuid::Uuid::new_v4()));fs::create_dir(&root).unwrap();
    let path=root.join("perfil.sqlite");let store=Store::open(&path).unwrap();
    let mut cfg=Settings::default();cfg.hotkey="Alt+KeyZ".into();cfg.recorder_scale=1.25;
    store.put("settings",&cfg).unwrap();store.append_once("synthetic","Texto de prueba á ñ 日本").unwrap();store.append_once("synthetic","No reemplazar").unwrap();
    store.update_transcript("synthetic","Corrección sintética").unwrap();drop(store);
    let store=Store::open(&path).unwrap();assert_eq!(store.get::<Settings>("settings").unwrap().hotkey,cfg.hotkey);
    assert_eq!(store.history().unwrap().len(),1);assert_eq!(store.history().unwrap()[0].text,"Corrección sintética");
    store.0.lock().unwrap().execute("UPDATE kv SET value='corrupt' WHERE key='settings'",[]).unwrap();assert!(store.get::<Settings>("settings").is_err());
    drop(store);fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore="Explicit synthetic-only performance measurement"]
fn audit_dictionary_performance() {
    let root=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../outputs/auditoria-2026-10-06/evidencias");
    let phase=std::env::var("WHISPERA_AUDIT_PHASE").unwrap_or_else(|_|"before".into());let mut rows=vec![];
    for count in [0,10,100,500] {
        let rules=(0..count).map(|i|Rule{id:i.to_string(),source:format!("término{i}"),target:format!("Producto{i}"),enabled:true}).collect::<Vec<_>>();
        let text=("Texto de prueba, término0. término9 y término99. ").repeat(50);
        for trial in 0..5 {let start=Instant::now();let result=groq::corrections(std::hint::black_box(&text),&rules);let ms=start.elapsed().as_secs_f64()*1000.;assert!(!result.is_empty());rows.push(serde_json::json!({"rules":count,"trial":trial,"ms":ms,"inputBytes":text.len(),"outputBytes":result.len()}));}
    }
    fs::write(root.join(format!("dictionary-{phase}.json")),serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}

#[test]
#[ignore="Explicit authorization: only locally generated synthetic speech is sent to Groq"]
fn audit_synthetic_groq_matrix() {
    let root=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.local/audit-2026-10-06/audio");
    let mut rows=vec![];
    tauri::async_runtime::block_on(async {
        for model in ["whisper-large-v3-turbo","whisper-large-v3"] {
            for (file,language) in [("es.wav","es"),("en.wav","en"),("es.wav","auto"),("es-low.wav","es"),("es-noise.wav","es"),("silence.wav","es"),("es.flac","es"),("en.mp3","en")] {
                for trial in 1..=2 {let mut s=Settings::default();s.model=model.into();s.language=language.into();let start=Instant::now();let result=groq::transcribe(&root.join(file),&s,&[]).await;
                    rows.push(serde_json::json!({"model":model,"file":file,"language":language,"trial":trial,"ms":start.elapsed().as_secs_f64()*1000.,"text":result.as_ref().ok(),"error":result.as_ref().err()}));
                }
            }
        }
    });
    fs::write(root.join("../../../outputs/auditoria-2026-10-06/evidencias/synthetic-groq.json"),serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
    assert!(rows.iter().all(|r|r["text"].is_string() && r["error"].is_null()),"At least one request failed; see recorded errors");
}

#[test]
#[ignore="Synthetic speech only; exercises the real incremental queue and Groq for over one minute"]
fn audit_incremental_synthetic_realtime_and_cached_finish() {
    use std::{io::Write,sync::{Arc,atomic::{AtomicBool,Ordering}},time::Duration};
    let workspace=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut input=hound::WavReader::open(workspace.join(".local/audit-2026-10-06/audio/es.wav")).unwrap();
    let spec=input.spec();assert_eq!(spec.channels,1);assert_eq!(spec.bits_per_sample,16);
    let base:Vec<i16>=input.samples().map(Result::unwrap).collect();
    let repetitions=(65*spec.sample_rate as usize).div_ceil(base.len());
    let samples:Vec<i16>=base.repeat(repetitions);let pcm:Vec<u8>=samples.iter().flat_map(|v|v.to_le_bytes()).collect();
    let root=workspace.join(".local/audit-2026-10-06").join(format!("incremental-{}",uuid::Uuid::new_v4()));fs::create_dir(&root).unwrap();
    fs::write(root.join("session.json"),serde_json::to_vec(&crate::audio::Manifest{id:"audit-synthetic".into(),sample_rate:spec.sample_rate,created_at:chrono::Utc::now().to_rfc3339()}).unwrap()).unwrap();
    fs::write(root.join("audio.pcm"),[]).unwrap();crate::incremental::prepare(&root,Settings::default(),vec![]).unwrap();
    tauri::async_runtime::block_on(async {
        let stop=Arc::new(AtomicBool::new(false));let worker=tauri::async_runtime::spawn(crate::incremental::run(root.clone(),stop.clone()));
        let dir=root.clone();let rate=spec.sample_rate;let started=Instant::now();
        let producer=std::thread::spawn(move||{let mut file=fs::OpenOptions::new().append(true).open(dir.join("audio.pcm")).unwrap();
            for (i,block) in pcm.chunks(rate as usize/10*2).enumerate(){std::thread::sleep((started+Duration::from_millis((i as u64+1)*100)).saturating_duration_since(Instant::now()));file.write_all(block).unwrap();}file.sync_all().unwrap();Instant::now()});
        let stopped=tauri::async_runtime::spawn_blocking(move||producer.join().unwrap()).await.unwrap();stop.store(true,Ordering::SeqCst);
        worker.await.unwrap().unwrap();let text=crate::incremental::finish(&root).await.unwrap();let tail=stopped.elapsed().as_secs_f64()*1000.;
        let cached=Instant::now();assert_eq!(crate::incremental::finish(&root).await.unwrap(),text);let cached_ms=cached.elapsed().as_secs_f64()*1000.;
        assert!(text.to_lowercase().contains("carpeta"),"Reference phrase missing");
        let report=serde_json::json!({"synthetic":true,"seconds":samples.len() as f64/spec.sample_rate as f64,"repetitions":repetitions,"afterStopMs":tail,"cachedMs":cached_ms,"text":text,"fallback":root.join("full-audio-fallback.json").exists(),"fixtureDirectory":root});
        fs::write(workspace.join("outputs/auditoria-2026-10-06/evidencias/incremental-synthetic.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    });
}
