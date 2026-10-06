//! Independent still-image state while the primary screen session records video.
use std::{ops::Deref, sync::Arc};
use tauri::{Manager, State};
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
pub fn label(scope:&str,base:&str)->String {
    if secondary(scope) {base.replacen("screen-","image-",1)} else {base.into()}
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
