//! Read-only provider quotas. No inference, credential copies or chat history.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, io::{BufRead, BufReader, Read, Write}, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::{mpsc, Mutex}, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
use tauri::Manager;

const MAX_FILE: u64 = 262144;
#[derive(Clone, Default, Serialize)]
#[serde(rename_all="camelCase")]
pub struct Window { pub label:String, pub used_percent:f64, pub reset_at:Option<u64>, pub window_minutes:Option<u64> }
#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
pub struct Credit { pub label:String, pub balance:Option<String>, pub unlimited:bool }
#[derive(Clone, Default, Serialize)]
#[serde(rename_all="camelCase")]
pub struct Provider {
    pub id:String, pub name:String, pub status:String, pub message:String,
    pub observed_at:Option<u64>, pub plan:Option<String>, pub windows:Vec<Window>, pub credits:Vec<Credit>,
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct Connection { pub connected:bool, pub settings_path:String }
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct Snapshot { pub providers:Vec<Provider>, pub claude_connection:Connection }
#[derive(Default)]
pub struct Usage { cache:Mutex<Option<(Instant,Provider)>>, refresh:Mutex<()>, connection:Mutex<()> }

fn now()->u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
fn authorize(window:&tauri::WebviewWindow)->Result<(),String> {
    if window.label()!="main" { return Err("Abrí Uso de IA desde Configuración.".into()); } Ok(())
}
fn root(app:&tauri::AppHandle)->Result<PathBuf,String> { Ok(app.path().app_data_dir().map_err(|_|"Perfil no disponible.")?.join("ai-usage")) }
fn read_json(path:&Path)->Result<Value,String> {
    let file=fs::File::open(path).map_err(|_|"Archivo de conexión no disponible.")?;
    let mut bytes=vec![]; file.take(MAX_FILE+1).read_to_end(&mut bytes).map_err(|_|"No se pudo leer la conexión.")?;
    if bytes.len() as u64>MAX_FILE { return Err("El archivo de conexión es demasiado grande.".into()); }
    serde_json::from_slice(bytes.strip_prefix(&[0xef,0xbb,0xbf]).unwrap_or(&bytes)).map_err(|_|"El archivo de conexión no es JSON válido.".into())
}
fn write_atomic(path:&Path, bytes:&[u8])->Result<(),String> {
    let parent=path.parent().ok_or("Ruta no válida.")?;
    fs::create_dir_all(parent).map_err(|_|"No se pudo crear la carpeta de conexión.")?;
    let temporary=parent.join(format!(".whispera-{}.tmp",uuid::Uuid::new_v4()));
    let result:Result<(),String>=(|| {
        let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(|_|"No se pudo preparar la conexión.")?;
        file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|_|"No se pudo guardar la conexión.")?;
        fs::rename(&temporary,path).map_err(|_|"No se pudo actualizar la conexión.".to_string())
    })();
    let _=fs::remove_file(&temporary); result
}
fn save_json(path:&Path,value:&Value)->Result<(),String> { write_atomic(path,&serde_json::to_vec_pretty(value).map_err(|_|"Conexión no válida.")?) }
fn number(value:&Value)->Option<f64> { value.as_f64().filter(|v|v.is_finite()&&*v>=0.0) }
fn timestamp(value:&Value)->Option<u64> { value.as_u64().filter(|v|*v>0&&*v<4102444800) }
fn short(value:&Value)->Option<String> { value.as_str().filter(|s|!s.is_empty()&&s.len()<=80&&!s.chars().any(char::is_control)).map(str::to_owned) }
fn provider(id:&str,name:&str,message:&str)->Provider { Provider {id:id.into(),name:name.into(),status:"unavailable".into(),message:message.into(),..Default::default()} }

