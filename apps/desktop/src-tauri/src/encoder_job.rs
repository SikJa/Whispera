//! Child encoders cannot keep capturing after Whispera exits or crashes.
use std::process::{Child,Command};
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use windows::Win32::{Foundation::{CloseHandle,HANDLE},System::JobObjects::{CreateJobObjectW,AssignProcessToJobObject,SetInformationJobObject,JobObjectExtendedLimitInformation,JOBOBJECT_EXTENDED_LIMIT_INFORMATION,JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE}};
#[cfg(windows)]
pub struct Encoder {pub child:Child,job:HANDLE}
#[cfg(windows)]
impl Encoder{
    pub fn spawn(command:&mut Command)->Result<Self,String>{
        // Create the job before the encoder and close it on every failed path.
        let job=unsafe{CreateJobObjectW(None,None)}.map_err(|e|e.to_string())?;
        let mut info=JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();info.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if let Err(error)=unsafe{SetInformationJobObject(job,JobObjectExtendedLimitInformation,&info as *const _ as *const _,std::mem::size_of_val(&info) as u32)}{
            unsafe{let _=CloseHandle(job);}return Err(error.to_string());
        }
        let mut child=match command.spawn(){Ok(child)=>child,Err(error)=>{unsafe{let _=CloseHandle(job);}return Err(error.to_string());}};
        if let Err(error)=unsafe{AssignProcessToJobObject(job,HANDLE(child.as_raw_handle()))}{let _=child.kill();let _=child.wait();unsafe{let _=CloseHandle(job);}return Err(error.to_string());}
        Ok(Self{child,job})
    }
}
#[cfg(windows)]
impl Drop for Encoder{fn drop(&mut self){let _=self.child.kill();let _=self.child.wait();unsafe{let _=CloseHandle(self.job);}}}

#[cfg(target_os = "macos")]
pub struct Encoder {
    pub child: Child,
    guardian: Child,
}
#[cfg(target_os = "macos")]
impl Encoder {
    pub fn spawn(command: &mut Command) -> Result<Self, String> {
        use std::{os::unix::process::CommandExt, process::Stdio};
        // macOS has no PR_SET_PDEATHSIG. Only this process owns the guardian's
        // stdin writer: a crash closes it and kills the encoder's process group.
        let mut child = command.process_group(0).spawn().map_err(|e| e.to_string())?;
        let guardian = Command::new("/bin/sh")
            .args(["-c", "IFS= read -r message; if [ \"$message\" != done ]; then /bin/kill -KILL -- -\"$1\" 2>/dev/null; fi", "whispera-encoder"])
            .arg(child.id().to_string())
            .process_group(0)
            .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null())
            .spawn();
        match guardian {
            Ok(guardian) => Ok(Self { child, guardian }),
            Err(error) => { let _ = child.kill(); let _ = child.wait(); Err(error.to_string()) }
        }
    }
}
#[cfg(target_os = "macos")]
impl Drop for Encoder {
    fn drop(&mut self) {
        use std::io::Write;
        unsafe extern "C" { fn kill(pid: i32, signal: i32) -> i32; }
        if self.child.try_wait().ok().flatten().is_none() {
            unsafe { kill(-(self.child.id() as i32), 9); }
        }
        let _ = self.child.wait();
        if let Some(mut input) = self.guardian.stdin.take() { let _ = input.write_all(b"done\n"); }
        let _ = self.guardian.wait();
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn child_interface_preserves_exit_status() {
        let mut encoder = Encoder::spawn(&mut Command::new("/usr/bin/true")).unwrap();
        assert!(encoder.child.wait().unwrap().success());
    }
    #[test]
    fn dropping_encoder_reaps_its_process() {
        let encoder = Encoder::spawn(Command::new("/bin/sleep").arg("30")).unwrap();
        let pid = encoder.child.id();
        drop(encoder);
        unsafe extern "C" { fn kill(pid: i32, signal: i32) -> i32; }
        assert_eq!(unsafe { kill(pid as i32, 0) }, -1);
    }
}
