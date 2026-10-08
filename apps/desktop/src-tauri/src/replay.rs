//! Opt-in, bounded, hardware-encoded replay of the primary monitor.
use crate::{encoder_job::Encoder,replay_audio,storage::Store};
use serde::{Deserialize,Serialize};
use std::{fs,io::{BufRead,BufReader,Read},os::windows::process::CommandExt,path::{Path,PathBuf},process::{Command,Stdio},sync::{Arc,Mutex,mpsc,atomic::{AtomicBool,AtomicU64,Ordering}},time::{Duration,Instant}};
use tauri::{Emitter,Manager};

#[derive(Clone,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",default)]
pub struct Preferences{pub enabled:bool,pub seconds:u32,pub audio:String,pub folder:String,pub hotkey:String}
impl Default for Preferences{fn default()->Self{Self{enabled:false,seconds:60,audio:"none".into(),folder:String::new(),hotkey:String::new()}}}
impl Preferences{
    pub fn validate(&self)->Result<(),String>{
        if !(5..=600).contains(&self.seconds){return Err("Elegí un tiempo entre 5 y 600 segundos".into());}
        if !["none","system","microphone","both"].contains(&self.audio.as_str()){return Err("Audio de repetición inválido".into());}
        if !self.hotkey.is_empty(){if crate::shortcuts::parse(&self.hotkey)?==crate::shortcuts::parse("Escape")? {return Err("Escape se reserva para salir".into());}}
        if self.enabled&&(self.folder.is_empty()||self.hotkey.is_empty()){return Err("Creá una carpeta y elegí un atajo antes de activar la grabación hacia atrás".into());}
        if !self.folder.is_empty(){let path=Path::new(&self.folder);if !path.is_absolute()||!path.is_dir()||!path.join(".whispera-replays").is_file(){return Err("Elegí una carpeta creada para las repeticiones de Whispera".into());}}
        Ok(())
    }
}
#[derive(Clone,Default,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Status{pub phase:String,pub available_seconds:f64,pub encoder:String,pub error:String,pub last_path:String}
struct Worker{tx:mpsc::Sender<Control>,thread:std::thread::JoinHandle<()>}
enum Control{Stop,Save(ExportPermit)}
struct ExportPermit(Arc<AtomicBool>);
impl ExportPermit {
    fn acquire(flag:Arc<AtomicBool>)->Result<Self,String>{
        flag.compare_exchange(false,true,Ordering::SeqCst,Ordering::SeqCst).map_err(|_|"Ya se está preparando una repetición".to_string())?;
        Ok(Self(flag))
    }
}
impl Drop for ExportPermit{fn drop(&mut self){self.0.store(false,Ordering::SeqCst);}}
#[derive(Default)]
pub struct Replay{worker:Mutex<Option<Worker>>,pub gate:Mutex<()>,status:Mutex<Status>,pub exporting:Arc<AtomicBool>}
pub fn preferences(app:&tauri::AppHandle)->Preferences{app.state::<Store>().get("replay_preferences").unwrap_or_default()}
pub fn hotkey(app:&tauri::AppHandle)->String{preferences(app).hotkey}
fn change(app:&tauri::AppHandle,edit:impl FnOnce(&mut Status)){
    if let Ok(mut status)=app.state::<Replay>().status.lock(){edit(&mut status);let _=app.emit_to("main","replay-status",status.clone());}
}
fn settings_window(window:&tauri::WebviewWindow)->Result<(),String>{if window.label()=="main"{Ok(())}else{Err("Abrí Configuración para cambiar la repetición".into())}}
#[tauri::command]
pub fn replay_preferences(app:tauri::AppHandle,window:tauri::WebviewWindow)->Result<Preferences,String>{settings_window(&window)?;Ok(preferences(&app))}
#[tauri::command]
pub fn replay_status(app:tauri::AppHandle,window:tauri::WebviewWindow)->Result<Status,String>{settings_window(&window)?;app.state::<Replay>().status.lock().map(|s|s.clone()).map_err(|_|"Repetición ocupada".into())}
#[tauri::command]
pub fn replay_create_folder(parent:String,app:tauri::AppHandle,window:tauri::WebviewWindow)->Result<String,String>{
    settings_window(&window)?;
    let parent=fs::canonicalize(parent).map_err(|e|e.to_string())?;if !parent.is_dir(){return Err("Seleccioná una carpeta donde crear las repeticiones".into());}
    let folder=parent.join("Whispera-Repeticiones");
    if folder.exists()&&!folder.join(".whispera-replays").is_file(){return Err("Ya existe una carpeta Whispera-Repeticiones ajena a esta función. Elegí otra ubicación".into());}
    fs::create_dir_all(&folder).map_err(|e|e.to_string())?;
    let folder=fs::canonicalize(folder).map_err(|e|e.to_string())?;
    if folder.parent()!=Some(parent.as_path()){return Err("La carpeta de repeticiones no puede ser un enlace a otra ubicación".into());}
    fs::write(folder.join(".whispera-replays"),b"Whispera replay recordings\n").map_err(|e|e.to_string())?;
    let _=app;Ok(folder.to_string_lossy().into_owned())
}
#[tauri::command]
pub async fn replay_save_preferences(value:Preferences,app:tauri::AppHandle,window:tauri::WebviewWindow)->Result<(),String>{
    settings_window(&window)?;value.validate()?;
    tauri::async_runtime::spawn_blocking(move||configure(&app,value)).await.map_err(|e|e.to_string())?
}
fn configure(app:&tauri::AppHandle,value:Preferences)->Result<(),String>{
    let _work=crate::updates::work(app)?;
    let restore=Instant::now();while app.state::<crate::shortcuts::Capture>().active.load(Ordering::SeqCst)&&restore.elapsed()<Duration::from_secs(2){std::thread::sleep(Duration::from_millis(10));}
    if app.state::<crate::shortcuts::Capture>().active.load(Ordering::SeqCst){return Err("Terminá de elegir el atajo antes de guardar".into());}
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let state=app.state::<Replay>();let _gate=state.gate.lock().map_err(|_|"Repetición ocupada")?;
    let old=preferences(app);
    if !value.hotkey.is_empty()&&old.hotkey!=value.hotkey{
        let key=crate::shortcuts::parse(&value.hotkey)?;
        if app.global_shortcut().is_registered(key){return Err("Ese atajo ya está en uso".into());}
        crate::shortcuts::register(app,&value.hotkey)?;
    }
    if let Err(error)=app.state::<Store>().put("replay_preferences",&value){
        if old.hotkey!=value.hotkey&&!value.hotkey.is_empty(){let _=app.global_shortcut().unregister(crate::shortcuts::parse(&value.hotkey)?);}return Err(error);
    }
    if old.hotkey!=value.hotkey&&!old.hotkey.is_empty(){if let Ok(key)=crate::shortcuts::parse(&old.hotkey){let _=app.global_shortcut().unregister(key);}}
    stop(app);
    if value.enabled{if let Err(error)=start(app,value){change(app,|s|{s.phase="error".into();s.error=error;});}}else{change(app,|status|{status.phase="off".into();status.available_seconds=0.;status.error.clear();});}
    Ok(())
}
pub fn initialize(app:&tauri::AppHandle){
    let value=preferences(app);
    if !value.hotkey.is_empty(){if let Err(error)=crate::shortcuts::register(app,&value.hotkey){change(app,|s|s.error=error);return;}}
    if value.enabled{let app=app.clone();std::thread::spawn(move||{let state=app.state::<Replay>();let _gate=state.gate.lock();let value=preferences(&app);if let Err(error)=start_enabled(value,|value|start(&app,value)){change(&app,|s|{s.phase="error".into();s.error=error;});}});}
}
fn start_enabled(value:Preferences,start:impl FnOnce(Preferences)->Result<(),String>)->Result<(),String>{
    // Configuration may have changed while the startup worker waited for the gate.
    if !value.enabled{return Ok(());}
    value.validate()?;start(value)
}
fn start(app:&tauri::AppHandle,value:Preferences)->Result<(),String>{
    if app.state::<Replay>().worker.lock().map_err(|_|"Repetición ocupada")?.is_some(){return Ok(());}
    let binary=crate::screen::ffmpeg(app)?;
    let monitor=app.primary_monitor().map_err(|e|e.to_string())?.ok_or("No hay pantalla principal")?;
    let screen=Screen{x:monitor.position().x,y:monitor.position().y,width:monitor.size().width,height:monitor.size().height,name:monitor.name().cloned().unwrap_or_default()};
    let (tx,rx)=mpsc::channel();let app_worker=app.clone();
    change(app,|s|{s.phase="starting".into();s.error.clear();s.available_seconds=0.;});
    let thread=std::thread::spawn(move||{
        if let Err(error)=record(&app_worker,&value,&binary,screen,rx){change(&app_worker,|s|{s.phase="error".into();s.error=error;});}
    });
    *app.state::<Replay>().worker.lock().map_err(|_|"Repetición ocupada")?=Some(Worker{tx,thread});Ok(())
}
pub fn stop(app:&tauri::AppHandle){
    let worker=app.state::<Replay>().worker.lock().ok().and_then(|mut worker|worker.take());
    if let Some(worker)=worker{let _=worker.tx.send(Control::Stop);let _=worker.thread.join();}
}
pub fn save(app:&tauri::AppHandle)->Result<(),String>{
    let _work=crate::updates::work(app)?;
    let state=app.state::<Replay>();
    let permit=ExportPermit::acquire(state.exporting.clone())?;
    let result=(||{let worker=state.worker.lock().map_err(|_|"Repetición ocupada")?;
        let status=state.status.lock().map_err(|_|"Repetición ocupada")?;
        if status.phase!="buffering"||status.available_seconds<1.{return Err("La repetición todavía no está lista. Activala en Capturas y video".into());}
        worker.as_ref().ok_or("Activá la grabación hacia atrás")?.tx.send(Control::Save(permit)).map_err(|_|"La repetición se detuvo".to_string())
    })();
    result
}
#[tauri::command]
pub fn replay_save(app:tauri::AppHandle,window:tauri::WebviewWindow)->Result<(),String>{settings_window(&window)?;save(&app)}
pub fn report_error(app:&tauri::AppHandle,error:String){change(app,|s|s.error=error);}
pub(crate) fn command(binary:&Path)->Command{let mut command=Command::new(binary);command.creation_flags(0x08000000);command}
pub(crate) fn encoder_args(encoder:&str)->Vec<&'static str>{match encoder{
    "h264_nvenc"=>vec!["-c:v","h264_nvenc","-forced-idr","1","-preset","p4","-rc","vbr","-cq","20","-b:v","5M","-maxrate","8M","-bufsize","8M"],
    "h264_amf"=>vec!["-c:v","h264_amf","-quality","speed","-rc","cqp","-qp_i","20","-qp_p","22"],
    "h264_qsv"=>vec!["-c:v","h264_qsv","-forced_idr","1","-preset","veryfast","-global_quality","22"],
    _=>vec!["-c:v","libx264","-preset","veryfast","-crf","18"],
}}
pub(crate) fn hardware_encoder(binary:&Path)->Result<String,String>{
    static ENCODER:Mutex<Option<String>>=Mutex::new(None);
    let mut cached=ENCODER.lock().map_err(|_|"Codificador ocupado")?;if let Some(value)=cached.as_ref(){return Ok(value.clone());}
    for name in ["h264_nvenc","h264_amf","h264_qsv"]{
        let mut cmd=command(binary);cmd.args(["-hide_banner","-loglevel","error","-f","lavfi","-i","color=size=128x128:rate=30","-t","0.1","-an"]).args(encoder_args(name)).args(["-f","null","-"]).stdout(Stdio::null()).stderr(Stdio::null());
        if run_command(&mut cmd,Duration::from_secs(4)).is_ok(){*cached=Some(name.into());return Ok(name.into());}
    }
    Err("No hay codificador de video por hardware compatible. La grabación hacia atrás queda detenida para evitar una carga continua en la CPU".into())
}
pub(crate) fn probe(binary:&Path,path:&Path)->Result<String,String>{
    let mut command=command(binary);command.args(["-hide_banner","-i"]).arg(path).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    let mut process=Encoder::spawn(&mut command)?;let stderr=process.child.stderr.take().ok_or("No se pudo leer el video")?;
    let reader=std::thread::spawn(move||{let mut text=String::new();let _=stderr.take(65536).read_to_string(&mut text);text});
    let start=Instant::now();let mut timed_out=false;
    while process.child.try_wait().map_err(|e|e.to_string())?.is_none(){if start.elapsed()>Duration::from_secs(10){timed_out=true;break;}std::thread::sleep(Duration::from_millis(25));}
    drop(process);let text=reader.join().map_err(|_|"No se pudo leer el video")?;if timed_out{Err("No se pudo leer el video a tiempo".into())}else{Ok(text)}
}
pub(crate) fn run_command(command:&mut Command,timeout:Duration)->Result<(),String>{
    let mut process=Encoder::spawn(command)?;let start=Instant::now();
    loop{if let Some(exit)=process.child.try_wait().map_err(|e|e.to_string())?{return if exit.success(){Ok(())}else{Err("No se pudo preparar el video".into())};}
        if start.elapsed()>timeout{return Err("El video tardó demasiado en prepararse".into());}std::thread::sleep(Duration::from_millis(25));}
}
struct Screen{x:i32,y:i32,width:u32,height:u32,name:String}
fn dda_index(name:&str)->Option<u32>{
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1,IDXGIFactory1};
    unsafe{let factory:IDXGIFactory1=CreateDXGIFactory1().ok()?;let adapter=factory.EnumAdapters1(0).ok()?;
        for index in 0..16{let Ok(output)=adapter.EnumOutputs(index)else{break};let desc=output.GetDesc().ok()?;
            let len=desc.DeviceName.iter().position(|c|*c==0).unwrap_or(desc.DeviceName.len());if String::from_utf16_lossy(&desc.DeviceName[..len])==name{return Some(index);}}
    }None
}
fn dimensions(width:u32,height:u32)->(u32,u32){let scale=(1920./width.max(1) as f64).min(1080./height.max(1) as f64).min(1.);((width as f64*scale) as u32/2*2,(height as f64*scale) as u32/2*2)}
fn capture_command(binary:&Path,screen:&Screen,encoder:&str,dir:&Path,dda:Option<u32>)->Result<Command,String>{
    let mut cmd=command(binary);cmd.args(["-hide_banner","-loglevel","error","-y"]);
    if let Some(index)=dda{cmd.args(["-f","lavfi","-i"]).arg(format!("ddagrab=output_idx={index}:framerate=30:draw_mouse=1"));}
    else{cmd.args(["-f","gdigrab","-framerate","30","-probesize","32","-analyzeduration","0","-draw_mouse","1","-offset_x"]).arg(screen.x.to_string()).arg("-offset_y").arg(screen.y.to_string()).arg("-video_size").arg(format!("{}x{}",screen.width,screen.height)).args(["-i","desktop"]);}
    let (width,height)=dimensions(screen.width,screen.height);
    let filter=format!("{}scale={width}:{height},format=nv12,setpts=PTS-STARTPTS",if dda.is_some(){"hwdownload,format=bgra,"}else{""});
    cmd.args(["-vf",&filter,"-an"]).args(encoder_args(encoder)).args(["-g","30","-bf","0","-force_key_frames","expr:gte(t,n_forced*1)","-f","segment","-segment_time","1","-segment_format","mpegts","-reset_timestamps","1","-segment_list_type","csv","-segment_list_size","610","-segment_list"]).arg(dir.join("index.csv")).args(["-progress","pipe:1","-stats_period","0.25"]).arg(dir.join("part-%09d.ts")).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::from(fs::File::create(dir.join("capture.log")).map_err(|e|e.to_string())?));Ok(cmd)
}
struct Temporary{path:PathBuf}
impl Drop for Temporary{fn drop(&mut self){let _=fs::remove_dir_all(&self.path);}}
fn temp_dir(parent:&Path)->Result<Temporary,String>{
    let base=parent.join(".whispera-buffer");fs::create_dir_all(&base).map_err(|e|e.to_string())?;
    let actual=fs::canonicalize(&base).map_err(|e|e.to_string())?;let parent=fs::canonicalize(parent).map_err(|e|e.to_string())?;
    if actual.parent()!=Some(parent.as_path()){return Err("El buffer no puede ser un enlace a otra carpeta".into());}
    let path=actual.join(uuid::Uuid::new_v4().to_string());fs::create_dir(&path).map_err(|e|e.to_string())?;Ok(Temporary{path})
}
fn segment_number(path:&Path)->Option<u64>{let name=path.file_name()?.to_str()?;name.strip_prefix("part-")?.strip_suffix(".ts")?.parse().ok()}
fn segments(dir:&Path)->Result<Vec<(u64,PathBuf)>,String>{let mut parts=Vec::new();for entry in fs::read_dir(dir).map_err(|e|e.to_string())?{let path=entry.map_err(|e|e.to_string())?.path();if let Some(number)=segment_number(&path){parts.push((number,path));}}parts.sort_by_key(|part|part.0);Ok(parts)}
fn prune(dir:&Path,seconds:u32)->Result<(),String>{let parts=segments(dir)?;if let Some((latest,_))=parts.last(){for (number,path) in &parts{if latest.saturating_sub(*number)>seconds as u64+3{fs::remove_file(path).map_err(|e|e.to_string())?;}}}Ok(())}
fn record(app:&tauri::AppHandle,prefs:&Preferences,binary:&Path,screen:Screen,rx:mpsc::Receiver<Control>)->Result<(),String>{
    let encoder=hardware_encoder(binary)?;let temp=temp_dir(Path::new(&prefs.folder))?;
    let mut tracks=Vec::new();if ["system","both"].contains(&prefs.audio.as_str()){tracks.push(replay_audio::Audio::new(true,prefs.seconds)?);}if ["microphone","both"].contains(&prefs.audio.as_str()){tracks.push(replay_audio::Audio::new(false,prefs.seconds)?);}
    let mut process=Encoder::spawn(&mut capture_command(binary,&screen,&encoder,&temp.path,dda_index(&screen.name))?)?;
    let started=Instant::now();let elapsed=Arc::new(AtomicU64::new(0));let progress=elapsed.clone();
    let stdout=process.child.stdout.take().ok_or("No se pudo leer el buffer")?;
    let reader=std::thread::spawn(move||{for line in BufReader::new(stdout).lines().map_while(Result::ok){if let Some(value)=line.strip_prefix("out_time_us="){if let Ok(value)=value.parse::<u64>(){progress.store(value,Ordering::Relaxed);}}}});
    let mut last_prune=Instant::now();let mut first_time=None;
    let result=(||{loop{
        if process.child.try_wait().map_err(|e|e.to_string())?.is_some(){return Err("La captura en segundo plano se detuvo. Revisá el dispositivo o desactivá y volvé a activar la opción".into());}
        let time=elapsed.load(Ordering::Relaxed) as f64/1_000_000.;
        if first_time.is_none()&&time>0.{first_time=Some(Instant::now().checked_sub(Duration::from_secs_f64(time)).unwrap_or(started));}
        if started.elapsed()>Duration::from_secs(15)&&time==0.{return Err("La captura en segundo plano no pudo iniciar".into());}
        if tracks.iter().any(|track|!track.error().is_empty()){return Err("El dispositivo de audio de la repetición se desconectó".into());}
        if last_prune.elapsed()>Duration::from_secs(1){prune(&temp.path,prefs.seconds)?;last_prune=Instant::now();change(app,|s|{s.phase="buffering".into();s.available_seconds=time.min(prefs.seconds as f64);s.encoder=encoder.clone();});}
        match rx.recv_timeout(Duration::from_millis(100)){
            Ok(Control::Stop)|Err(mpsc::RecvTimeoutError::Disconnected)=>return Ok(()),
            Ok(Control::Save(permit))=>{
                let snapshot=(||{
                    let end=time;let fence=Instant::now();
                    // Finish the segment containing the requested frame before copying it.
                    // Capture continues; neither the requested end nor the audio interval moves.
                    while elapsed.load(Ordering::Relaxed) as f64/1_000_000.<end.ceil()+0.05 && fence.elapsed()<Duration::from_millis(1600){std::thread::sleep(Duration::from_millis(25));}
                    let begin=(end-prefs.seconds as f64).max(0.);if end-begin<1.{return Err("El buffer todavía no tiene suficiente video".into());}
                    let export=temp_dir(Path::new(&prefs.folder))?;let parts=segments(&temp.path)?;let mut selected=Vec::new();
                    for (number,path) in parts{if number as f64+1.>=begin&&number as f64<=end&&fs::metadata(&path).map_err(|e|e.to_string())?.len()>=188{let target=export.path.join(format!("part-{number:09}.ts"));fs::copy(path,&target).map_err(|e|e.to_string())?;selected.push((number,target));}}
                    if selected.is_empty(){return Err("El video todavía no está disponible".into());}
                    let first=selected[0].0 as f64;let begin=begin.max(first);let base=first_time.unwrap_or(started);
                    let audio=tracks.iter().map(|track|track.snapshot(base+Duration::from_secs_f64(begin),base+Duration::from_secs_f64(end))).collect::<Result<Vec<_>,_>>()?;
                    Ok((export,selected,begin-first,end-begin,audio))
                })();
                match snapshot{
                    Ok((export,parts,offset,duration,audio))=>{let app=app.clone();let binary=binary.to_owned();let folder=prefs.folder.clone();std::thread::spawn(move||{
                        let _permit=permit;
                        let result=export_clip(&binary,Path::new(&folder),&export.path,&parts,offset,duration,audio).and_then(|path|{
                            crate::library::remember_file(&app.state::<Store>(),&path)?;change(&app,|s|{s.last_path=path.to_string_lossy().into_owned();s.error.clear();});crate::video_trim::open_path(&app,&path)
                        });if let Err(error)=result{report_error(&app,error);}drop(export);
                    });}
                    Err(error)=>{report_error(app,error);drop(permit);}
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout)=>{}
        }
    }})();
    drop(process);let _=reader.join();result
}
fn export_clip(binary:&Path,folder:&Path,dir:&Path,parts:&[(u64,PathBuf)],offset:f64,duration:f64,audio:Vec<replay_audio::Snapshot>)->Result<PathBuf,String>{
    let list=parts.iter().map(|(_,path)|format!("file '{}'\n",path.file_name().unwrap().to_string_lossy())).collect::<String>();fs::write(dir.join("join.txt"),list).map_err(|e|e.to_string())?;
    let output=folder.join(format!("Whispera-{}.mp4",chrono::Local::now().format("%Y-%m-%d_%H-%M-%S%.3f")));let temporary=dir.join("result.mp4");
    // Seeking inside the concat demuxer's first TS segment can skip to its next
    // keyframe and silently shorten the requested interval. First build an indexed
    // MP4, without re-encoding; its edit list preserves the leading video packets.
    let joined=dir.join("joined.mp4");let join_log=dir.join("join.log");
    let mut join=command(binary);join.args(["-hide_banner","-loglevel","error","-y","-f","concat","-safe","1","-i","join.txt","-map","0:v:0","-c:v","copy","-an"]).arg(&joined).current_dir(dir).stdout(Stdio::null()).stderr(Stdio::from(fs::File::create(&join_log).map_err(|e|e.to_string())?));
    if let Err(error)=run_command(&mut join,Duration::from_secs(60)){let log=fs::read_to_string(join_log).unwrap_or_default();return Err(format!("{error}: {}",log.chars().take(1000).collect::<String>()));}
    let mut cmd=command(binary);cmd.args(["-hide_banner","-loglevel","error","-y","-ss",&format!("{offset:.3}"),"-i"]).arg(&joined).current_dir(dir);
    for (index,track) in audio.iter().enumerate(){let path=dir.join(format!("audio-{index}.pcm"));fs::write(&path,&track.bytes).map_err(|e|e.to_string())?;cmd.args(["-f","s16le","-ar"]).arg(track.rate.to_string()).arg("-ac").arg(track.channels.to_string()).arg("-i").arg(path);}
    // Seek the video only; each PCM track already contains exactly the selected interval.
    cmd.args(["-t",&format!("{duration:.3}"),"-map","0:v:0","-c:v","copy"]);
    if audio.len()==2{cmd.args(["-filter_complex","[1:a][2:a]amix=inputs=2:duration=longest:normalize=0,alimiter=limit=0.95[a]","-map","[a]"]);}else if audio.len()==1{cmd.args(["-map","1:a:0"]);}if !audio.is_empty(){cmd.args(["-c:a","aac","-b:a","192k","-ar","48000","-ac","2"]);}else{cmd.arg("-an");}
    cmd.args(["-movflags","+faststart"]).arg(&temporary).stdout(Stdio::null()).stderr(Stdio::from(fs::File::create(dir.join("export.log")).map_err(|e|e.to_string())?));if let Err(error)=run_command(&mut cmd,Duration::from_secs(60)){let log=fs::read_to_string(dir.join("export.log")).unwrap_or_default();return Err(format!("{error}: {}",log.chars().take(1000).collect::<String>()));}
    if fs::metadata(&temporary).map_err(|e|e.to_string())?.len()==0{return Err("La repetición quedó vacía".into());}fs::rename(temporary,&output).map_err(|e|e.to_string())?;Ok(output)
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn discarded_save_and_failed_worker_release_export_lock(){
        let flag=Arc::new(AtomicBool::new(false));let (tx,rx)=mpsc::channel();
        let permit=ExportPermit::acquire(flag.clone()).unwrap();assert!(ExportPermit::acquire(flag.clone()).is_err());
        assert!(tx.send(Control::Save(permit)).is_ok());assert!(flag.load(Ordering::SeqCst));
        drop(rx);assert!(!flag.load(Ordering::SeqCst));
        let permit=ExportPermit::acquire(flag.clone()).unwrap();assert!(tx.send(Control::Save(permit)).is_err());assert!(!flag.load(Ordering::SeqCst));
    }
    #[test]fn disabled_latest_preferences_never_start_capture(){
        start_enabled(Preferences::default(),|_|panic!("Capture started after being disabled")).unwrap();
        let mut invalid=Preferences::default();invalid.enabled=true;assert!(start_enabled(invalid,|_|panic!("Invalid settings started capture")).is_err());
    }
    #[test]fn only_explicitly_configured_replay_can_run(){assert!(!Preferences::default().enabled);let mut prefs=Preferences::default();prefs.enabled=true;assert!(prefs.validate().is_err());prefs.enabled=false;prefs.seconds=601;assert!(prefs.validate().is_err());prefs.seconds=5;assert!(prefs.validate().is_ok());}
    #[test]fn bounded_video_size_keeps_aspect_and_even_dimensions(){assert_eq!(dimensions(3840,2160),(1920,1080));assert_eq!(dimensions(1920,1080),(1920,1080));assert_eq!(dimensions(1080,1920),(606,1080));}
    #[test]fn cleanup_only_removes_old_owned_segments(){let root=std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());fs::create_dir(&root).unwrap();for number in 0..30{fs::write(root.join(format!("part-{number:09}.ts")),b"test").unwrap();}fs::write(root.join("keep.mp4"),b"keep").unwrap();prune(&root,5).unwrap();assert_eq!(segments(&root).unwrap().len(),9);assert!(root.join("keep.mp4").exists());fs::remove_dir_all(root).unwrap();}
}

