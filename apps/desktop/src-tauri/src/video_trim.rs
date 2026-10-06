//! Time-only video trimming. Every editor is bound to one authorized local video.
use crate::{library,storage::Store};
use serde::Serialize;
use std::{collections::HashMap,fs,path::{Path,PathBuf},process::Stdio,sync::{Mutex,atomic::{AtomicBool,Ordering}},time::Duration};
use tauri::{Manager,WebviewUrl,WebviewWindowBuilder};
#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Context{path:String,name:String,duration:f64,has_audio:bool}
struct Last{start:f64,end:f64,path:PathBuf}
#[derive(Default)]
pub struct Editors{contexts:Mutex<HashMap<String,Context>>,last:Mutex<HashMap<String,Last>>,pub busy:AtomicBool}
fn context(app:&tauri::AppHandle,label:&str)->Result<Context,String>{app.state::<Editors>().contexts.lock().map_err(|_|"Editor ocupado")?.get(label).cloned().ok_or("Editor de video no disponible".into())}
pub fn open_path(app:&tauri::AppHandle,path:&Path)->Result<(),String>{
    if !path.is_file()||!crate::library_media::is_video(path){return Err("Video no disponible".into());}
    let binary=crate::screen::ffmpeg(app)?;
    let info=crate::replay::probe(&binary,path)?;
    let duration=crate::library_media::duration(&info).ok_or("No se pudo leer la duración del video")?;
    let ctx=Context{path:path.to_string_lossy().into_owned(),name:path.file_name().unwrap_or_default().to_string_lossy().into_owned(),duration,has_audio:info.contains("Audio:")};
    let label=format!("video-trim-{}",uuid::Uuid::new_v4());
    app.asset_protocol_scope().allow_file(path).map_err(|e|e.to_string())?;
    app.state::<Editors>().contexts.lock().map_err(|_|"Editor ocupado")?.insert(label.clone(),ctx);
    let result=WebviewWindowBuilder::new(app,&label,WebviewUrl::App("index.html?view=video-trim".into())).title("Whispera · Recortar video").inner_size(640.,520.).min_inner_size(520.,440.).decorations(false).resizable(true).center().build();
    if let Err(error)=result{app.state::<Editors>().contexts.lock().map_err(|_|"Editor ocupado")?.remove(&label);return Err(error.to_string());}Ok(())
}
#[tauri::command]
pub async fn video_trim_open(id:String,app:tauri::AppHandle,window:tauri::WebviewWindow)->Result<(),String>{
    if window.label()!="main"&&!window.label().starts_with("library"){return Err("Abrí el video desde el Portapapeles".into());}
    tauri::async_runtime::spawn_blocking(move||{
        let item=library::rows(&app.state::<Store>())?.into_iter().find(|item|item["id"]==id).ok_or("Video fuera del portapapeles")?;
        let path=library::paths(&item["data"]).into_iter().find(|path|crate::library_media::is_video(path)).ok_or("Este elemento no contiene un video")?;open_path(&app,&path)
    }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub fn video_trim_context(app:tauri::AppHandle,window:tauri::WebviewWindow)->Result<Context,String>{context(&app,window.label())}
#[tauri::command]
pub fn video_trim_close(app:tauri::AppHandle,window:tauri::WebviewWindow)->Result<(),String>{
    context(&app,window.label())?;
    if app.state::<Editors>().busy.load(Ordering::SeqCst){return Err("Esperá a que termine de guardar el tramo".into());}
    app.state::<Editors>().contexts.lock().map_err(|_|"Editor ocupado")?.remove(window.label());window.destroy().map_err(|e|e.to_string())
}
pub fn destroyed(app:&tauri::AppHandle,label:&str){if let Ok(mut last)=app.state::<Editors>().last.lock(){last.remove(label);}if let Ok(mut contexts)=app.state::<Editors>().contexts.lock(){contexts.remove(label);}}
fn validate_range(start:f64,end:f64,duration:f64)->Result<(),String>{
    if !start.is_finite()||!end.is_finite()||start<0.||end-start<0.15||end>duration+0.05{return Err("Elegí un tramo válido de al menos 0,15 segundos".into());}Ok(())
}
pub(crate) fn trim(binary:&Path,source:&Path,output:&Path,start:f64,end:f64)->Result<(),String>{
    let mut command=crate::replay::command(binary);
    command.args(["-hide_banner","-loglevel","error","-n","-ss"]).arg(format!("{start:.3}")).arg("-i").arg(source).arg("-t").arg(format!("{:.3}",end-start)).args(["-map","0:v:0","-map","0:a?","-vf","format=nv12"]);
    let encoder=crate::replay::hardware_encoder(binary).unwrap_or_else(|_|"libx264".into());
    command.args(crate::replay::encoder_args(&encoder)).args(["-pix_fmt","yuv420p","-c:a","aac","-b:a","192k","-movflags","+faststart"]).arg(output).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    crate::replay::run_command(&mut command,Duration::from_secs(120))?;
    if fs::metadata(output).map_err(|e|e.to_string())?.len()==0{return Err("El tramo quedó vacío".into());}Ok(())
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct Saved{path:String,transcript:bool,warning:String}
#[tauri::command]
pub async fn video_trim_save(app:tauri::AppHandle,window:tauri::WebviewWindow,start:f64,end:f64)->Result<Saved,String>{
    let _work=crate::updates::work(&app)?;
    let ctx=context(&app,window.label())?;validate_range(start,end,ctx.duration)?;
    if app.state::<Editors>().busy.swap(true,Ordering::SeqCst){return Err("Ya se está guardando un tramo".into());}
    let job_app=app.clone();let label=window.label().to_string();
    let result=tauri::async_runtime::spawn_blocking(move||{
        let source=PathBuf::from(&ctx.path);let folder=source.parent().ok_or("Video sin carpeta")?;
        let previous=job_app.state::<Editors>().last.lock().map_err(|_|"Editor ocupado")?.get(&label).filter(|last|last.start==start&&last.end==end&&last.path.is_file()).map(|last|last.path.clone());
        let output=previous.clone().unwrap_or_else(||folder.join(format!("Whispera-{}_recorte.mp4",chrono::Local::now().format("%Y-%m-%d_%H-%M-%S%.3f"))));
        let binary=crate::screen::ffmpeg(&job_app)?;
        if previous.is_none(){if let Err(error)=trim(&binary,&source,&output,start,end){let _=fs::remove_file(&output);return Err(error);}
            job_app.state::<Editors>().last.lock().map_err(|_|"Editor ocupado")?.insert(label.clone(),Last{start,end,path:output.clone()});
        }
        library::remember_file(&job_app.state::<Store>(),&output)?;
        let mut warning=if ctx.has_audio && !crate::video_transcript::get(&job_app.state::<Store>(),&output).is_ok_and(|t|t.status=="ready"){crate::video_transcript::transcribe_selection(&job_app,&output).err().unwrap_or_default()}else{String::new()};
        if let Err(error)=crate::video_transcript::copy_selection(&job_app,&output){if !warning.is_empty(){warning.push_str(". ");}warning.push_str(&format!("Guardado, pero no se pudo copiar: {error}. Volvé a pulsar Guardar para reintentar."));}
        let transcript=ctx.has_audio&&warning.is_empty()&&output.with_extension("transcript.txt").is_file();
        Ok(Saved{path:output.to_string_lossy().into_owned(),transcript,warning})
    }).await.map_err(|e|e.to_string()).and_then(|result|result);
    app.state::<Editors>().busy.store(false,Ordering::SeqCst);result
}

#[cfg(test)]
mod tests{use super::*;#[test]fn range_rejects_invalid_or_outside_selection(){assert!(validate_range(1.,2.,5.).is_ok());for (start,end) in [(f64::NAN,3.),(-1.,3.),(3.,2.),(0.,8.),(0.,0.1)]{assert!(validate_range(start,end,5.).is_err());}}}
