#[derive(Clone, Copy)]
pub struct Tracked {
    pub focus: isize,
    pub token: u64,
}

unsafe extern "C" {
    fn whispera_paste_tracker_start();
    fn whispera_paste_capture_for_pid(pid: u32, token: *mut u64) -> i32;
    fn whispera_paste_focus(token: u64, restore: i32) -> i32;
}

pub fn start() {
    unsafe { whispera_paste_tracker_start() }
}

// On macOS the opaque focus handle is the AX registry token, never a raw pointer.
pub fn capture(_window: isize, pid: u32) -> Option<Tracked> {
    start();
    let mut token = 0;
    (unsafe { whispera_paste_capture_for_pid(pid, &mut token) } == 0).then_some(Tracked {
        focus: token as isize,
        token,
    })
}

pub fn focus(token: u64, restore: bool) -> bool {
    unsafe { whispera_paste_focus(token, i32::from(restore)) == 0 }
}
