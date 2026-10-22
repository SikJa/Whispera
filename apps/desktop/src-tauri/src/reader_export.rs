//! Explicit, local MP3 export. Separate synthesis never changes speaker playback.
use crate::reader::{self, Reading};
use std::{fs, io::Write, path::{Path, PathBuf}, process::{Command, Stdio}, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};
use tauri::{Emitter, Manager};

#[derive(Default)]
pub struct Exports(Mutex<Option<Arc<AtomicBool>>>);
pub fn busy(app: &tauri::AppHandle) -> bool {
    app.state::<Exports>().0.lock().map(|slot|slot.is_some()).unwrap_or(true)
}
pub fn cancel(app: &tauri::AppHandle) {
    if let Ok(slot) = app.state::<Exports>().0.lock() {
        if let Some(token) = slot.as_ref() { token.store(true, Ordering::SeqCst); }
    }
}
pub fn sync_tray(app: &tauri::AppHandle) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let Some(menu) = app.try_state::<reader::TrayMenu>() else { return; };
        let exporting = busy(&app);
        let _ = menu.export.set_text(if exporting {"Guardando MP3…"} else {"Guardar lectura como MP3…"});
        let _ = menu.export.set_enabled(!exporting && reader::current(&app).is_some());
        let _ = menu.cancel_export.set_enabled(exporting);
    });
}
struct Job(tauri::AppHandle);
impl Drop for Job {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.0.state::<Exports>().0.lock() { *slot = None; }
        sync_tray(&self.0);
    }
}
struct Staging(PathBuf);
impl Drop for Staging { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn target(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || !path.extension().and_then(|ext|ext.to_str()).is_some_and(|ext|ext.eq_ignore_ascii_case("mp3")) ||
        path.file_name().is_none_or(|name|name.to_string_lossy().contains(':')) || path.is_dir() {
        return Err("Elegí un archivo con extensión .mp3.".into());
    }
    Ok(())
}
fn check(token: &AtomicBool) -> Result<(), String> {
    if token.load(Ordering::SeqCst) { Err("Guardado MP3 cancelado.".into()) } else { Ok(()) }
}
#[cfg(windows)]
fn run(command: &mut Command, input: Option<&[u8]>, token: &AtomicBool, timeout: Duration) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    check(token)?;
    command.creation_flags(0x08000000).stdin(if input.is_some(){Stdio::piped()}else{Stdio::null()})
        .stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = crate::encoder_job::Encoder::spawn(command).map_err(|_|"No se pudo iniciar el guardado MP3.")?;
    let writes = if let Some(input) = input {
        let mut stdin = child.child.stdin.take().ok_or("Motor de voz sin entrada.")?;
        let input = input.to_vec();
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        // A stalled voice engine must not block cancellation while writing stdin.
        std::thread::spawn(move || { let _ = send.send(stdin.write_all(&input)); });
        Some(receive)
    } else { None };
    let deadline = Instant::now() + timeout;
    loop {
        check(token)?;
        if writes.as_ref().is_some_and(|receive|matches!(receive.try_recv(),Ok(Err(_)))) {
            return Err("El motor de voz se desconectó.".into());
        }
        if let Some(status) = child.child.try_wait().map_err(|_|"No se pudo comprobar el guardado.")? {
            return if status.success() {Ok(())} else {Err("No se pudo generar el audio. Comprobá la voz elegida y el espacio disponible.".into())};
        }
        if Instant::now() >= deadline { return Err("El guardado tardó demasiado. Probá un texto más corto.".into()); }
        std::thread::sleep(Duration::from_millis(100));
    }
}
#[cfg(windows)]
fn render(app: &tauri::AppHandle, reading: &Reading, destination: &Path, token: &AtomicBool) -> Result<(), String> {
    target(destination)?;
    check(token)?;
    let parent = destination.parent().ok_or("Carpeta de destino no válida.")?;
    // On the destination volume, so final rename never leaves a partial MP3.
    let staging_path = parent.join(format!(".whispera-audio-{}",uuid::Uuid::new_v4()));
    fs::create_dir(&staging_path).map_err(|_|"No se pudo preparar el audio en esa carpeta.")?;
    let staging = Staging(staging_path);
    let wave = staging.0.join("speech.wav");
    let mp3 = staging.0.join("speech.mp3");
    let input = serde_json::to_vec(&serde_json::json!({"action":"render-wave","text":reading.text,"voice":reading.voice,"rate":reading.rate,"path":wave}))
        .map_err(|_|"Solicitud de audio no válida.")?;
    let mut line = input; line.push(b'\n');
    run(Command::new(reader::binary(app)?).arg("--render-wave"),Some(&line),token,Duration::from_secs(600))?;
    // No metadata from transcripts, filenames, sessions or account information.
    let mut encoder = Command::new(crate::screen::ffmpeg(app)?);
    encoder.args(["-hide_banner","-loglevel","error","-nostdin","-n","-i"]).arg(&wave)
        .args(["-map","0:a:0","-vn","-map_metadata","-1","-c:a","libmp3lame","-b:a","128k","-f","mp3"]).arg(&mp3);
    run(&mut encoder,None,token,Duration::from_secs(120))?;
    if fs::metadata(&mp3).map_err(|_|"No se generó el MP3.")?.len() == 0 { return Err("El MP3 quedó vacío.".into()); }
    check(token)?;
    fs::rename(&mp3,destination).map_err(|_|"No se pudo guardar el MP3. El archivo anterior se conserva; revisá los permisos o si está abierto.".into())
}
pub fn save(app: &tauri::AppHandle, reading: Option<Reading>) -> Result<Option<String>, String> {
    if !cfg!(windows) { return Err("Guardar lecturas requiere Windows en esta versión.".into()); }
    let reading = match reading {
        Some(reading)=>reader::resolve_reading(app,reading)?,
        None=>reader::current(app).ok_or("Primero leé un texto o escribilo en Lectura.")?,
    };
    reader::validate(&reading.text,reading.rate)?;
    if reading.voice.is_empty() || reading.voice.len() > 512 { return Err("Elegí una voz instalada.".into()); }
    let token = Arc::new(AtomicBool::new(false));
    {
        let state = app.state::<Exports>();
        let mut slot = state.0.lock().map_err(|_|"Guardado ocupado.")?;
        if slot.is_some() { return Err("Ya se está guardando un MP3.".into()); }
        *slot = Some(token.clone());
    }
    let _job = Job(app.clone());
    sync_tray(app);
    let _work = crate::updates::work(app)?;
    use tauri_plugin_dialog::DialogExt;
    let Some(picked) = app.dialog().file().set_title("Guardar lectura como MP3")
        .set_file_name("Whispera-lectura.mp3").add_filter("Audio MP3", &["mp3"]).blocking_save_file()
        else { return Ok(None); };
    let destination = picked.into_path().map_err(|_|"Elegí una carpeta local para el MP3.")?;
    if token.load(Ordering::SeqCst) {return Ok(None);}
    #[cfg(windows)]
    if let Err(error) = render(app,&reading,&destination,&token) {
        if token.load(Ordering::SeqCst) {return Ok(None);}
        return Err(error);
    }
    let path = destination.to_string_lossy().into_owned();
    let _ = app.emit_to("main","reader-export-saved",&path);
    Ok(Some(path))
}
#[tauri::command]
pub async fn reader_export_mp3(app: tauri::AppHandle, window: tauri::WebviewWindow, reading: Option<Reading>) -> Result<Option<String>, String> {
    reader::authorize(&window)?;
    tauri::async_runtime::spawn_blocking(move || save(&app,reading)).await.map_err(|_|"No se pudo completar el guardado MP3.".to_string())?
}
#[tauri::command]
pub fn reader_export_cancel(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), String> {
    reader::authorize(&window)?; cancel(&app); Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn reader_export_rejects_wrong_extension_and_cleans_staging() {
        if !cfg!(windows) {return;}
        assert!(target(Path::new(r"C:\Users\test\lecture.mp3")).is_ok());
        for value in [r"lecture.mp3",r"C:\Users\test\lecture.wav",r"C:\Users\test\lecture.mp3:stream"] {assert!(target(Path::new(value)).is_err());}
        let directory = std::env::temp_dir().join(format!("whispera-reader-export-{}",uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        { let staging = Staging(directory.clone());fs::write(staging.0.join("speech.wav"),b"private fixture").unwrap(); }
        assert!(!directory.exists());
        assert!(check(&AtomicBool::new(true)).is_err());
    }
    #[cfg(windows)]
    #[test] fn reader_export_can_cancel_a_stalled_own_process() {
        let token = Arc::new(AtomicBool::new(false));
        let cancel = token.clone();
        let trigger = std::thread::spawn(move || {std::thread::sleep(Duration::from_millis(150));cancel.store(true,Ordering::SeqCst);});
        let shell = PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        // Deliberately never reads its input, exercising cancellation of pipe writes too.
        let result = run(Command::new(shell).args(["-NoProfile","-NonInteractive","-Command","Start-Sleep -Seconds 30"]),Some(&vec![b'x';300000]),&token,Duration::from_secs(10));
        trigger.join().unwrap();
        assert!(result.unwrap_err().contains("cancelado"));
    }
}