fn codex_snapshot(value:&Value,observed:u64)->Result<Provider,String> {
    let mut result=provider("codex","OpenAI · Codex","");
    let buckets:Vec<(&str,&Value)>=if let Some(map)=value["rateLimitsByLimitId"].as_object().filter(|m|!m.is_empty()) {
        map.iter().map(|(id,b)|(id.as_str(),b)).collect()
    } else { vec![("codex",&value["rateLimits"])] };
    for (id,bucket) in buckets {
        let label=short(&bucket["limitName"]).unwrap_or_else(||if id=="codex"{"Codex".into()}else{id.chars().take(60).collect()});
        if result.plan.is_none() { result.plan=short(&bucket["planType"]); }
        for (field,name) in [("primary","Principal"),("secondary","Secundaria")] {
            let window=&bucket[field];
            if let Some(used)=number(&window["usedPercent"]) {
                result.windows.push(Window{label:format!("{label} · {name}"),used_percent:used,reset_at:timestamp(&window["resetsAt"]),window_minutes:window["windowDurationMins"].as_u64().filter(|v|*v>0)});
            }
        }
        let credit=&bucket["credits"];
        let balance=short(&credit["balance"]).filter(|s|s.parse::<f64>().is_ok_and(|v|v.is_finite()&&v>=0.0));
        if balance.is_some()||credit["unlimited"].as_bool()==Some(true) {
            result.credits.push(Credit{label:format!("Créditos · {label}"),balance,unlimited:credit["unlimited"]==true});
        }
    }
    if result.windows.is_empty()&&result.credits.is_empty() { return Err("Codex no devolvió cuotas para esta cuenta. Comprobá el inicio de sesión con ChatGPT.".into()); }
    result.status="ready".into(); result.observed_at=Some(observed); Ok(result)
}
fn claude_snapshot(value:&Value,current:u64)->Result<Provider,String> {
    if value["schema"]!=1 { return Err("Formato de cuotas de Claude no compatible.".into()); }
    let observed=timestamp(&value["observedAt"]).filter(|t|*t<=current+60).ok_or("Fecha de cuotas de Claude no válida.")?;
    let mut result=provider("claude","Anthropic · Claude","");
    result.observed_at=Some(observed);
    for (key,label,minutes) in [("five_hour","Sesión",Some(300)),("seven_day","Semana",Some(10080)),("spend_limit","Límite de gasto del gateway",None)] {
        let window=&value["rate_limits"][key];
        if let Some(used)=number(&window["used_percentage"]) {
            result.windows.push(Window{label:label.into(),used_percent:used,reset_at:timestamp(&window["resets_at"]),window_minutes:minutes});
            if key=="spend_limit" {
                if let (Some(spent),Some(limit))=(number(&window["used_usd"]),number(&window["limit_usd"])) {
                    result.credits.push(Credit{label:"Disponible dentro del límite del gateway (USD)".into(),balance:Some(format!("{:.2}",(limit-spent).max(0.0))),unlimited:false});
                }
            }
        }
    }
    result.status=if result.windows.is_empty(){"waiting"}else{"ready"}.into();
    result.message="Datos recibidos de Claude Code; una nueva respuesta actualiza sus cuotas. Las sesiones de CLI, VS Code y app comparten la cuota si usan la misma cuenta.".into();
    Ok(result)
}

