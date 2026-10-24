//! Wake the existing collector on clipboard changes, including Cut. No key hook.
#[cfg(windows)]
use std::sync::{mpsc::{sync_channel,Receiver,SyncSender},OnceLock};
#[cfg(windows)]
static WAKE:OnceLock<SyncSender<()>>=OnceLock::new();
#[cfg(windows)]
const SUBCLASS:usize=0x57434c50;
#[cfg(windows)]
unsafe extern "system" fn clipboard_changed(hwnd:windows::Win32::Foundation::HWND,message:u32,wp:windows::Win32::Foundation::WPARAM,lp:windows::Win32::Foundation::LPARAM,id:usize,_data:usize)->windows::Win32::Foundation::LRESULT {
    use windows::Win32::{System::DataExchange::RemoveClipboardFormatListener,UI::{Shell::{DefSubclassProc,RemoveWindowSubclass},WindowsAndMessaging::{WM_CLIPBOARDUPDATE,WM_NCDESTROY}}};
    if message==WM_CLIPBOARDUPDATE {if let Some(sender)=WAKE.get(){let _=sender.try_send(());}}
    if message==WM_NCDESTROY {let _=RemoveClipboardFormatListener(hwnd);let _=RemoveWindowSubclass(hwnd,Some(clipboard_changed),id);}
    DefSubclassProc(hwnd,message,wp,lp)
}
#[cfg(windows)]
pub fn subscribe(window:&tauri::WebviewWindow)->Receiver<()> {
    let (send,receive)=sync_channel(1);
    if WAKE.set(send).is_ok() {
        let clone=window.clone();
        let _=window.run_on_main_thread(move||unsafe{
            use windows::Win32::{UI::Shell::SetWindowSubclass,System::DataExchange::AddClipboardFormatListener};
            if let Ok(hwnd)=clone.hwnd(){let hwnd=windows::Win32::Foundation::HWND(hwnd.0);if SetWindowSubclass(hwnd,Some(clipboard_changed),SUBCLASS,0).as_bool(){let _=AddClipboardFormatListener(hwnd);}}
        });
    }
    receive
}
