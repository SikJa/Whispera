use crate::{paste, storage::Store};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};
use tauri::{Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
static GATE: Mutex<()> = Mutex::new(());
static INCOMING: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
pub fn enqueue(path: &Path) {
    if let Ok(mut queue) = INCOMING.lock() {
        if !queue.contains(&path.to_path_buf()) {
            queue.push(path.to_path_buf());
        }
        if queue.len() > 64 {
            queue.remove(0);
        }
    }
}
fn image_hash(rgba: &[u8], width: u32, height: u32) -> String {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    rgba.hash(&mut hash);
    width.hash(&mut hash);
    height.hash(&mut hash);
    format!("{:x}", hash.finish())
}

#[derive(Default)]
pub struct Library {
    target: Mutex<Option<paste::Target>>,
    sequence: Mutex<Option<std::num::NonZeroU32>>,
}
pub(crate) fn suppress_clipboard(app: &tauri::AppHandle) {
    if let Ok(mut sequence) = app.state::<Library>().sequence.lock() {
        *sequence = clipboard_win::seq_num();
    }
}
pub(crate) fn rows(store: &Store) -> Result<Vec<Value>, String> {
    store.get("library_items")
}
pub(crate) fn settings(store: &Store) -> Result<Value, String> {
    store.get("library_settings")
}
#[tauri::command]
pub fn library_preferences(app: tauri::AppHandle) -> Result<Value, String> {
    let mut config = settings(&app.state::<Store>())?;
    if !config.is_object() { config = json!({}); }
    config["toggleHotkey"] = json!(hotkey(&app));
    Ok(config)
}
pub fn hotkey(app: &tauri::AppHandle) -> String {
    settings(&app.state::<Store>())
        .ok()
        .and_then(|v| v["toggleHotkey"].as_str().map(String::from))
        .unwrap_or_else(|| "Alt+C".into())
}
fn update_settings(app: &tauri::AppHandle, patch: &Value) -> Result<Value, String> {
    validate_preferences(patch)?;
    let store = app.state::<Store>();
    let mut config = settings(&store)?;
    if !config.is_object() {
        config = json!({});
    }
    for (k, v) in patch.as_object().ok_or("Configuracion invalida")? {
        config[k] = v.clone();
    }
    if let Some(enabled) = patch["launchAtLogin"].as_bool() {
        crate::startup::set_enabled(enabled)?;
    }
    if let Some(key) = config["toggleHotkey"].as_str() {
        let parsed = crate::shortcuts::parse(key)?;
        let old = hotkey(app);
        if key != old {
            if app.global_shortcut().is_registered(parsed) {
                return Err("Ese atajo ya esta en uso".into());
            }
            crate::shortcuts::register(app, key)?;
            if let Err(e) = store.put("library_settings", &config) {
                let _ = app.global_shortcut().unregister(parsed);
                return Err(e);
            }
            if let Ok(old) = crate::shortcuts::parse(&old) {
                let _ = app.global_shortcut().unregister(old);
            }
            return Ok(config);
        }
    }
    store.put("library_settings", &config)?;
    Ok(config)
}
fn validate_preferences(patch: &Value) -> Result<(), String> {
    let object=patch.as_object().ok_or("Configuracion invalida")?;
    for name in ["captureGlobal","incognito","clearUnpinnedOnRestart","transcribeVideo","videoTranscriptAttachment"] {
        if object.get(name).is_some_and(|v|!v.is_boolean()) { return Err(format!("Opcion invalida: {name}")); }
    }
    for (name,min,max) in [("historyLimit",10,1000),("autoDeleteHours",0,8760)] {
        if let Some(value)=object.get(name) { if !value.as_u64().is_some_and(|v|v>=min && v<=max) { return Err(format!("{name} debe estar entre {min} y {max}")); } }
    }
    Ok(())
}
fn save(store: &Store, mut items: Vec<Value>) -> Result<Vec<Value>, String> {
    let previous = rows(store)?;
    let config = settings(store)?;
    let limit = config["historyLimit"]
        .as_u64()
        .unwrap_or(250)
        .clamp(10, 1000) as usize;
    let hours = config["autoDeleteHours"]
        .as_i64()
        .unwrap_or(48)
        .clamp(0, 8760);
    let cutoff = chrono::Utc::now().timestamp_millis() - hours * 3_600_000;
    let mut count = 0;
    items.retain(|i| {
        if i["pinned"].as_bool().unwrap_or(false) {
            return true;
        }
        if hours != 0 && i["capturedAt"].as_i64().unwrap_or(0) < cutoff {
            return false;
        }
        count += 1;
        count <= limit
    });
    if previous != items {
        store.put("library_items", &items)?;
    }
    Ok(items)
}
fn add(store: &Store, data: Value) -> Result<(), String> {
    let mut items = rows(store)?;
    if let Some(index) = items.iter().position(|i| {
        i["data"] == data
            || (data["fingerprint"].is_string()
                && i["data"]["fingerprint"] == data["fingerprint"])
    }) {
        // A repeated copy is a new use, not a new identity. Refresh retention and ordering.
        let mut item = items.remove(index);
        item["data"] = data;
        item["capturedAt"] = json!(chrono::Utc::now().timestamp_millis());
        item["hitCount"] = json!(item["hitCount"].as_u64().unwrap_or(0).saturating_add(1));
        items.insert(0, item);
        save(store, items)?;
        return Ok(());
    }
    items.insert(0, json!({"id":uuid::Uuid::new_v4().to_string(),"data":data,"capturedAt":chrono::Utc::now().timestamp_millis(),"hitCount":1,"pinned":false}));
    save(store, items)?;
    Ok(())
}
fn files_data(paths: &[String]) -> Value {
    let entries: Vec<Value> = paths.iter().map(|p| { let path=Path::new(p); json!({"name":path.file_name().unwrap_or_default().to_string_lossy(),"ext":path.extension().unwrap_or_default().to_string_lossy(),"size":fs::metadata(path).map(|m|m.len()).unwrap_or(0),"isImage":false}) }).collect();
    json!({"kind":"files","paths":paths,"entries":entries})
}
pub fn remember_file(store: &Store, path: &Path) -> Result<(), String> {
    let _guard = GATE.lock().map_err(|_| "Biblioteca ocupada")?;
    remember_file_unlocked(store, path)
}
fn remember_file_unlocked(store: &Store, path: &Path) -> Result<(), String> {
    if settings(store)?["incognito"].as_bool().unwrap_or(false) {
        return Ok(());
    }
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(image) = image::load_from_memory(&bytes) {
                let fingerprint =
                    image_hash(image.to_rgba8().as_raw(), image.width(), image.height());
                return add(
                    store,
                    json!({"kind":"image","imageId":path.to_string_lossy(),"fingerprint":fingerprint,"width":image.width(),"height":image.height(),"bytes":bytes.len(),"ext":"png","source":"screenshot"}),
                );
            }
        }
    }
    add(store, files_data(&[path.to_string_lossy().into_owned()]))
}
fn payload(store: &Store) -> Result<Value, String> {
    let items = save(store, rows(store)?)?;
    Ok(
        json!({"items":items,"settings":settings(store)?,"version":env!("CARGO_PKG_VERSION"),"isStoreBuild":false}),
    )
}
fn image_data(app: &tauri::AppHandle, image: tauri::image::Image<'_>) -> Result<Value, String> {
    if image.width() as u64 * image.height() as u64 > 40_000_000 {
        return Err("Imagen demasiado grande".into());
    }
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("library-images");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let rgba = image.rgba();
    let fingerprint = image_hash(rgba, image.width(), image.height());
    let path = dir.join(format!("{fingerprint}.png"));
    let bitmap = image::RgbaImage::from_raw(image.width(), image.height(), rgba.to_vec())
        .ok_or("Imagen no valida")?;
    if !path.exists() {
        bitmap.save(&path).map_err(|e| e.to_string())?;
    }
    Ok(
        json!({"kind":"image","imageId":path.to_string_lossy(),"fingerprint":fingerprint,"width":image.width(),"height":image.height(),"bytes":fs::metadata(&path).map(|m|m.len()).unwrap_or(0),"ext":"png","source":"image"}),
    )
}
fn collect(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<Library>();
    let _guard = GATE.lock().map_err(|_| "Biblioteca ocupada")?;
    let store = app.state::<Store>();
    let sequence = clipboard_win::seq_num();
    if sequence == *state.sequence.lock().map_err(|_| "Biblioteca ocupada")? {
        return Ok(());
    }
    if clipboard_win::register_format("ExcludeClipboardContentFromMonitorProcessing")
        .is_some_and(|format| clipboard_win::is_format_avail(format.get()))
    {
        *state.sequence.lock().map_err(|_| "Biblioteca ocupada")? = sequence;
        return Ok(());
    }
    let paths: Result<Vec<String>, _> =
        clipboard_win::get_clipboard(clipboard_win::formats::FileList);
    if let Ok(paths) = paths {
        let paths: Vec<_> = paths
            .into_iter()
            .filter(|p| Path::new(p).is_file())
            .take(10)
            .collect();
        if !paths.is_empty() {
            add(&store, files_data(&paths))?;
        }
    } else if let Ok(image) = app.clipboard().read_image() {
        add(&store, image_data(app, image)?)?;
    } else if let Ok(text) = app.clipboard().read_text() {
        let secret = text.trim().starts_with("gsk_") || text.trim().starts_with("sk-proj-");
        if !secret && !text.is_empty() && text.len() <= 262144 {
            let is_url = text.starts_with("https://") || text.starts_with("http://");
            add(&store, json!({"kind":"text","text":text,"isUrl":is_url}))?;
        }
    }
    *state.sequence.lock().map_err(|_| "Biblioteca ocupada")? = sequence;
    Ok(())
}
pub fn start(app: &tauri::AppHandle) {
    let store = app.state::<Store>();
    if settings(&store).unwrap_or(Value::Null).is_null() {
        let _ = store.put("library_settings", &json!({"captureGlobal":true,"autoDeleteHours":48,"historyLimit":250,"incognito":false,"toggleHotkey":"Alt+C"}));
    }
    if settings(&store)
        .ok()
        .is_some_and(|c| c["clearUnpinnedOnRestart"] == true)
    {
        if let Ok(_guard) = GATE.lock() {
            if let Ok(mut items) = rows(&store) {
                items.retain(|i| i["pinned"] == true);
                let _ = save(&store, items);
            }
        }
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let mut ticks = 0u64;
        loop {
            std::thread::sleep(Duration::from_millis(750));
            ticks += 1;
            if ticks % 4800 == 1 {
                if let Ok(_guard) = GATE.lock() {
                    let store = app.state::<Store>();
                    if let Ok(items) = rows(&store).and_then(|items| save(&store, items)) {
                        if let Ok(root) = app.path().app_data_dir() {
                            let directory = root.join("library-images");
                            let used: Vec<_> =
                                items.iter().flat_map(|i| paths(&i["data"])).collect();
                            if let Ok(entries) = fs::read_dir(directory) {
                                for entry in entries.flatten() {
                                    let path = entry.path();
                                    if path.extension().is_some_and(|e| e == "png")
                                        && !used.contains(&path)
                                        && fs::symlink_metadata(&path).is_ok_and(|m| {
                                            use std::os::windows::fs::MetadataExt;
                                            m.is_file() && m.file_attributes() & 0x400 == 0
                                        })
                                    {
                                        let _ = fs::remove_file(path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let config = settings(&app.state::<Store>()).unwrap_or(Value::Null);
            let pending = INCOMING
                .lock()
                .map(|mut q| std::mem::take(&mut *q))
                .unwrap_or_default();
            if !pending.is_empty() && !config["incognito"].as_bool().unwrap_or(false) {
                if let Ok(_guard) = GATE.lock() {
                    let store = app.state::<Store>();
                    for path in pending {
                        if path.is_file() {
                            let _ = remember_file_unlocked(&store, &path);
                        }
                    }
                }
            }
            if config["captureGlobal"].as_bool().unwrap_or(false)
                && !config["incognito"].as_bool().unwrap_or(false)
            {
                let _ = collect(&app);
            }
        }
    });
}
#[tauri::command]
pub async fn library_state(
    app: tauri::AppHandle,
    revision: Option<String>,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = GATE.lock().map_err(|_| "Biblioteca ocupada")?;
        let store = app.state::<Store>();
        let items = save(&store, rows(&store)?)?;
        use std::hash::{Hash, Hasher};
        let startup=crate::startup::cached_enabled()?;
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_string(&(items, settings(&store)?, startup))
            .map_err(|e| e.to_string())?
            .hash(&mut hash);
        let current = format!("{:x}", hash.finish());
        if revision.as_deref() == Some(&current) {
            return Ok(json!({"unchanged":true}));
        }
        let mut result = payload(&store)?;
        for item in result["items"].as_array().into_iter().flatten() {
            for path in paths(&item["data"]) {
                if path.is_file() {
                    let _ = app.asset_protocol_scope().allow_file(path);
                }
            }
        }
        result["revision"] = json!(current);
        if !result["settings"].is_object() {
            result["settings"] = json!({});
        }
        result["settings"]["launchAtLogin"] =
            json!(startup);
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn library_collect(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || collect(&app))
        .await
        .map_err(|e| e.to_string())?
}
pub(crate) fn paths(data: &Value) -> Vec<PathBuf> {
    if data["kind"] == "image" {
        data["imageId"]
            .as_str()
            .map(PathBuf::from)
            .into_iter()
            .collect()
    } else if data["kind"] == "image-collection" {
        data["images"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|i| i["imageId"].as_str().map(PathBuf::from))
            .collect()
    } else {
        data["paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|i| i.as_str().map(PathBuf::from))
            .collect()
    }
}
fn selected(data: &Value, request: &Value) -> Result<Value, String> {
    if let Some(image_id) = request["imageId"].as_str() {
        let mut image = data["images"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|i| i["imageId"].as_str() == Some(image_id))
            .cloned()
            .ok_or("Imagen inexistente")?;
        image["kind"] = json!("image");
        return Ok(image);
    }
    if let Some(requested) = request["paths"].as_array() {
        let allowed = paths(data);
        let selected: Vec<String> = requested
            .iter()
            .filter_map(|p| p.as_str())
            .filter(|p| allowed.contains(&PathBuf::from(p)))
            .map(String::from)
            .collect();
        if selected.is_empty() {
            return Err("Archivo inexistente".into());
        }
        return Ok(files_data(&selected));
    }
    Ok(data.clone())
}
fn detach(data: &mut Value, request: &Value) -> Result<Value, String> {
    let extracted = selected(data, request)?;
    if data["kind"] == "image-collection" {
        let id = extracted["imageId"]
            .as_str()
            .ok_or("Selecciona una imagen")?;
        let images = data["images"].as_array_mut().ok_or("Grupo invalido")?;
        images.retain(|image| image["imageId"].as_str() != Some(id));
        if images.len() == 1 {
            let mut remaining = images[0].clone();
            remaining["kind"] = json!("image");
            *data = remaining;
        }
    } else if data["kind"] == "files" {
        let selected = paths(&extracted);
        let remaining: Vec<_> = paths(data)
            .into_iter()
            .filter(|p| !selected.contains(p))
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        *data = files_data(&remaining);
    } else {
        return Err("No es un grupo".into());
    }
    Ok(extracted)
}
#[tauri::command]
pub async fn library_action(
    app: tauri::AppHandle,
    action: String,
    id: Option<String>,
    value: Value,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move||{
    let state=app.state::<Library>();let _guard=GATE.lock().map_err(|_|"Biblioteca ocupada")?;let store=app.state::<Store>();let mut items=rows(&store)?;
    if action=="settings" { return update_settings(&app,&value); }
    let index=id.as_ref().and_then(|id|items.iter().position(|i|i["id"].as_str()==Some(id)));
    match action.as_str(){
      "pin"=>{let i=index.ok_or("Elemento inexistente")?;items[i]["pinned"]=json!(value.as_bool().unwrap_or(false));},
      "delete"=>{if let Some(i)=index{items.remove(i);}},
      "clear"=>items.retain(|i|i["pinned"].as_bool().unwrap_or(false)),
      "merge"=>{let source=index.ok_or("Elemento inexistente")?;let target=items.iter().position(|i|i["id"]==value).ok_or("Destino inexistente")?;if source==target{return Err("Selecciona otro elemento".into());}let a=items[source]["data"].clone();let b=items[target]["data"].clone();if a["kind"]=="files"&&b["kind"]=="files"{let mut ps=b["paths"].as_array().cloned().unwrap_or_default();ps.extend(a["paths"].as_array().cloned().unwrap_or_default());if ps.len()>10{return Err("Maximo 10 archivos".into());}let ps:Vec<_>=ps.iter().filter_map(|p|p.as_str().map(String::from)).collect();items[target]["data"]=files_data(&ps);}else{let imgs=|d:&Value|->Vec<Value>{if d["kind"]=="image"{vec![d.clone()]}else if d["kind"]=="image-collection"{d["images"].as_array().cloned().unwrap_or_default()}else{vec![]}};let mut images=imgs(&b);let more=imgs(&a);if images.is_empty()||more.is_empty(){return Err("Tipos incompatibles".into());}images.extend(more);if images.len()>10{return Err("Maximo 10 imagenes".into());}items[target]["data"]=json!({"kind":"image-collection","images":images});}items.remove(source);},
      "split"|"remove-part"=>{let i=index.ok_or("Elemento inexistente")?;let part=detach(&mut items[i]["data"],&value)?;if paths(&items[i]["data"]).is_empty(){items.remove(i);}if action=="split"{items.insert(0,json!({"id":uuid::Uuid::new_v4().to_string(),"data":part,"capturedAt":chrono::Utc::now().timestamp_millis(),"hitCount":1,"pinned":false}));}},
      "reveal"=>{let i=index.ok_or("Elemento inexistente")?;let data=selected(&items[i]["data"],&value)?;let path=paths(&data).into_iter().next().filter(|p|p.is_file()).ok_or("Archivo inexistente")?;std::process::Command::new("explorer.exe").arg(format!("/select,{}",path.display())).spawn().map_err(|e|e.to_string())?;return Ok(json!(true));},
      "copy"|"paste"=>{let i=index.ok_or("Elemento inexistente")?;let data=selected(&items[i]["data"],&value)?;if data["kind"]=="text"{app.clipboard().write_text(data["text"].as_str().unwrap_or("")).map_err(|e|e.to_string())?;}else if data["kind"]=="image"{let path=paths(&data).into_iter().next().ok_or("Imagen inexistente")?;let bytes=fs::read(path).map_err(|e|e.to_string())?;let image=tauri::image::Image::from_bytes(&bytes).map_err(|e|e.to_string())?;app.clipboard().write_image(&image).map_err(|e|e.to_string())?;}else{let p=paths(&data);if p.is_empty(){return Err("Los archivos ya no existen".into());}crate::video_transcript::copy_files(&app,&p)?;}
        *state.sequence.lock().map_err(|_|"Biblioteca ocupada")?=clipboard_win::seq_num();
        if action=="paste"{if let Some(w)=app.get_webview_window("library"){let _=w.hide();}let target=state.target.lock().map_err(|_|"Destino ocupado")?.ok_or("No hay destino. Contenido copiado para pegar manualmente")?;paste::restore_and_paste(target)?;}return Ok(json!(true));},
      "add"=>{let p:Vec<String>=value.as_array().ok_or("Archivos invalidos")?.iter().filter_map(|p|p.as_str().map(String::from)).filter(|p|Path::new(p).is_file()).take(10).collect();if !p.is_empty(){add(&store,files_data(&p))?;}return payload(&store);},
      _=>return Err("Accion no disponible todavia".into())
    }save(&store,items)?;payload(&store)
}).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn library_drag(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    id: String,
    request: Value,
) -> Result<(), String> {
    if window.label() != "library" && !window.label().starts_with("media-") {
        return Err("Arrastre solo desde Biblioteca".into());
    }
    let data = rows(&app.state::<Store>())?
        .into_iter()
        .find(|i| i["id"] == id)
        .ok_or("Elemento inexistente")?;
    let files = paths(&selected(&data["data"], &request)?);
    if files.is_empty() || files.iter().any(|p| !p.is_file()) {
        return Err("El archivo ya no existe".into());
    }
    let preview = crate::library_media::drag_image(&app, &files[0])
        .unwrap_or_else(|_| include_bytes!("../icons/wave/32x32.png").to_vec());
    let (files, _) = crate::video_transcript::package(&app.state::<Store>(), &files)?;
    let files = crate::video_transcript::share_files(&app, &files)?;
    let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
    let w = window.clone();
    let (send, receive) = tokio::sync::oneshot::channel();
    window
        .run_on_main_thread(move || {
            let result = drag::start_drag(
                &w,
                drag::DragItem::Files(files),
                drag::Image::Raw(preview),
                |_, _| {},
                Default::default(),
            );
            let _ = send.send(result.map_err(|e| e.to_string()));
        })
        .map_err(|e| e.to_string())?;
    receive.await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn library_pick(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        let picked = app.dialog().file().blocking_pick_files();
        if let Some(picked) = picked {
            let store = app.state::<Store>();
            let _guard = GATE.lock().map_err(|_| "Biblioteca ocupada")?;
            for path in picked.into_iter().take(10) {
                if let Ok(path) = path.into_path() {
                    remember_file_unlocked(&store, &path)?;
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn main_library_preferences_reject_invalid_limits_and_flags() {
        assert!(validate_preferences(&json!({"captureGlobal":true,"historyLimit":250,"autoDeleteHours":0})).is_ok());
        for value in [json!({"historyLimit":0}),json!({"historyLimit":1001}),json!({"autoDeleteHours":-1}),json!({"transcribeVideo":"yes"})] { assert!(validate_preferences(&value).is_err()); }
    }
    #[test]
    fn identical_capture_keeps_id_and_pin_but_updates_the_real_file() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        add(
            &s,
            json!({"kind":"image","imageId":"clipboard.png","fingerprint":"same"}),
        )
        .unwrap();
        let mut items = rows(&s).unwrap();
        let id = items[0]["id"].clone();
        items[0]["pinned"] = json!(true);
        s.put("library_items", &items).unwrap();
        add(
            &s,
            json!({"kind":"image","imageId":"capture.png","fingerprint":"same"}),
        )
        .unwrap();
        let items = rows(&s).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], id);
        assert_eq!(items[0]["pinned"], true);
        assert_eq!(items[0]["data"]["imageId"], "capture.png");
    }
    #[test]
    fn retention_preserves_pins_and_limits_recent() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        s.put(
            "library_settings",
            &json!({"historyLimit":10,"autoDeleteHours":1}),
        )
        .unwrap();
        let mut items = vec![json!({"id":"pin","pinned":true,"capturedAt":0})];
        for n in 0..25 {
            items.push(json!({"id":n,"capturedAt":chrono::Utc::now().timestamp_millis()}));
        }
        assert_eq!(save(&s, items).unwrap().len(), 11);
    }
    #[test]
    fn repeated_text_is_deduplicated() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let data = json!({"kind":"text","text":"test","isUrl":false});
        add(&s, data.clone()).unwrap();
        add(&s, data).unwrap();
        assert_eq!(rows(&s).unwrap().len(), 1);
        assert_eq!(rows(&s).unwrap()[0]["hitCount"], 2);
    }
    #[test]
    fn repeated_copy_refreshes_expired_item_and_moves_it_first() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let data = json!({"kind":"text","text":"again"});
        s.put("library_items", &vec![
            json!({"id":"other","pinned":true,"capturedAt":1}),
            json!({"id":"reused","data":data,"capturedAt":1,"hitCount":3}),
        ]).unwrap();
        add(&s, data).unwrap();
        let items = rows(&s).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["id"], "reused");
        assert_eq!(items[0]["hitCount"], 4);
        assert!(items[0]["capturedAt"].as_i64().unwrap() > 1);
    }
    #[test]
    fn expired_rows_do_not_consume_history_capacity() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        s.put("library_settings", &json!({"historyLimit":10,"autoDeleteHours":1})).unwrap();
        let mut items: Vec<_> = (0..10).map(|n| json!({"id":n,"capturedAt":1})).collect();
        items.push(json!({"id":"recent","capturedAt":chrono::Utc::now().timestamp_millis()}));
        let saved = save(&s, items).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0]["id"], "recent");
    }
    #[test]
    fn groups_normalize_after_detaching_and_reject_foreign_paths() {
        let mut images =
            json!({"kind":"image-collection","images":[{"imageId":"a.png"},{"imageId":"b.png"}]});
        assert_eq!(
            detach(&mut images, &json!({"imageId":"a.png"})).unwrap()["kind"],
            "image"
        );
        assert_eq!(images["kind"], "image");
        assert_eq!(images["imageId"], "b.png");
        let mut files = files_data(&["a.mp4".into(), "b.pdf".into()]);
        assert!(selected(&files, &json!({"paths":["not-in-history.txt"]})).is_err());
        detach(&mut files, &json!({"paths":["a.mp4"]})).unwrap();
        assert_eq!(paths(&files), vec![PathBuf::from("b.pdf")]);
        assert_eq!(files["entries"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn incognito_does_not_add_capture_items() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        s.put("library_settings", &json!({"incognito":true}))
            .unwrap();
        remember_file(&s, Path::new("fixture.mp4")).unwrap();
        assert!(rows(&s).unwrap().is_empty());
    }
    #[test]
    fn expiration_can_be_disabled_and_expired_unpinned_items_are_removed() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let old = json!({"id":"old","capturedAt":1});
        assert!(save(&s, vec![old.clone()]).unwrap().is_empty());
        s.put("library_settings", &json!({"autoDeleteHours":0}))
            .unwrap();
        assert_eq!(save(&s, vec![old]).unwrap().len(), 1);
    }
}
#[tauri::command]
pub async fn library_toggle(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || toggle(app))
        .await
        .map_err(|e| e.to_string())?
}
fn toggle(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("library") {
        if w.is_visible().map_err(|e| e.to_string())? {
            return w.hide().map_err(|e| e.to_string());
        }
        *app.state::<Library>()
            .target
            .lock()
            .map_err(|_| "Destino ocupado")? = paste::capture();
        let _ = w.emit("library-open", ());
        return w.show().map_err(|e| e.to_string());
    }
    *app.state::<Library>()
        .target
        .lock()
        .map_err(|_| "Destino ocupado")? = paste::capture();
    let w = tauri::WebviewWindowBuilder::new(
        &app,
        "library",
        tauri::WebviewUrl::App("library/browser-preview.html".into()),
    )
    .title("Whispera - Biblioteca")
    .inner_size(740., 720.)
    .decorations(false)
    .shadow(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(false)
    .visible(false)
    .build()
    .map_err(|e| e.to_string())?;
    crate::floating_window::apply(&w)?;
    if let Some(m) = w.current_monitor().map_err(|e| e.to_string())? {
        let work = m.work_area();
        let scale = m.scale_factor();
        let _ = w.set_size(tauri::LogicalSize::new(
            work.size.width as f64 / scale,
            work.size.height as f64 / scale,
        ));
        let _ = w.set_position(tauri::PhysicalPosition::new(
            work.position.x,
            work.position.y,
        ));
    }
    Ok(())
}
#[tauri::command]
pub fn library_window(window: tauri::WebviewWindow, operation: String) -> Result<(), String> {
    if window.label() != "library" {
        return Err("Ventana invalida".into());
    }
    match operation.as_str() {
        "show" => window.show(),
        "hide" => window.hide(),
        "focus" => window.set_focus(),
        _ => return Err("Accion invalida".into()),
    }
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn library_cursor(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<Value, String> {
    if window.label() != "library" {
        return Err("Ventana invalida".into());
    }
    let origin = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let mut cursor = windows::Win32::Foundation::POINT::default();
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor)
            .map_err(|e| e.to_string())?;
    }
    Ok(
        json!({"x":(cursor.x-origin.x) as f64/scale,"y":(cursor.y-origin.y) as f64/scale,"displayWidth":size.width as f64/scale,"displayHeight":size.height as f64/scale,"stickPosition":settings(&app.state::<Store>())?["stickPosition"]}),
    )
}