struct Process(Child);
impl Drop for Process { fn drop(&mut self){let _=self.0.kill();let _=self.0.wait();} }
fn hidden(command:&mut Command) {
    #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
    #[cfg(not(windows))] let _=command;
}
#[cfg(windows)]
fn codex_engine()->Result<PathBuf,String> {
    let shell=PathBuf::from(std::env::var_os("WINDIR").ok_or("Windows no disponible.")?).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let script="$p=Get-AppxPackage -Name OpenAI.Codex | Sort-Object Version -Descending | Select-Object -First 1; if(-not $p -or $p.SignatureKind -ne 'Store' -or $p.IsDevelopmentMode){exit 1}; $e=Join-Path $p.InstallLocation 'app/resources/codex.exe'; if(Test-Path -LiteralPath $e){[Console]::Write($e)}else{exit 1}";
    let mut command=Command::new(shell);command.args(["-NoProfile","-NonInteractive","-Command",script]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());hidden(&mut command);
    let mut child=Process(command.spawn().map_err(|_|"No se pudo localizar Codex.")?);
    let stdout=child.0.stdout.take().ok_or("Codex no disponible.")?;
    let (tx,rx)=mpsc::sync_channel(1);
    std::thread::spawn(move||{let mut out=String::new();let _=stdout.take(8192).read_to_string(&mut out);let _=tx.send(out);});
    let output=rx.recv_timeout(Duration::from_secs(10)).map_err(|_|"La búsqueda de Codex tardó demasiado.")?;
    let path=PathBuf::from(output.trim().trim_start_matches('\u{feff}'));
    if !path.is_absolute()||!path.is_file(){return Err("Instalá o abrí la aplicación oficial de Codex e iniciá sesión con ChatGPT.".into());}Ok(path)
}
#[cfg(not(windows))]
fn codex_engine()->Result<PathBuf,String>{Err("Esta vista conecta las aplicaciones de Windows.".into())}
fn query_codex(directory:&Path)->Result<Provider,String> {
    let engine=codex_engine()?;
    fs::create_dir_all(directory).map_err(|_|"No se pudo preparar la consulta de Codex.")?;
    let mut command=Command::new(engine);
    command.args(["app-server","--listen","stdio://","--disable","memories"]).current_dir(directory).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());hidden(&mut command);
    let mut child=Process(command.spawn().map_err(|_|"No se pudo iniciar la consulta de Codex.")?);
    let mut input=child.0.stdin.take().ok_or("Codex no disponible.")?;
    let output=child.0.stdout.take().ok_or("Codex no disponible.")?;
    let (tx,rx)=mpsc::sync_channel(32);
    std::thread::spawn(move||{
        let mut reader=BufReader::new(output.take(1048576));
        loop {let mut line=String::new();match reader.read_line(&mut line){Ok(0)|Err(_)=>break,_=>{if tx.try_send(line).is_err(){break;}}}}
    });
    let deadline=Instant::now()+Duration::from_secs(15);
    let receive=|id:u64|->Result<Value,String>{loop{
        let line=rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).map_err(|_|"Codex no respondió a tiempo. Volvé a intentar.")?;
        let Ok(message)=serde_json::from_str::<Value>(line.trim_start_matches('\u{feff}')) else {continue;};
        if message["id"].as_u64()!=Some(id){continue;}
        if message.get("error").is_some(){return Err("No se pudieron consultar las cuotas de Codex. Comprobá tu sesión de ChatGPT.".into());}
        return Ok(message["result"].clone());
    }};
    writeln!(input,"{}",json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"whispera_usage","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":true}}})).map_err(|_|"Codex se desconectó.")?;
    input.flush().map_err(|_|"Codex se desconectó.")?;receive(1)?;
    writeln!(input,"{}",json!({"method":"initialized"})).map_err(|_|"Codex se desconectó.")?;
    writeln!(input,"{}",json!({"id":2,"method":"account/rateLimits/read"})).map_err(|_|"Codex se desconectó.")?;
    input.flush().map_err(|_|"Codex se desconectó.")?;
    codex_snapshot(&receive(2)?,now())
}

