use std::sync::{atomic::{AtomicBool,AtomicUsize,Ordering},Mutex};
use std::time::{Duration,Instant};
use serde::Serialize;
use tauri::{Emitter,Manager};
use tauri_plugin_updater::{Update,UpdaterExt};

#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Status {
    pub current_version:String,pub version:Option<String>,pub notes:String,
    pub phase:String,pub downloaded:u64,pub total:Option<u64>,pub error:String,
}
pub struct Updates {
    status:Mutex<Status>,available:Mutex<Option<Update>>,operation:AtomicBool,
    installing:AtomicBool,work:AtomicUsize,
}
pub struct UpdateMenu(pub tauri::menu::MenuItem<tauri::Wry>);
impl Default for Updates {
    fn default()->Self {Self {
        status:Mutex::new(Status{current_version:env!("CARGO_PKG_VERSION").into(),version:None,notes:String::new(),phase:"idle".into(),downloaded:0,total:None,error:String::new()}),
        available:Mutex::new(None),operation:AtomicBool::new(false),installing:AtomicBool::new(false),work:AtomicUsize::new(0),
    }}
}
pub struct Work<'a>(&'a Updates);
impl Drop for Work<'_>{fn drop(&mut self){self.0.work.fetch_sub(1,Ordering::SeqCst);}}
fn enter(state:&Updates)->Result<Work<'_>,String>{
    if state.installing.load(Ordering::SeqCst){return Err("Whispera se está actualizando. Esperá a que vuelva a abrirse.".into());}
    state.work.fetch_add(1,Ordering::SeqCst);
    let guard=Work(state);
    if state.installing.load(Ordering::SeqCst){return Err("Whispera se está actualizando. Esperá a que vuelva a abrirse.".into());}
    Ok(guard)
}
pub fn work(app:&tauri::AppHandle)->Result<Work<'_>,String>{enter(app.state::<Updates>().inner())}
fn snapshot(app:&tauri::AppHandle)->Status {app.state::<Updates>().status.lock().unwrap_or_else(|e|e.into_inner()).clone()}
fn change(app:&tauri::AppHandle,apply:impl FnOnce(&mut Status)) {
    let state=app.state::<Updates>();
    let value={let mut s=state.status.lock().unwrap_or_else(|e|e.into_inner());apply(&mut s);s.clone()};
    let _=app.emit_to("main","updater-status",value);
    if let Some(menu)=app.try_state::<UpdateMenu>() {
        let status=snapshot(app);
        if ["available","current","error"].contains(&status.phase.as_str()) {
            let label=status.version.map(|v|format!("Actualizar Whispera ({v})")).unwrap_or_else(||"Buscar actualizaciones".into());
            let _=menu.0.set_text(label);
        }
    }
}
struct Operation<'a>(&'a AtomicBool);
impl Drop for Operation<'_>{fn drop(&mut self){self.0.store(false,Ordering::SeqCst);}}
fn operation(flag:&AtomicBool)->Result<Operation<'_>,String>{
    flag.compare_exchange(false,true,Ordering::SeqCst,Ordering::SeqCst).map_err(|_|"Ya hay una consulta o actualización en curso")?;
    Ok(Operation(flag))
}
fn trusted(url:&str)->bool {url.starts_with("https://github.com/kazu00001/Whispera-K/releases/download/")}

#[tauri::command]
pub fn updater_status(app:tauri::AppHandle)->Status {snapshot(&app)}

