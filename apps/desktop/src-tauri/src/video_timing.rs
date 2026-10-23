//! Cheap local timing bookkeeping; no extra transcription or video decoding.
use crate::{groq, storage::Rule};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::{Read, Seek, SeekFrom}, path::Path};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Cue { pub start: f64, pub end: f64, pub text: String }
#[derive(Clone, Debug, Default)]
pub struct TimedText { pub text: String, pub cues: Vec<Cue> }

fn words(text: &str) -> String { text.split_whitespace().collect::<Vec<_>>().join(" ") }
pub fn parse(value: &serde_json::Value, rules: &[Rule], duration: Option<f64>) -> Result<TimedText, String> {
    let text = value["text"].as_str().ok_or("Groq no devolvió texto")?;
    let segments = value["segments"].as_array();
    let mut texts=vec![text];
    if let Some(segments)=segments { texts.extend(segments.iter().map(|segment|segment["text"].as_str().unwrap_or_default().trim())); }
    let mut corrected=groq::corrections_many(&texts,rules).into_iter();
    let mut result = TimedText { text: corrected.next().unwrap_or_default(), cues: Vec::new() };
    let Some(segments) = segments else { return Ok(result); };
    for (segment,corrected) in segments.iter().zip(corrected) {
        let (Some(start), Some(end), Some(text)) = (segment["start"].as_f64(), segment["end"].as_f64(), segment["text"].as_str()) else { result.cues.clear(); return Ok(result); };
        if !start.is_finite() || !end.is_finite() || start < 0. || end < start
            || duration.is_some_and(|limit| start > limit || end > limit + 0.25)
            || result.cues.last().is_some_and(|cue| start < cue.start) {
            result.cues.clear(); return Ok(result);
        }
        if text.trim().is_empty() { continue; }
        result.cues.push(Cue { start, end: duration.map_or(end, |limit|end.min(limit)), text: corrected });
    }
    // Dictionary rules can span API segments. Never export an incomplete or
    // differently corrected transcript just to retain more timestamp labels.
    let combined = result.cues.iter().map(|cue|cue.text.as_str()).collect::<Vec<_>>().join(" ");
    if words(&combined) != words(&result.text) { result.cues.clear(); }
    Ok(result)
}

pub fn wav_seconds(path: &Path) -> Option<f64> {
    let wave = hound::WavReader::open(path).ok()?;
    let rate = wave.spec().sample_rate;
    (rate > 0).then(|| wave.duration() as f64 / rate as f64)
}

fn timestamp(seconds: f64) -> String {
    let ms = (seconds.max(0.) * 1000.).round() as u64;
    format!("{:02}:{:02}:{:02}.{:03}",ms/3_600_000,ms/60_000%60,ms/1000%60,ms%1000)
}
pub fn render(text: &str, cues: &[Cue], complete: bool) -> String {
    if !complete || cues.is_empty() { return text.to_owned(); }
    cues.iter().map(|cue|format!("[{}–{}] {}",timestamp(cue.start),timestamp(cue.end),cue.text.trim())).collect::<Vec<_>>().join("\n")
}

// Read only the movie header. Paused/reframed parts must use their encoded MP4
// duration, not wall-clock waiting or rounded 300-second chunk numbers.
pub fn mp4_seconds(path: &Path) -> Option<f64> {
    fn find(file: &mut File, mut at: u64, limit: u64, kind: &[u8;4]) -> Option<(u64,u64)> {
        while at.checked_add(8)? <= limit {
            file.seek(SeekFrom::Start(at)).ok()?;
            let mut header = [0;8]; file.read_exact(&mut header).ok()?;
            let short = u32::from_be_bytes(header[..4].try_into().ok()?) as u64;
            let (size,head) = match short {
                0 => (limit.checked_sub(at)?,8),
                1 => { let mut long=[0;8];file.read_exact(&mut long).ok()?;(u64::from_be_bytes(long),16) },
                _ => (short,8),
            };
            let end=at.checked_add(size)?;
            if size < head || end > limit { return None; }
            if &header[4..] == kind { return Some((at+head,end)); }
            at=end;
        }
        None
    }
    let mut file=File::open(path).ok()?;let size=file.metadata().ok()?.len();
    let (movie,end)=find(&mut file,0,size,b"moov")?;
    let (header,end)=find(&mut file,movie,end,b"mvhd")?;
    file.seek(SeekFrom::Start(header)).ok()?;
    let mut version=[0;4];file.read_exact(&mut version).ok()?;
    let offset=match version[0] {0=>12,1=>20,_=>return None};
    let length=if version[0]==0 {8}else{12};
    if header.checked_add(offset+length)? > end {return None;}
    file.seek(SeekFrom::Start(header+offset)).ok()?;
    let mut data=[0;12];file.read_exact(&mut data[..length as usize]).ok()?;
    let scale=u32::from_be_bytes(data[..4].try_into().ok()?) as f64;
    let units=if version[0]==0 {u32::from_be_bytes(data[4..8].try_into().ok()?) as f64}else{u64::from_be_bytes(data[4..12].try_into().ok()?) as f64};
    let seconds=units/scale;
    (scale>0. && seconds.is_finite() && seconds>0.).then_some(seconds)
}

