use crate::config::*;
use crate::fs::{open, FileOpenMode, SeekMode};
use crate::memory::{copyout, copyout_vec};
use crate::proc::{get_current_user_token, get_current_task};
use crate::timer::get_time_ms;
use crate::utils::Error;
use super::time::Timeval;
use log::*;

pub fn sys_uname(utsname_ptr: *mut Utsname) -> Result<isize, Error> {
    let token = get_current_user_token();
    copyout(token, utsname_ptr, &Utsname::default_init())?;
    Ok(0)
}

pub fn sys_getrusage(who: i32, usage: *mut Rusage) -> Result<isize, Error> {
    let task = get_current_task().unwrap();
    let time_info = task.time_info.lock();
    let token = task.get_user_token();
    let mut rusage: Rusage = Default::default();
    rusage.ru_utime = Timeval::from_tick(time_info.utime as usize);
    rusage.ru_stime = Timeval::from_tick(time_info.stime as usize);

    copyout(token, usage, &rusage)?;
    Ok(0)
}

pub fn sys_syslog(types: i32, buf: *mut u8, len: i32) -> Result<isize, Error> {
    const SYSLOG_ACTION_CLOSE: i32 = 0;
    const SYSLOG_ACTION_OPEN: i32 = 1;
    const SYSLOG_ACTION_READ: i32 = 2;
    const SYSLOG_ACTION_READ_ALL: i32 = 3;
    const SYSLOG_ACTION_READ_CLEAR: i32 = 4;
    const SYSLOG_ACTION_CLEAR: i32 = 5;
    const SYSLOG_ACTION_CONSOLE_OFF: i32 = 6;
    const SYSLOG_ACTION_CONSOLE_ON: i32 = 7;
    const SYSLOG_ACTION_CONSOLE_LEVER: i32 = 8;
    const SYSLOG_ACTION_SIZE_UNREAD: i32 = 9;
    const SYSLOG_ACTION_SIZE_BUFFER: i32 = 10;
    /* 为了方便，将syslog作为文件 */
    trace!("types = {}, len = {:x}", types, len);
    let task = get_current_task().unwrap();
    let token = task.get_user_token();
    let syslog = open("syslog".into(), FileOpenMode::SYS)?;
    match types {
        SYSLOG_ACTION_READ_ALL => {
            syslog.seek(0, SeekMode::SET)?;
            let data = syslog.read(0x400)?;
            let len = data.len();
            copyout_vec(token, buf, data)?;
            Ok(len as isize)
        }
        SYSLOG_ACTION_SIZE_BUFFER => {
            return Ok(0x400)
        },
        _ => unimplemented!()
    }
    //todo!()
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct SysInfo {
    pub uptime      :usize,
    pub loads       :[usize; 3],
    pub totalram    :usize,
    pub freeram     :usize,
    pub sharedram   :usize,
    pub bufferram   :usize,
    pub totalswap   :usize,
    pub freeswap    :usize,
    pub reserve     :[u8; 22]
}

pub fn sys_sysinfo(info_ptr: *mut SysInfo) -> Result<isize, Error> {
    let mut info: SysInfo = Default::default();
    const MSEC_PER_SEC: usize = 1000;
    info.uptime = get_time_ms() / MSEC_PER_SEC;
    info.totalram = 0x80_000_000;
    let token = get_current_user_token();
    copyout(token, info_ptr, &info)?;
    Ok(0)
}

pub fn sys_getrandom(buf:usize,buf_len:usize,flag:u32) -> Result<isize, Error> {
    Ok(buf_len as isize)
}


#[repr(C)]
#[derive(Debug, Default)]
pub struct Rusage{
    ru_utime   :Timeval,     /* user CPU time used */
    ru_stime   :Timeval,     /* system CPU time used */
    ru_maxrss  :isize  ,     /* maximum resident set size */
    ru_ixrss   :isize  ,     /* integral shared memory size */
    ru_idrss   :isize  ,     /* integral unshared data size */
    ru_isrss   :isize  ,     /* integral unshared stack size */
    ru_minflt  :isize  ,     /* page reclaims (soft page faults) */
    ru_majflt  :isize  ,     /* page faults (hard page faults) */
    ru_nswap   :isize  ,     /* swaps */
    ru_inblock :isize  ,     /* block input operations */
    ru_oublock :isize  ,     /* block output operations */
    ru_msgsnd  :isize  ,     /* IPC messages sent */
    ru_msgrcv  :isize  ,     /* IPC messages received */
    ru_nsignals:isize  ,     /* signals received */
    ru_nvcsw   :isize  ,     /* voluntary context switches */
    ru_nivcsw  :isize  ,     /* involuntary context switches */
}

#[repr(C)]
#[derive(Debug)]
pub struct Utsname {
    pub sysname     : [u8; UTSNAME_LEN],
    pub nodename    : [u8; UTSNAME_LEN],
    pub release     : [u8; UTSNAME_LEN],
    pub version     : [u8; UTSNAME_LEN],
    pub machine     : [u8; UTSNAME_LEN],
    pub domainname  : [u8; UTSNAME_LEN],
}

impl Utsname{
    pub fn default_init() -> Self {
        let mut utsname = Utsname{
            sysname     : [0; UTSNAME_LEN],
            nodename    : [0; UTSNAME_LEN],
            release     : [0; UTSNAME_LEN],
            version     : [0; UTSNAME_LEN],
            machine     : [0; UTSNAME_LEN],
            domainname  : [0; UTSNAME_LEN],
        };
        utsname.sysname[..UTS_SYSNAME.len()].copy_from_slice(UTS_SYSNAME.as_bytes());
        utsname.nodename[..UTS_NODENAME.len()].copy_from_slice(UTS_NODENAME.as_bytes());
        utsname.release[..UTS_RELEASE.len()].copy_from_slice(UTS_RELEASE.as_bytes());
        utsname.version[..UTS_VERSION.len()].copy_from_slice(UTS_VERSION.as_bytes());
        utsname.machine[..UTS_MACHINE.len()].copy_from_slice(UTS_MACHINE.as_bytes());
        utsname.domainname[..UTS_DOMAINNAME.len()].copy_from_slice(UTS_DOMAINNAME.as_bytes());
        utsname
    }
}