#[tauri::command]
pub async fn updater_check(app:tauri::AppHandle)->Result<Status,String> {
    let state=app.state::<Updates>();let _operation=operation(&state.operation)?;
    change(&app,|s|{s.phase="checking".into();s.error.clear();});
    let result=async {
        let exe=std::env::current_exe().map_err(|e|e.to_string())?;
        let directory=exe.parent().ok_or("No se pudo localizar la aplicación")?;
        let exit_app=app.clone();
        let update=app.updater_builder().timeout(Duration::from_secs(20))
            .installer_arg(format!("/D={}",directory.display()))
            .on_before_exit(move||crate::engine::shutdown(&exit_app))
            .build().map_err(|e|e.to_string())?.check().await.map_err(|e|e.to_string())?;
        if let Some(ref u)=update {if !trusted(u.download_url.as_str()){return Err("La actualización no proviene del repositorio de Whispera (K)".into());}}
        change(&app,|s|{s.version=update.as_ref().map(|u|u.version.clone());s.notes=update.as_ref().and_then(|u|u.body.clone()).unwrap_or_default();s.phase=if update.is_some(){"available"}else{"current"}.into();s.downloaded=0;s.total=None;});
        *state.available.lock().map_err(|_|"Actualizador ocupado")?=update;
        Ok(snapshot(&app))
    }.await;
    if let Err(ref e)=result {change(&app,|s|{s.phase="error".into();s.error=format!("No se pudo consultar la actualización. Revisá tu conexión y volvé a intentar. ({e})");});}
    result
}
fn idle(app:&tauri::AppHandle)->Result<(),String> {
    let engine=app.state::<crate::engine::Engine>();
    if crate::health::busy(&engine.recorder.snapshot().phase)||engine.processing.load(Ordering::SeqCst)||crate::screen::busy(app)||app.state::<Updates>().work.load(Ordering::SeqCst)>0 {
        return Err("Terminá el dictado, la transcripción o la captura antes de actualizar. Tu trabajo sigue abierto.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn updater_install(app:tauri::AppHandle,window:tauri::WebviewWindow)->Result<(),String>{
    if window.label()!="main"{return Err("Abrí Configuración para actualizar".into());}
    let state=app.state::<Updates>();let _operation=operation(&state.operation)?;
    idle(&app)?;
    let mut update=state.available.lock().map_err(|_|"Actualizador ocupado")?.clone().ok_or("Buscá una actualización primero")?;
    update.timeout=Some(Duration::from_secs(600));
    change(&app,|s|{s.phase="downloading".into();s.error.clear();s.downloaded=0;s.total=None;});
    let result=async {
        let mut downloaded=0u64;let mut last=Instant::now()-Duration::from_secs(1);
        let bytes=update.download(|chunk,total|{
            downloaded+=chunk as u64;
            if last.elapsed()>=Duration::from_millis(150){last=Instant::now();change(&app,|s|{s.downloaded=downloaded;s.total=total;});}
        },||{}).await.map_err(|e|format!("No se pudo descargar o verificar la actualización: {e}"))?;
        let handle=app.clone();
        tauri::async_runtime::spawn_blocking(move||{
            let state=handle.state::<Updates>();
            state.installing.store(true,Ordering::SeqCst);
            let _reset=Operation(&state.installing);
            idle(&handle)?;
            // Backup committed SQLite pages, including WAL, before the installer starts.
            let path=crate::profile::database_path()?;
            let backup_dir=path.parent().ok_or("Perfil no disponible")?.join("update-backups");
            std::fs::create_dir_all(&backup_dir).map_err(|e|e.to_string())?;
            let backup=backup_dir.join(format!("before-{}-{}.sqlite",update.version,chrono::Utc::now().format("%Y%m%d-%H%M%S")));
            handle.state::<crate::storage::Store>().0.lock().map_err(|_|"Configuración ocupada")?.backup(rusqlite::DatabaseName::Main,&backup,None).map_err(|e|format!("No se pudo respaldar la configuración: {e}"))?;
            change(&handle,|s|{s.phase="installing".into();s.downloaded=bytes.len() as u64;s.total=Some(bytes.len() as u64);});
            update.install(bytes).map_err(|e|format!("No se pudo iniciar la instalación: {e}"))
        }).await.map_err(|e|e.to_string())?
    }.await;
    if let Err(ref e)=result {change(&app,|s|{s.phase="available".into();s.error=e.clone();});}
    result
}
pub fn start(app:&tauri::AppHandle){
    let app=app.clone();
    tauri::async_runtime::spawn(async move{
        tokio::time::sleep(Duration::from_secs(8)).await;
        loop {
            let _=updater_check(app.clone()).await;
            tokio::time::sleep(Duration::from_secs(6*60*60)).await;
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]fn new_work_cannot_race_with_installation(){let state=Updates::default();let guard=enter(&state).unwrap();state.installing.store(true,Ordering::SeqCst);assert!(enter(&state).is_err());assert_eq!(state.work.load(Ordering::SeqCst),1);drop(guard);assert_eq!(state.work.load(Ordering::SeqCst),0);state.installing.store(false,Ordering::SeqCst);assert!(enter(&state).is_ok());}
    #[test]fn releases_only_come_from_this_repository(){assert!(trusted("https://github.com/kazu00001/Whispera-K/releases/download/v0.2.15/Whispera.exe"));for u in ["http://github.com/kazu00001/Whispera-K/releases/download/x", "https://github.com.evil.test/kazu00001/Whispera-K/releases/download/x","https://github.com/another/repo/releases/download/x"]{assert!(!trusted(u));}}
    #[test]
    fn native_updater_download_accepts_signed_bytes_and_rejects_tampering() {
        use std::io::{Read,Write};
        const PAYLOAD:&[u8]=include_bytes!("../tests/updater/payload.txt");
        for corrupt in [false,true] {
            let server=std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address=server.local_addr().unwrap();
            let url=format!("http://{address}/latest.json");
            let worker=std::thread::spawn(move||{
                for socket in server.incoming().take(2) {
                    let mut socket=socket.unwrap();socket.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
                    let mut request=[0;4096];let n=socket.read(&mut request).unwrap();
                    let manifest=String::from_utf8_lossy(&request[..n]).starts_with("GET /latest.json ");
                    let mut bytes=if manifest {serde_json::to_vec(&serde_json::json!({"version":"9999.0.0","platforms":{"windows-x86_64":{"url":format!("http://{address}/payload"),"signature":include_str!("../tests/updater/payload.txt.sig").trim()}}})).unwrap()}else{PAYLOAD.to_vec()};
                    if corrupt&&!manifest{bytes[0]^=1;}
                    write!(socket,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",bytes.len(),if manifest{"application/json"}else{"application/octet-stream"}).unwrap();
                    socket.write_all(&bytes).unwrap();
                }
            });
            let config:serde_json::Value=serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
            let mut context=tauri::test::mock_context(tauri::test::noop_assets());
            context.config_mut().plugins.0.insert("updater".into(),config["plugins"]["updater"].clone());
            let app=tauri::test::mock_builder().plugin(tauri_plugin_updater::Builder::new().build()).build(context).unwrap();
            tauri::async_runtime::block_on(async {
                let update=app.updater_builder().pubkey(config["plugins"]["updater"]["pubkey"].as_str().unwrap()).no_proxy().timeout(Duration::from_secs(10)).endpoints(vec![url.parse().unwrap()]).unwrap().build().unwrap().check().await.unwrap().unwrap();
                let downloaded=update.download(|_,_|{},||{}).await;
                if corrupt {assert!(downloaded.is_err(),"modified signed payload must be rejected");}
                else {assert_eq!(downloaded.unwrap(),PAYLOAD);}
            });
            worker.join().unwrap();
        }
    }
}
