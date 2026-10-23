//! Opt-in network benchmark: never uses microphone or personal recordings.
use crate::{groq, storage::Settings};
use std::{fs, path::PathBuf, time::Instant};

#[test]
#[cfg(windows)]
#[ignore = "Local screenshot preparation benchmark; synthetic pixels only, no desktop clipboard"]
fn synthetic_capture_copy_preparation() {
    use image::{ImageEncoder,codecs::png::PngEncoder};
    let mut rows=Vec::new();
    for (width,height) in [(1920,1080),(3840,2160)] {
        let mut pixels=image::RgbaImage::new(width,height);
        for (x,y,pixel) in pixels.enumerate_pixels_mut(){*pixel=image::Rgba([((x*7+y*3)%256) as u8,((x+y*13)%256) as u8,((x*19+y)%256) as u8,255]);}
        let original=crate::screen_capture::rgba_png(&pixels).unwrap();
        for trial in 0..4 {
            let at=Instant::now();
            let _validated=tauri::image::Image::from_bytes(&original).unwrap();
            let png=crate::screen_capture::rounded_png(&original,1.).unwrap();
            let decoded=tauri::image::Image::from_bytes(&png).unwrap();
            // Existing Windows provider re-encodes PNG with the default encoder.
            let mut provider_png=Vec::new();PngEncoder::new(&mut provider_png).write_image(decoded.rgba(),width,height,image::ExtendedColorType::Rgba8).unwrap();
            let before_ms=at.elapsed().as_secs_f64()*1000.;
            let at=Instant::now();let rounded=crate::screen_capture::rounded_image(&original,1.).unwrap();let after_png=crate::screen_capture::rgba_png(&rounded).unwrap();let _dib=crate::image_clipboard::bitmap(&rounded).unwrap();let after_ms=at.elapsed().as_secs_f64()*1000.;
            assert_eq!(image::load_from_memory(&provider_png).unwrap().into_rgba8(),rounded);assert_eq!(image::load_from_memory(&after_png).unwrap().into_rgba8(),rounded);
            rows.push(serde_json::json!({"width":width,"height":height,"trial":trial,"warmup":trial==0,"beforeMs":before_ms,"afterMs":after_ms,"originalBytes":original.len(),"pixelsIdentical":true,"clipboardWriteMeasured":false}));
        }
    }
    fs::write(std::env::var_os("WHISPERA_CAPTURE_RESULTS").expect("Results path"),serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}

#[test]
#[ignore = "Sends only the explicitly supplied synthetic 20-second fixture to Groq"]
fn synthetic_segment_latency() {
    crate::lossless_upload::configure(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(if cfg!(windows){"../node_modules/ffmpeg-static/ffmpeg.exe"}else{"../node_modules/ffmpeg-static/ffmpeg"}));
    let root = PathBuf::from(std::env::var_os("WHISPERA_TIMING_FIXTURES").expect("Synthetic fixture directory required"));
    let reference: serde_json::Value = serde_json::from_slice(&fs::read(root.join("reference.json")).unwrap()).unwrap();
    assert_eq!(reference["syntheticOnly"], true);
    let audio = root.join("synthetic-20s.wav");
    let wave = hound::WavReader::open(&audio).unwrap();
    assert_eq!(wave.duration() as f64 / wave.spec().sample_rate as f64, 20.);
    let output = PathBuf::from(std::env::var_os("WHISPERA_TIMING_RESULTS").expect("Result path required"));
    let mut settings = Settings::default(); settings.language = "es".into();
    let mut rows = Vec::new();
    tauri::async_runtime::block_on(async {
        // Cold connection/model cost is recorded separately, not hidden in a median.
        for trial in 0..7 {
            for timed in if trial % 2 == 0 { [false,true] } else { [true,false] } {
                let start = Instant::now();
                let response = if timed { groq::request_segments(&audio, &settings, &[]).await }
                    else { groq::request(&audio, &settings, &[], false).await };
                let ms = start.elapsed().as_secs_f64()*1000.;
                let row = match response {
                    Ok(value) => serde_json::json!({"trial":trial,"warmup":trial==0,"timed":timed,"ms":ms,"text":value["text"],"segments":value.get("segments"),"responseBytes":serde_json::to_vec(&value).unwrap().len()}),
                    Err(error) => serde_json::json!({"trial":trial,"warmup":trial==0,"timed":timed,"ms":ms,"error":error}),
                };
                println!("Groq synthetic 20s: trial {trial}, timed {timed}, {ms:.1} ms, success {}",row.get("error").is_none());
                rows.push(row);
                fs::write(&output,serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
                if rows.last().unwrap().get("error").is_some() { break; }
            }
            if rows.last().unwrap().get("error").is_some() { break; }
        }
    });
    assert_eq!(rows.len(),14,"Benchmark incomplete; see sanitized results");
    assert!(rows.iter().all(|row|row.get("error").is_none()));
    assert!(rows.iter().filter(|row|row["timed"]==true).all(|row|row["segments"].as_array().is_some_and(|parts|!parts.is_empty())));
}

#[test]
#[ignore = "Local dictionary throughput comparison; no network or personal rules"]
fn synthetic_dictionary_segments_latency() {
    use crate::storage::Rule;
    let mut rows=Vec::new();
    for count in [0,100,500] {
        let rules:Vec<_>=(0..count).map(|i|Rule{id:i.to_string(),source:format!("término{i}"),target:format!("Producto{i}"),enabled:true}).collect();
        let phrases=["Esta prueba contiene término0.","Guardá término9 en la carpeta de proyectos.","La cámara y término99 están listos."];
        let full=phrases.join(" ");let input=vec![full.as_str(),phrases[0],phrases[1],phrases[2]];
        for trial in 0..8 {
            let at=Instant::now();let separate:Vec<_>=input.iter().map(|text|groq::corrections(text,&rules)).collect();let old=at.elapsed().as_secs_f64()*1000.;
            let at=Instant::now();let batch=groq::corrections_many(&input,&rules);let optimized=at.elapsed().as_secs_f64()*1000.;assert_eq!(separate,batch);
            rows.push(serde_json::json!({"rules":count,"trial":trial,"separateMs":old,"batchedMs":optimized}));
        }
    }
    fs::write(std::env::var_os("WHISPERA_DICTIONARY_RESULTS").expect("Result file"),serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}

#[test]
#[ignore = "Synthetic-only real API comparison, includes lossless encoding time"]
fn synthetic_lossless_upload_latency() {
    let root=PathBuf::from(std::env::var_os("WHISPERA_TIMING_FIXTURES").expect("Synthetic fixture directory"));
    let reference:serde_json::Value=serde_json::from_slice(&fs::read(root.join("reference.json")).unwrap()).unwrap();assert_eq!(reference["syntheticOnly"],true);
    let audio=root.join("synthetic-20s.wav");let wave=hound::WavReader::open(&audio).unwrap();assert_eq!(wave.duration() as f64/wave.spec().sample_rate as f64,20.);
    let output=PathBuf::from(std::env::var_os("WHISPERA_LOSSLESS_RESULTS").expect("Result path"));
    let flac=output.with_extension("flac");let binary=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../node_modules/ffmpeg-static/ffmpeg.exe");
    let mut settings=Settings::default();settings.language="es".into();let mut rows=Vec::new();
    tauri::async_runtime::block_on(async {
        for trial in 0..7 { for compressed in if trial%2==0 {[false,true]}else{[true,false]} {
            let at=Instant::now();let input=if compressed {
                let mut command=crate::replay::command(&binary);command.args(["-hide_banner","-loglevel","error","-y","-i"]).arg(&audio).args(["-c:a","flac","-compression_level","0"]).arg(&flac).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
                crate::replay::run_command(&mut command,std::time::Duration::from_secs(10)).unwrap();&flac
            }else{&audio};
            let preparation=at.elapsed().as_secs_f64()*1000.;let response=groq::request(input,&settings,&[],false).await.unwrap();
            rows.push(serde_json::json!({"trial":trial,"warmup":trial==0,"flac":compressed,"ms":at.elapsed().as_secs_f64()*1000.,"preparationMs":preparation,"bytes":fs::metadata(input).unwrap().len(),"text":response["text"]}));
            fs::write(&output,serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
        }}
    });
    assert_eq!(rows.len(),14);
}