pub fn record_part_duration(video: &Path, directory: &Path) {
    if let Some(seconds)=mp4_seconds(video) {
        if let Ok(data)=serde_json::to_vec(&serde_json::json!({"videoSeconds":seconds})) {
            let _=std::fs::write(directory.join("speech-timing.json"),data);
        }
    }
}
pub fn part_seconds(directory: &Path) -> Option<f64> {
    let file=File::open(directory.join("speech-timing.json")).ok()?;
    let value:serde_json::Value=serde_json::from_reader(file.take(4096)).ok()?;
    let seconds=value["videoSeconds"].as_f64()?;
    (seconds.is_finite() && seconds>0.).then_some(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn segment_times_preserve_text_and_use_recorded_silence() {
        let result=parse(&serde_json::json!({"text":"Hola. Adiós.","segments":[{"start":2.,"end":3.,"text":" Hola."},{"start":7.,"end":9.,"text":" Adiós."}]}),&[],Some(20.)).unwrap();
        assert_eq!(result.text,"Hola. Adiós.");assert_eq!(result.cues.len(),2);
        assert_eq!(render(&result.text,&result.cues,true),"[00:00:02.000–00:00:03.000] Hola.\n[00:00:07.000–00:00:09.000] Adiós.");
    }
    #[test] fn malformed_or_missing_timing_keeps_all_text_without_repeating_api_calls() {
        for segments in [serde_json::json!(null),serde_json::json!([{"start":-1.,"end":2.,"text":"Hola"}]),serde_json::json!([{"start":1.,"end":40.,"text":"Hola"}]),serde_json::json!([{"start":1.,"end":2.,"text":"Otro texto"}])] {
            let result=parse(&serde_json::json!({"text":"Hola","segments":segments}),&[],Some(20.)).unwrap();assert!(result.cues.is_empty());assert_eq!(render(&result.text,&result.cues,true),"Hola");
        }
    }
    #[test] fn dictionary_replacements_keep_original_interval_or_fall_back_to_full_correct_text() {
        let rules=vec![Rule{id:"1".into(),source:"grok desktop".into(),target:"Groq".into(),enabled:true}];
        let result=parse(&serde_json::json!({"text":"grok desktop","segments":[{"start":0.,"end":1.,"text":"grok"},{"start":1.,"end":2.,"text":" desktop"}]}),&rules,Some(3.)).unwrap();
        assert_eq!(result.text,"Groq");assert!(result.cues.is_empty());
    }
    #[test] fn incomplete_second_segment_never_exports_only_the_first_sentence() {
        let value=serde_json::json!({"text":"Hola. Adiós.","segments":[{"start":0.,"end":1.,"text":"Hola."},{"start":2.,"text":"Adiós."}]});
        let result=parse(&value,&[],Some(3.)).unwrap();assert!(result.cues.is_empty());assert_eq!(result.text,"Hola. Adiós.");
    }
    #[test] fn old_cached_transcripts_remain_plain_and_long_times_do_not_wrap_hours() {
        assert_eq!(render("Texto anterior",&[],false),"Texto anterior");assert_eq!(timestamp(3661.125),"01:01:01.125");
    }
    #[test] fn reads_movie_duration_without_decoding_and_rejects_truncated_boxes() {
        let root=std::env::temp_dir().join(format!("timed-mp4-{}",uuid::Uuid::new_v4()));std::fs::create_dir(&root).unwrap();let path=root.join("test.mp4");
        fn boxed(kind:&[u8;4],data:Vec<u8>)->Vec<u8>{let mut value=((data.len()+8) as u32).to_be_bytes().to_vec();value.extend_from_slice(kind);value.extend(data);value}
        for version in [0u8,1] {
            let mut data=vec![0;if version==0 {20}else{32}];data[0]=version;let at=if version==0{12}else{20};data[at..at+4].copy_from_slice(&1000u32.to_be_bytes());
            if version==0 {data[at+4..at+8].copy_from_slice(&2345u32.to_be_bytes());}else{data[at+4..at+12].copy_from_slice(&2345u64.to_be_bytes());}
            let bytes=boxed(b"moov",boxed(b"mvhd",data));std::fs::write(&path,&bytes).unwrap();assert_eq!(mp4_seconds(&path),Some(2.345));
            record_part_duration(&path,&root);assert_eq!(part_seconds(&root),Some(2.345));
            std::fs::write(&path,&bytes[..bytes.len()-1]).unwrap();assert_eq!(mp4_seconds(&path),None);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