#[derive(Deserialize,Serialize)]
#[serde(rename_all="camelCase")]
struct Bridge { settings:PathBuf, previous:Option<Value>, installed:Value }
fn settings_path()->Result<PathBuf,String> {
    if let Some(custom)=std::env::var_os("CLAUDE_CONFIG_DIR"){let path=PathBuf::from(custom);if !path.is_absolute(){return Err("CLAUDE_CONFIG_DIR debe ser una ruta absoluta.".into());}return Ok(path.join("settings.json"));}
    Ok(PathBuf::from(std::env::var_os("USERPROFILE").or_else(||std::env::var_os("HOME")).ok_or("Carpeta de usuario no disponible.")?).join(".claude/settings.json"))
}
fn bridge(directory:&Path)->Result<Bridge,String> { serde_json::from_value(read_json(&directory.join("bridge.json"))?).map_err(|_|"Conexión de Claude no válida.".into()) }
fn connection(directory:&Path)->Connection {
    if let Ok(value)=bridge(directory) {
        return Connection{connected:read_json(&value.settings).is_ok_and(|s|s.get("statusLine")==Some(&value.installed)),settings_path:value.settings.display().to_string()};
    }
    Connection{connected:false,settings_path:settings_path().map(|p|p.display().to_string()).unwrap_or_default()}
}
fn snapshot(app:&tauri::AppHandle)->Result<Snapshot,String> {
    let directory=root(app)?;
    let usage=app.state::<Usage>();
    let codex=usage.cache.lock().map_err(|_|"Cuotas ocupadas.")?.as_ref().map(|(_,p)|p.clone()).unwrap_or_else(||provider("codex","OpenAI · Codex","Actualizá para consultar la cuenta conectada a Codex."));
    let connected=connection(&directory);
    let claude=if !connected.connected {provider("claude","Anthropic · Claude","Conectá Claude Code para recibir las cuotas de tu suscripción.")}
        else {read_json(&directory.join("claude.json")).and_then(|v|claude_snapshot(&v,now())).unwrap_or_else(|_|{let mut p=provider("claude","Anthropic · Claude","Esperando datos. Abrí Claude Code CLI en Windows y enviá un mensaje. Si usás otra configuración o WSL, revisá la ayuda de conexión.");p.status="waiting".into();p})};
    Ok(Snapshot{providers:vec![codex,claude],claude_connection:connected})
}
#[tauri::command]
pub async fn ai_usage_snapshot(app:tauri::AppHandle,window:tauri::WebviewWindow,refresh:bool)->Result<Snapshot,String> {
    authorize(&window)?;
    tauri::async_runtime::spawn_blocking(move||{
        let usage=app.state::<Usage>();
        if refresh {
            if let Ok(_refresh)=usage.refresh.try_lock() {
                let due=usage.cache.lock().map_err(|_|"Cuotas ocupadas.")?.as_ref().is_none_or(|(time,_)|time.elapsed()>=Duration::from_secs(30));
                if due {
                    let next=match query_codex(&root(&app)?.join("codex-query")) {
                        Ok(value)=>value,
                        Err(error)=>{let mut value=usage.cache.lock().map_err(|_|"Cuotas ocupadas.")?.as_ref().map(|(_,p)|p.clone()).unwrap_or_else(||provider("codex","OpenAI · Codex",""));value.status="error".into();value.message=error;value},
                    };
                    *usage.cache.lock().map_err(|_|"Cuotas ocupadas.")?=Some((Instant::now(),next));
                }
            }
        }
        snapshot(&app)
    }).await.map_err(|_|"No se pudo consultar el uso de IA.".to_string())?
}

