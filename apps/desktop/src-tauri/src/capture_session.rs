//! Independent still-image state while the primary screen session records video.
use std::{ops::Deref, sync::Arc};
use tauri::{Emitter, Manager, State};
use crate::{screen::Screen, screen_editor::Editor};

pub struct ImageCapture {
    pub screen: Arc<Screen>,
    pub editor: Arc<Editor>,
}
impl Default for ImageCapture {
    fn default() -> Self {
        let mut screen=Screen::new();
        screen.secondary=true;
        Self { screen:Arc::new(screen),editor:Arc::new(Editor::default()) }
    }
}
pub enum Session<'a,T:Send+Sync+'static> { Primary(State<'a,T>), Image(Arc<T>) }
impl<T:Send+Sync+'static> Deref for Session<'_,T> {
    type Target=T;
    fn deref(&self)->&T { match self { Self::Primary(state)=>state,Self::Image(state)=>state } }
}
pub fn secondary(scope:&str)->bool { scope.starts_with("image-") }
pub fn selector(label:&str)->bool { label.starts_with("screen-select-")||label.starts_with("image-select-") }
pub fn frozen_source(label:&str)->Option<String> {
    let (prefix,index)=label.split_once("-freeze-")?;
    if !["screen","image"].contains(&prefix)||index.is_empty()||!index.bytes().all(|b|b.is_ascii_digit()){return None;}
    Some(format!("{prefix}-select-{index}"))
}
pub fn label(scope:&str,base:&str)->String {
    if secondary(scope) {base.replacen("screen-","image-",1)} else {base.into()}
}
// Emitter::emit broadcasts even when called on a window. Capture lifecycle
// events belong only to the named webview, including preloaded hidden windows.
pub fn emit_window<R:tauri::Runtime,S:serde::Serialize+Clone>(window:&tauri::WebviewWindow<R>,event:&str,payload:S)->tauri::Result<()> {
    window.emit_to(window.label(),event,payload)
}
pub fn screen<'a>(app:&'a tauri::AppHandle,scope:&str)->Session<'a,Screen> {
    if secondary(scope) { Session::Image(app.state::<ImageCapture>().screen.clone()) }
    else { Session::Primary(app.state::<Screen>()) }
}
pub fn editor<'a>(app:&'a tauri::AppHandle,scope:&str)->Session<'a,Editor> {
    if secondary(scope) { Session::Image(app.state::<ImageCapture>().editor.clone()) }
    else { Session::Primary(app.state::<Editor>()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_monitor_labels_keep_their_session_and_monitor() {
        assert_eq!(frozen_source("screen-freeze-0"),Some("screen-select-0".into()));
        assert_eq!(frozen_source("image-freeze-12"),Some("image-select-12".into()));
        for label in ["screen-select-0","other-freeze-0","image-freeze-","screen-freeze-../0"]{assert_eq!(frozen_source(label),None);}
    }
    #[test]
    fn resize_and_hide_events_never_reach_another_capture_window() {
        use std::sync::{Arc,Mutex};
        use tauri::Listener;
        let app=tauri::test::mock_app();
        let labels=["screen-hud","image-hud","screen-select-0","screen-select-1","image-select-0"];
        let calls=Arc::new(Mutex::new(Vec::new()));
        let windows:Vec<_>=labels.iter().map(|label| {
            let window=tauri::WebviewWindowBuilder::new(&app,*label,Default::default()).build().unwrap();
            for event in ["screen-editor-reset","screen-hide","screen-reset","screen-stage"] {
                let calls=calls.clone();let label=label.to_string();
                window.listen(event,move|_|calls.lock().unwrap().push(label.clone()));
            }
            window
        }).collect();
        for window in &windows {
            for event in ["screen-editor-reset","screen-hide","screen-reset","screen-stage"] {
                calls.lock().unwrap().clear();
                emit_window(window,event,serde_json::json!({"id":"image-session"})).unwrap();
                assert_eq!(*calls.lock().unwrap(),vec![window.label().to_string()],"{event} leaked to another window");
            }
        }
    }
    #[test]
    fn still_and_video_state_do_not_share_status_or_geometry() {
        let video=Screen::new();let image=ImageCapture::default();
        video.state.lock().unwrap().phase="recording".into();
        image.screen.state.lock().unwrap().phase="editing".into();
        image.screen.state.lock().unwrap().phase="idle".into();
        assert_eq!(video.state.lock().unwrap().phase,"recording");
        assert!(!video.secondary);assert!(image.screen.secondary);
        assert_eq!(label("image-session","screen-ink"),"image-ink");
        assert_eq!(label("video-session","screen-ink"),"screen-ink");
    }
}