#[cfg(test)]
mod media_tests {
    use super::*;
    fn binary()->PathBuf{Path::new(env!("CARGO_MANIFEST_DIR")).join("../node_modules/ffmpeg-static/ffmpeg.exe")}
    fn duration(binary:&Path,path:&Path)->f64{
        let output=command(binary).args(["-hide_banner","-i"]).arg(path).output().unwrap();
        crate::library_media::duration(&String::from_utf8_lossy(&output.stderr)).unwrap()
    }
    #[test]
    fn fractional_first_segment_keeps_all_selected_video_frames(){
        let root=std::env::temp_dir().join(format!("whispera-fractional-qa-{}",uuid::Uuid::new_v4()));fs::create_dir(&root).unwrap();
        let temp=temp_dir(&root).unwrap();let binary=binary();
        let mut cmd=command(&binary);cmd.args(["-v","error","-y","-f","lavfi","-i","testsrc2=size=320x180:rate=30","-t","8","-an","-c:v","libx264","-preset","ultrafast","-g","30","-bf","0","-force_key_frames","expr:gte(t,n_forced*1)","-f","segment","-segment_time","1","-segment_format","mpegts","-reset_timestamps","1"]).arg(temp.path.join("part-%09d.ts")).stdout(Stdio::null()).stderr(Stdio::null());
        run_command(&mut cmd,Duration::from_secs(30)).unwrap();let parts=segments(&temp.path).unwrap();
        let mut failures=Vec::new();
        for offset in [0.,0.1,0.35,0.63,0.85,0.99,1.,1.35]{
            let path=export_clip(&binary,&root,&temp.path,&parts,offset,5.,vec![]).unwrap();
            let total=duration(&binary,&path);
            let decoded=command(&binary).args(["-v","error","-i"]).arg(&path).args(["-map","0:v:0","-f","framemd5","-"]).output().unwrap();assert!(decoded.status.success());
            let frames=String::from_utf8_lossy(&decoded.stdout).lines().filter(|line|!line.starts_with('#')&&!line.trim().is_empty()).count();
            // Compare every decoded frame with a precise decode of the source.
            // Duration alone can pass while still dropping or repeating video.
            let reference=command(&binary).args(["-v","error","-f","concat","-safe","1","-i","join.txt","-map","0:v:0","-f","framemd5","-"]).current_dir(&temp.path).output().unwrap();assert!(reference.status.success());
            let hashes=|bytes:&[u8]|String::from_utf8_lossy(bytes).lines().filter(|line|!line.starts_with('#')&&!line.trim().is_empty()).map(|line|line.rsplit(',').next().unwrap().trim().to_owned()).collect::<Vec<_>>();
            let actual=hashes(&decoded.stdout);let original=hashes(&reference.stdout);
            let start=original.windows(actual.len()).position(|range|range==actual).expect("Export must contain unmodified consecutive source frames");
            // A request between frames rounds to a frame boundary, never a keyframe.
            assert!((start as f64/30.-offset).abs()<=1./30.+0.001,"Unexpected first frame {start} at offset {offset}");
            eprintln!("FRACTIONAL offset={offset} duration={total} frames={frames}");
            if (total-5.).abs()>0.04||frames!=150{failures.push((offset,total,frames));}
        }
        drop(temp);fs::remove_dir_all(root).unwrap();assert!(failures.is_empty(),"Selected video was lost: {failures:?}");
    }
    #[test]
    fn real_segments_export_and_precise_trim_keep_audio_and_duration(){
        let root=std::env::temp_dir().join(format!("whispera-qa-{}",uuid::Uuid::new_v4()));fs::create_dir(&root).unwrap();
        let temp=temp_dir(&root).unwrap();let binary=binary();let encoder=hardware_encoder(&binary).unwrap_or_else(|_|"libx264".into());
        let mut cmd=command(&binary);cmd.args(["-hide_banner","-loglevel","error","-y","-f","lavfi","-i","testsrc2=size=640x360:rate=30","-t","8","-an"]).args(encoder_args(&encoder)).args(["-g","30","-bf","0","-force_key_frames","expr:gte(t,n_forced*1)","-f","segment","-segment_time","1","-segment_format","mpegts","-reset_timestamps","1"]).arg(temp.path.join("part-%09d.ts")).stdout(Stdio::null()).stderr(Stdio::null());
        run_command(&mut cmd,Duration::from_secs(30)).unwrap();
        let rate=48000;let seconds=3.2;
        // Build signed PCM, exactly the selected interval, with a known tone.
        let bytes=(0..(rate as f64*seconds) as usize).flat_map(|i|(((i as f64*440.*std::f64::consts::TAU/rate as f64).sin()*16000.).round() as i16).to_le_bytes()).collect::<Vec<_>>();
        let path=export_clip(&binary,&root,&temp.path,&segments(&temp.path).unwrap(),1.35,seconds,vec![replay_audio::Snapshot{rate,channels:1,bytes}]).unwrap();
        let total=duration(&binary,&path);assert!((total-seconds).abs()<0.15,"Export duration {total}");
        let trimmed=root.join("trim.mp4");crate::video_trim::trim(&binary,&path,&trimmed,0.7,2.2).unwrap();
        let total=duration(&binary,&trimmed);assert!((total-1.5).abs()<0.12,"Trim duration {total}");
        let output=command(&binary).args(["-v","error","-i"]).arg(&trimmed).args(["-map","0:a:0","-f","s16le","-ar","48000","-ac","1","-"]).output().unwrap();assert!(output.status.success());
        let samples:Vec<_>=output.stdout.chunks_exact(2).map(|b|i16::from_le_bytes([b[0],b[1]])).collect();assert!(samples.len()>70000);assert!(samples[..24000].iter().any(|v|v.abs()>5000),"Audio at start was lost");assert!(samples[48000..70000].iter().any(|v|v.abs()>5000),"Audio at end was lost");
        drop(temp);fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod desktop_tests {
    use super::*;
    #[test]
    #[ignore = "Captures the real primary screen and optional loopback; run only with user permission"]
    fn real_primary_screen_loopback_bounded_buffer_and_encoder_shutdown(){
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics,SM_CXSCREEN,SM_CYSCREEN};
        let binary=Path::new(env!("CARGO_MANIFEST_DIR")).join("../node_modules/ffmpeg-static/ffmpeg.exe");
        let root=std::env::temp_dir().join(format!("whispera-live-qa-{}",uuid::Uuid::new_v4()));fs::create_dir(&root).unwrap();let temp=temp_dir(&root).unwrap();
        let screen=Screen{x:0,y:0,width:unsafe{GetSystemMetrics(SM_CXSCREEN)} as u32,height:unsafe{GetSystemMetrics(SM_CYSCREEN)} as u32,name:r"\\.\DISPLAY1".into()};
        let encoder=hardware_encoder(&binary).expect("Hardware encoder");let audio=replay_audio::Audio::new(true,5).expect("System loopback");
        let mut process=Encoder::spawn(&mut capture_command(&binary,&screen,&encoder,&temp.path,dda_index(&screen.name)).unwrap()).unwrap();let pid=process.child.id();
        let out=process.child.stdout.take().unwrap();let progress=Arc::new(AtomicU64::new(0));let elapsed=progress.clone();
        let reader=std::thread::spawn(move||{for line in BufReader::new(out).lines().map_while(Result::ok){if let Some(value)=line.strip_prefix("out_time_us="){if let Ok(value)=value.parse(){elapsed.store(value,Ordering::Relaxed);}}}});
        eprintln!("LIVE QA encoder={encoder} pid={pid}");let start=Instant::now();
        while start.elapsed()<Duration::from_secs(22){std::thread::sleep(Duration::from_secs(1));assert!(process.child.try_wait().unwrap().is_none(),"Encoder stopped: {}",fs::read_to_string(temp.path.join("capture.log")).unwrap_or_default());prune(&temp.path,5).unwrap();}
        let end=progress.load(Ordering::Relaxed) as f64/1_000_000.;assert!(end>15.,"Capture progress {end}");let parts=segments(&temp.path).unwrap();assert!(parts.len()<=9,"Buffer must stay bounded, {} files",parts.len());
        eprintln!("LIVE progress={end} parts={:?}",parts.iter().map(|(n,p)|(*n,fs::metadata(p).unwrap().len())).collect::<Vec<_>>());
        let export=temp_dir(&root).unwrap();let begin=end-5.;let mut selected=Vec::new();for (n,path) in parts{if n as f64+1.>=begin&&fs::metadata(&path).unwrap().len()>=188{let target=export.path.join(format!("part-{n:09}.ts"));fs::copy(path,&target).unwrap();selected.push((n,target));}}
        let first=selected[0].0 as f64;let now=Instant::now();let snapshot=audio.snapshot(now-Duration::from_secs(5),now).unwrap();assert!(snapshot.bytes.len()<=5*192000*8*2);assert!(audio.error().is_empty());
        let clip=export_clip(&binary,&root,&export.path,&selected,begin-first,5.,vec![snapshot]).unwrap();
        let info=probe(&binary,&clip).unwrap();let duration=crate::library_media::duration(&info).unwrap();assert!((duration-5.).abs()<0.3,"Live replay duration {duration}");assert!(info.contains("Audio:"));
        let trimmed=root.join("selected.mp4");crate::video_trim::trim(&binary,&clip,&trimmed,1.,3.).unwrap();let info=probe(&binary,&trimmed).unwrap();assert!((crate::library_media::duration(&info).unwrap()-2.).abs()<0.15);
        drop(process);reader.join().unwrap();let output=Command::new("powershell.exe").args(["-NoProfile","-Command",&format!("if(Get-Process -Id {pid} -ErrorAction SilentlyContinue){{exit 1}}")]).output().unwrap();assert!(output.status.success(),"Encoder survived shutdown");
        eprintln!("LIVE QA passed: bounded 5-second buffer, primary monitor, loopback, replay, precise trim, process shutdown");drop(audio);drop(export);drop(temp);fs::remove_dir_all(root).unwrap();
    }
}
