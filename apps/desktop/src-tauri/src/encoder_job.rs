//! Child encoders cannot keep capturing after Whispera exits or crashes.
use std::{os::windows::io::AsRawHandle,process::{Child,Command}};
use windows::Win32::{Foundation::{CloseHandle,HANDLE},System::JobObjects::{CreateJobObjectW,AssignProcessToJobObject,SetInformationJobObject,JobObjectExtendedLimitInformation,JOBOBJECT_EXTENDED_LIMIT_INFORMATION,JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE}};
pub struct Encoder {pub child:Child,job:HANDLE}
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
impl Drop for Encoder{fn drop(&mut self){let _=self.child.kill();let _=self.child.wait();unsafe{let _=CloseHandle(self.job);}}}