fn set_connection(directory:&Path,settings:&Path,collector:&Path,connect:bool)->Result<(),String> {
    if !cfg!(windows){return Err("El conector automático de esta versión requiere Windows.".into());}
    let manifest=directory.join("bridge.json");
    if !connect {
        let old=bridge(directory)?;
        let mut current=read_json(&old.settings)?;
        if current.get("statusLine")!=Some(&old.installed){return Err("La barra de Claude cambió fuera de Whispera. Se conserva sin modificar.".into());}
        let object=current.as_object_mut().ok_or("Configuración de Claude no válida.")?;
        match old.previous{Some(value)=>{object.insert("statusLine".into(),value);},None=>{object.remove("statusLine");}}
        save_json(&old.settings,&current)?;
        let _=fs::remove_file(manifest);let _=fs::remove_file(directory.join("claude.json"));return Ok(());
    }
    if connection(directory).connected {return Ok(());}
    // Keep an existing file symlink intact (for example a synced Claude config).
    let canonical=if settings.exists(){fs::canonicalize(settings).map_err(|_|"No se pudo resolver la configuración de Claude.")?}else{settings.to_owned()};
    let settings=canonical.as_path();
    let bytes=if settings.exists(){Some(fs::read(settings).map_err(|_|"No se pudo leer la configuración de Claude.")?)}else{None};
    if bytes.as_ref().is_some_and(|v|v.len() as u64>MAX_FILE){return Err("Configuración de Claude demasiado grande.".into());}
    let mut current=if bytes.is_some(){read_json(settings)?}else{json!({})};
    let object=current.as_object_mut().ok_or("Configuración de Claude no válida.")?;
    let previous=object.get("statusLine").cloned();
    fs::create_dir_all(directory).map_err(|_|"No se pudo preparar el conector.")?;
    let target=directory.join("WhisperaClaudeUsage.exe");
    fs::copy(collector,&target).map_err(|_|"No se pudo copiar el conector de Claude.")?;
    let literal=|p:&Path|format!("'{}'",p.to_string_lossy().replace('\'',"''"));
    let script=format!("$OutputEncoding=[Text.UTF8Encoding]::new($false);[Console]::OutputEncoding=$OutputEncoding;$input | & {} {}",literal(&target),literal(&directory.join("claude.json")));
    use base64::Engine;
    let encoded=base64::engine::general_purpose::STANDARD.encode(script.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>());
    let installed=json!({"type":"command","command":format!("powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand {encoded}")});
    object.insert("statusLine".into(),installed.clone());
    if let Some(bytes)=&bytes {
        let backup=settings.with_file_name(format!("settings.whispera-backup-{}.json",uuid::Uuid::new_v4()));
        fs::write(backup,bytes).map_err(|_|"No se pudo respaldar la configuración de Claude.")?;
    }
    save_json(&manifest,&serde_json::to_value(Bridge{settings:settings.to_owned(),previous,installed}).map_err(|_|"Conexión no válida.")?)?;
    // Refuse to overwrite edits made while preparing the bridge.
    if fs::read(settings).ok()!=bytes {return Err("La configuración de Claude cambió durante la conexión. Volvé a intentar.".into());}
    save_json(settings,&current)
}
#[tauri::command]
pub async fn ai_usage_connect_claude(app:tauri::AppHandle,window:tauri::WebviewWindow,connect:bool)->Result<Snapshot,String> {
    authorize(&window)?;
    tauri::async_runtime::spawn_blocking(move||{
        let usage=app.state::<Usage>();let _guard=usage.connection.lock().map_err(|_|"Conexión ocupada.")?;
        let collector=if cfg!(debug_assertions){PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/reader/WhisperaClaudeUsage.exe")}else{app.path().resource_dir().map_err(|_|"Recursos no disponibles.")?.join("bin/WhisperaClaudeUsage.exe")};
        set_connection(&root(&app)?,&settings_path()?,&collector,connect)?;
        snapshot(&app)
    }).await.map_err(|_|"No se pudo cambiar la conexión de Claude.".to_string())?
}
#[tauri::command]
pub fn ai_usage_open_provider(window:tauri::WebviewWindow,provider:String)->Result<(),String> {
    authorize(&window)?;
    crate::platform::open_url(match provider.as_str(){"codex"=>"https://chatgpt.com/codex/settings/usage","claude"=>"https://claude.ai/settings/usage",_=>return Err("Proveedor no válido.".into())})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn quotas_keep_all_windows_and_missing_is_not_zero(){
        let p=codex_snapshot(&json!({"rateLimitsByLimitId":{"codex":{"primary":{"usedPercent":25,"windowDurationMins":300,"resetsAt":2000000000},"secondary":{"usedPercent":null}},"other":{"primary":{"usedPercent":80,"windowDurationMins":10080}}}}),100).unwrap();
        assert_eq!(p.windows.len(),2);assert_eq!(p.windows[0].used_percent,25.0);assert_eq!(p.windows[1].reset_at,None);
        assert!(codex_snapshot(&json!({"rateLimits":{"primary":{"usedPercent":null}}}),100).is_err());
        assert!(codex_snapshot(&json!({"rateLimits":{"primary":{"usedPercent":-1}}}),100).is_err());
    }
    #[test] fn claude_requires_an_observation_and_keeps_expired_values_explicit(){
        assert!(claude_snapshot(&json!({"schema":1}),100).is_err());
        let p=claude_snapshot(&json!({"schema":1,"observedAt":100,"rate_limits":{"five_hour":{"used_percentage":85,"resets_at":150},"seven_day":{"used_percentage":null}}}),200).unwrap();
        assert_eq!(p.windows.len(),1);assert_eq!(p.windows[0].reset_at,Some(150));assert_eq!(p.observed_at,Some(100));
        assert_eq!(claude_snapshot(&json!({"schema":1,"observedAt":100}),200).unwrap().status,"waiting");
    }
    #[test] fn claude_connection_restores_only_its_field_and_preserves_later_edits(){
        if !cfg!(windows){return;}
        let directory=std::env::temp_dir().join(format!("whispera-quota-test-{}-á '$",uuid::Uuid::new_v4()));fs::create_dir_all(&directory).unwrap();
        let settings=directory.join("settings.json");let collector=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/reader/WhisperaClaudeUsage.exe");
        let previous=json!({"type":"command","command":"echo original"});save_json(&settings,&json!({"statusLine":previous,"theme":"dark"})).unwrap();
        let connection_dir=directory.join("connection");set_connection(&connection_dir,&settings,&collector,true).unwrap();
        assert!(connection(&connection_dir).connected);
        let configured=read_json(&settings).unwrap();
        let encoded=configured["statusLine"]["command"].as_str().unwrap().split_whitespace().last().unwrap();
        let shell=PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut command=Command::new(shell);command.args(["-NoProfile","-NonInteractive","-EncodedCommand",encoded]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null());hidden(&mut command);
        let mut child=Process(command.spawn().unwrap());
        child.0.stdin.take().unwrap().write_all(br#"{"rate_limits":{"five_hour":{"used_percentage":22,"resets_at":2000000000}},"transcript_path":"must-not-be-retained"}"#).unwrap();
        let deadline=Instant::now()+Duration::from_secs(15);
        loop {if let Some(status)=child.0.try_wait().unwrap(){assert!(status.success());break;}assert!(Instant::now()<deadline,"fixture connector timed out");std::thread::sleep(Duration::from_millis(20));}
        let captured=read_json(&connection_dir.join("claude.json")).unwrap();assert_eq!(captured["rate_limits"]["five_hour"]["used_percentage"],22);assert!(captured.get("transcript_path").is_none());
        let mut current=read_json(&settings).unwrap();current["theme"]=json!("light");save_json(&settings,&current).unwrap();
        set_connection(&connection_dir,&settings,&collector,false).unwrap();let restored=read_json(&settings).unwrap();assert_eq!(restored["statusLine"],previous);assert_eq!(restored["theme"],"light");
        set_connection(&connection_dir,&settings,&collector,true).unwrap();let mut current=read_json(&settings).unwrap();current["statusLine"]=json!({"type":"command","command":"echo edited elsewhere"});save_json(&settings,&current).unwrap();
        assert!(set_connection(&connection_dir,&settings,&collector,false).is_err());assert_eq!(read_json(&settings).unwrap(),current);
        fs::remove_dir_all(directory).unwrap();
    }
}
