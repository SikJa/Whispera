//! Reduce upload bytes without resampling or changing even one PCM sample.
use std::{fs,io::Cursor,path::{Path,PathBuf},sync::OnceLock,time::{Duration,Instant}};
static ENCODER:OnceLock<PathBuf>=OnceLock::new();
#[cfg(test)]
pub static BENCHMARK_BYPASS:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
pub fn configure(binary:PathBuf) {let _=ENCODER.set(binary);}
pub async fn prepare(bytes:Vec<u8>,extension:String)->(Vec<u8>,String) {
    #[cfg(test)]
    if BENCHMARK_BYPASS.load(std::sync::atomic::Ordering::SeqCst) {return (bytes,extension);}
    if extension!="wav" || bytes.len()<512_000 {return (bytes,extension);}
    let Some(binary)=ENCODER.get() else {return (bytes,extension);};let binary=binary.clone();
    // All work runs off the UI and async executor threads. A failed conversion
    // uses the exact original payload; the durable recording is never modified.
    let compressed=tauri::async_runtime::spawn_blocking(move||{
        let compressed=encode(&binary,&bytes);(bytes,compressed)
    }).await;
    match compressed {
        Ok((_,Some(flac)))=>(flac,"flac".into()),
        Ok((original,None))=>(original,extension),
        Err(error)=>{ // A worker panic is not a successful upload of partial audio.
            // The caller propagates this as an invalid empty payload; never infer text.
            eprintln!("Audio upload preparation failed: {error}");(Vec::new(),extension)
        }
    }
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        for name in ["input.wav","upload.flac"] {let _=fs::remove_file(self.0.join(name));}
        let _=fs::remove_dir(&self.0);
    }
}
fn encode(binary:&Path,bytes:&[u8])->Option<Vec<u8>> {
    let reader=hound::WavReader::new(Cursor::new(bytes)).ok()?;let spec=reader.spec();
    // Do not quantize floating-point or higher-depth sources into 16-bit audio.
    if spec.sample_format!=hound::SampleFormat::Int || spec.bits_per_sample!=16 {return None;}
    let scratch=Scratch(std::env::temp_dir().join(format!("whispera-upload-{}",uuid::Uuid::new_v4())));
    fs::create_dir(&scratch.0).ok()?;let input=scratch.0.join("input.wav");let output=scratch.0.join("upload.flac");fs::write(&input,bytes).ok()?;
    let mut command=crate::replay::command(binary);
    command.args(["-hide_banner","-loglevel","error","-y","-i"]).arg(&input).args(["-map","0:a:0","-c:a","flac","-compression_level","0"]).arg(&output).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let mut process=crate::encoder_job::Encoder::spawn(&mut command).ok()?;let start=Instant::now();
    loop {
        if let Some(status)=process.child.try_wait().ok()? {if !status.success(){return None;}break;}
        if start.elapsed()>Duration::from_secs(2) {return None;}
        std::thread::sleep(Duration::from_millis(5));
    }
    let size=fs::metadata(&output).ok()?.len();if size>=bytes.len() as u64 || size<4 {return None;}
    let compressed=fs::read(output).ok()?;compressed.starts_with(b"fLaC").then_some(compressed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compressed_upload_decodes_to_every_original_sample_and_failure_preserves_source() {
        let binary=Path::new(env!("CARGO_MANIFEST_DIR")).join(if cfg!(windows){"../node_modules/ffmpeg-static/ffmpeg.exe"}else{"../node_modules/ffmpeg-static/ffmpeg"});
        let mut cursor=Cursor::new(Vec::new());let spec=hound::WavSpec{channels:2,sample_rate:48000,bits_per_sample:16,sample_format:hound::SampleFormat::Int};
        let expected:Vec<i16>=(0..48000*2*4).map(|i|((i%200) as i16-100)*123).collect();
        {let mut writer=hound::WavWriter::new(&mut cursor,spec).unwrap();for sample in &expected {writer.write_sample(*sample).unwrap();}writer.finalize().unwrap();}
        let original=cursor.into_inner();let compressed=encode(&binary,&original).unwrap();assert!(compressed.len()<original.len());
        let root=Scratch(std::env::temp_dir().join(format!("whispera-upload-test-{}",uuid::Uuid::new_v4())));fs::create_dir(&root.0).unwrap();let input=root.0.join("upload.flac");let output=root.0.join("input.wav");fs::write(&input,compressed).unwrap();
        let mut command=crate::replay::command(&binary);command.args(["-hide_banner","-loglevel","error","-y","-i"]).arg(&input).args(["-c:a","pcm_s16le"]).arg(&output).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());crate::replay::run_command(&mut command,Duration::from_secs(10)).unwrap();
        let mut decoded=hound::WavReader::open(output).unwrap();assert_eq!(decoded.spec(),spec);assert_eq!(decoded.samples::<i16>().map(Result::unwrap).collect::<Vec<_>>(),expected);
        assert!(encode(Path::new("missing-encoder"),&original).is_none());assert!(encode(&binary,b"not audio").is_none());assert_eq!(original.len(),44+expected.len()*2);
    }
}
