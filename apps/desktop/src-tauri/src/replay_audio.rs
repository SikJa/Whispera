//! Bounded PCM history for the optional replay audio. Nothing runs when audio is off.
use cpal::traits::{DeviceTrait,HostTrait,StreamTrait};
use std::{collections::VecDeque,sync::{Arc,Mutex},time::{Duration,Instant}};

struct Block { end:Instant, samples:Vec<i16> }
struct History { blocks:VecDeque<Block>, error:String }
pub struct Audio {
    _stream:cpal::Stream,
    history:Arc<Mutex<History>>,
    pub rate:u32,
    pub channels:u16,
}
pub struct Snapshot {pub rate:u32,pub channels:u16,pub bytes:Vec<u8>}
fn push<T:Copy>(data:&[T],convert:impl Fn(T)->f32,history:&Mutex<History>,keep:Duration){
    let now=Instant::now();
    let samples=data.iter().map(|sample|(convert(*sample).clamp(-1.,1.)*32767.).round() as i16).collect();
    if let Ok(mut history)=history.lock(){
        history.blocks.push_back(Block{end:now,samples});
        while history.blocks.front().is_some_and(|block|now.saturating_duration_since(block.end)>keep){history.blocks.pop_front();}
    }
}
impl Audio {
    pub fn new(system:bool,seconds:u32)->Result<Self,String>{
        let host=cpal::default_host();
        let device=if system{host.default_output_device()}else{host.default_input_device()}.ok_or("No hay dispositivo de audio disponible")?;
        let config=if system{device.default_output_config()}else{device.default_input_config()}.map_err(|e|e.to_string())?;
        let cfg:cpal::StreamConfig=config.clone().into();
        if cfg.channels==0||cfg.channels>8||cfg.sample_rate.0>192000{return Err("Formato de audio no admitido para la repetición".into());}
        let history=Arc::new(Mutex::new(History{blocks:VecDeque::new(),error:String::new()}));
        let samples=history.clone();let errors=history.clone();let keep=Duration::from_secs(seconds as u64+4);
        let on_error=move|error:cpal::StreamError|{if let Ok(mut history)=errors.lock(){history.error=error.to_string();}};
        // WASAPI loopback is the same backend used by the normal screen recorder.
        let stream=match config.sample_format(){
            cpal::SampleFormat::F32=>device.build_input_stream(&cfg,move|data:&[f32],_|push(data,|v|v,&samples,keep),on_error,None),
            cpal::SampleFormat::I16=>device.build_input_stream(&cfg,move|data:&[i16],_|push(data,|v|v as f32/32768.,&samples,keep),on_error,None),
            cpal::SampleFormat::U16=>device.build_input_stream(&cfg,move|data:&[u16],_|push(data,|v|(v as f32-32768.)/32768.,&samples,keep),on_error,None),
            _=>return Err("Formato de audio no admitido".into()),
        }.map_err(|e|e.to_string())?;
        stream.play().map_err(|e|e.to_string())?;
        Ok(Self{_stream:stream,history,rate:cfg.sample_rate.0,channels:cfg.channels})
    }
    pub fn error(&self)->String{self.history.lock().map(|history|history.error.clone()).unwrap_or_else(|_|"Audio ocupado".into())}
    pub fn snapshot(&self,start:Instant,end:Instant)->Result<Snapshot,String>{
        let history=self.history.lock().map_err(|_|"Audio ocupado")?;
        if !history.error.is_empty(){return Err(history.error.clone());}
        let frames=(end.saturating_duration_since(start).as_secs_f64()*self.rate as f64).round() as usize;
        let mut result=vec![0i16;frames*self.channels as usize];
        for block in &history.blocks{
            let duration=Duration::from_secs_f64(block.samples.len() as f64/self.channels as f64/self.rate as f64);
            let begin=block.end.checked_sub(duration).unwrap_or(block.end);
            if block.end<=start||begin>=end{continue;}
            let source=if begin<start{(start.duration_since(begin).as_secs_f64()*self.rate as f64).round() as usize*self.channels as usize}else{0};
            let target=if begin>start{(begin.duration_since(start).as_secs_f64()*self.rate as f64).round() as usize*self.channels as usize}else{0};
            let count=block.samples.len().saturating_sub(source).min(result.len().saturating_sub(target));
            if count>0{result[target..target+count].copy_from_slice(&block.samples[source..source+count]);}
        }
        let bytes=result.into_iter().flat_map(i16::to_le_bytes).collect();
        Ok(Snapshot{rate:self.rate,channels:self.channels,bytes})
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn audio_history_is_bounded(){
        let history=Mutex::new(History{blocks:VecDeque::from([Block{end:Instant::now()-Duration::from_secs(20),samples:vec![1;10]}]),error:String::new()});
        push(&[0.5f32;4],|v|v,&history,Duration::from_secs(10));
        let history=history.lock().unwrap();assert_eq!(history.blocks.len(),1);assert_eq!(history.blocks[0].samples,vec![16384;4]);
    }
}
