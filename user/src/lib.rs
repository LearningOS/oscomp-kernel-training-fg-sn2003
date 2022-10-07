#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

#[macro_use]
pub mod console;
mod syscall;
mod lang_item;
mod config;

extern crate alloc;
use alloc::string::String;
use config::*;
use syscall::*;
use buddy_system_allocator::LockedHeap;
use alloc::vec::Vec;
use core::mem::zeroed;
use core::arch::global_asm;

#[macro_use]
extern crate bitflags;

const USER_HEAP_SIZE:usize = 0x10_000;
//动态内存分配区分配在.data段
static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];
pub const SIGHUP    : usize =  1; 
pub const SIGINT    : usize =  2; 
pub const SIGQUIT   : usize =  3; 
pub const SIGILL    : usize =  4; 
pub const SIGTRAP   : usize =  5; 
pub const SIGABRT   : usize =  6; 
pub const SIGBUS    : usize =  7; 
pub const SIGFPE    : usize =  8; 
pub const SIGKILL   : usize =  9; 
pub const SIGUSR1   : usize = 10; 
pub const SIGSEGV   : usize = 11; 
pub const SIGUSR2   : usize = 12; 
pub const SIGPIPE   : usize = 13; 
pub const SIGALRM   : usize = 14; 
pub const SIGTERM   : usize = 15; 
pub const SIGSTKFLT : usize = 16; 
pub const SIGCHLD   : usize = 17; 
pub const SIGCONT   : usize = 18; 
pub const SIGSTOP   : usize = 19; 
pub const SIGTSTP   : usize = 20; 
pub const SIGTTIN   : usize = 21; 
pub const SIGTTOU   : usize = 22; 
pub const SIGURG    : usize = 23; 
pub const SIGXCPU   : usize = 24; 
pub const SIGXFSZ   : usize = 25; 
pub const SIGVTALRM : usize = 26; 
pub const SIGPROF   : usize = 27; 
pub const SIGWINCH  : usize = 28; 
pub const SIGIO     : usize = 29; 
pub const SIGPWR    : usize = 30; 
pub const SIGSYS    : usize = 31; 
#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

global_asm!(include_str!("clone.s"));

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start(argc: usize, argv: *const usize) -> ! {
    unsafe {
        HEAP.lock().init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);
    }

    let mut arg_strings = Vec::new();
    for i in 0..argc {
        let arg_start: *const u8 = unsafe{
            *argv.add(i) as *const u8
        };
        let mut arg_end = arg_start;
        loop{
            unsafe{
                if *arg_end == 0{
                    break
                }
                arg_end = arg_end.add(1);
            }
        }
        unsafe{
            arg_strings.push(core::str::from_utf8(
                core::slice::from_raw_parts(arg_start, arg_end as usize - arg_start as usize)).unwrap()
            ) 
        }
    }

    exit(main(argc, arg_strings.as_slice()));
    panic!("unreachable after sys_exit!");
}

#[linkage = "weak"]
#[no_mangle]
fn main(_argc: usize, _argv: &[&str]) -> i32 {
    panic!("Cannot find main!");
}

pub const AT_FDCWD: isize = -100;

pub fn shutdown() -> isize {
    sys_shutdown()
}

pub fn chdir(path: &str) -> isize {
    let mut temp = String::from(path);
    temp.push(0 as char);
    sys_chdir(path)
}

pub fn getcwd(buf: &mut [u8]) -> isize {
    sys_getcwd(buf)
}

pub fn open(fd: isize, filename: &str, flags: u32, mode: u32) -> isize {
    let mut temp = String::from(filename);
    temp.push(0 as char);
    sys_open(fd, temp.as_str(), flags, mode)
}

pub fn close(fd: usize) -> isize {
    sys_close(fd)
}

pub fn mkdir(fd: isize, path: &str, mode: u32) -> isize {
    let mut temp = String::from(path);
    temp.push(0 as char);
    sys_mkdir(fd, temp.as_str(), mode)
}

pub fn getdent(fd: usize, buf: &mut [u8]) -> isize {
    sys_getdents(fd, buf)
}

pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}

pub fn read(fd: usize, buf: &mut [u8]) -> isize {
    sys_read(fd, buf)
}


pub fn exit(exit_code: i32) -> isize {
    sys_exit(exit_code)
}

pub fn yield_() -> isize {
    sys_sched_yield()
}

pub fn fork() -> isize {
    sys_clone(0,0,0,0,0)
}

extern "C" {
    fn __clone(
        func: usize,
        stack_addr: usize,
        flags: usize, 
        ptid: usize,
        tls: usize,
        ctid: usize,
    ) -> isize;
}

pub fn clone(
    func: usize,
    _arg: usize,
    stack_addr: usize,
    stack_size: usize,
    flags: usize, 
    _parent_tid: usize,
    _tls: usize,
    _child_tid: usize,
) -> isize {
    let stack_top = match stack_addr {
        0 => 0,
        _ => stack_addr + stack_size
    };
    unsafe {__clone(func, stack_top, flags, 0, 0, 0) }
}

/// 返回值：如果出错的话（找不到名字相符的可执行文件）则返回 -1，否则不应该返回。
/// 调用者需保证path的末尾加上 ‘\0’， 这样内核才能直到字符串的长度 
/// todo: 接收argp
pub fn exec(path: &str, argv: Vec<&str>, envp: Vec<&str>) -> isize {
    let mut argv: Vec<*const u8> = argv
        .iter()
        .map(|str| {
            str.as_ptr()
        })
        .collect();
    argv.push(core::ptr::null());
    //println!("argv = {:?}", argv);

    let mut envp: Vec<*const u8> = envp
        .iter()
        .map(|str| {
               str.as_ptr()
        })
        .collect();    
    envp.push(core::ptr::null());
    sys_execve(path, argv.as_slice(), envp.as_slice())
}

pub fn getpid() -> isize {
    sys_getpid()
}

pub fn getppid() -> isize {
    sys_getppid()
}

pub fn wait(exit_code: &mut i32) -> isize {
    sys_wait(-1, exit_code as *mut i32)
}

pub fn waitpid(pid: usize, exit_code: &mut i32) -> isize {
    sys_wait(pid as isize, exit_code as *mut i32)
}

pub fn sleep(time: usize) -> isize {
    let mut tv = Timespec {
        tv_sec: time as isize,
        tv_nsec: 0,
    };
    
    let ret = sys_nanosleep(&tv as *const Timespec, &mut tv as *mut Timespec);

    match ret {
        0 => 0,
        _ => tv.tv_sec
    }
}

pub fn get_time() -> isize {
    let mut timeval: Timeval = unsafe {zeroed()}; 
    let ret = sys_gettimeofday(&mut timeval, 0);
    match ret {
        0 => (timeval.tv_sec & 0xffff) * 1000 + timeval.tv_usec / 1000 ,
        _ => -1
    }
}

pub fn times(times: &mut Tms) -> isize {
    sys_times(times as *mut Tms)
}

pub fn uname(utsname: &mut Utsname) -> isize {
    sys_uname(utsname as *mut Utsname)
}

pub const PROT_READ      :usize=  0x1;                /* Page can be read.  */
pub const PROT_WRITE     :usize=  0x2;               /* Page can be written.  */
pub const PROT_EXEC      :usize=  0x4;                /* Page can be executed.  */
pub const PROT_NONE      :usize=  0x0;                /* Page can not be accessed.  */

pub fn brk(addr: usize) -> isize{
    sys_brk(addr)
}

pub fn mmap(start:usize,len:usize,prot:usize,flags:usize,fd:usize,off:usize) -> isize{
    sys_mmap(start,len,prot,flags,fd,off)
}

pub fn unmmap(start:usize,len:usize) -> isize{
    sys_unmmap(start,len)
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

#[repr(C)]
pub struct Timespec {
    pub tv_sec: isize,
    pub tv_nsec: isize,
}

#[repr(C)]
pub struct Timeval {
    pub tv_sec: isize,
    pub tv_usec: isize,
}

#[repr(C)]
pub struct Tms {
    pub tms_utime: isize,   //进程在U态消耗的CPU时间
    pub tms_stime: isize,   //进程在S态消耗的CPU时间
    pub tms_cutime: isize,  //所有子进程的utime之和
    pub tms_cstime: isize,  //所有子进程的stime之和
}

bitflags! {
    pub struct FileOpenMode: u32 {
        const READ      = 0;
        const WRITE     = 1 << 0;
        const RDWR      = 1 << 1;
        const CREATE    = 1 << 6;   
        const EXEC      = 1 << 7;   // 表示如果要创建的文件已存在，则出错，同时返回 -1
        const TRUNC     = 1 << 9;   // 表示截断，如果文件存在，并且以只写、读写方式打开，则将其长度截断为0
        const APPEND    = 1 << 10;  // 
        const NO_FOLLOW = 1 << 5;   // 打开不跟随link
    }
}

bitflags! {
    pub struct FilePerm: u32 {
        const NONE    = 0;
        const OWNER_R = 0o400;
        const OWNER_W = 0o200;
        const OWNER_X = 0o100;
        const GROUP_R = 0o040;
        const GROUP_W = 0o020;
        const GROUP_X = 0o010;
        const OTHER_R = 0o004;
        const OTHER_W = 0o002;
        const OTHER_X = 0o001;
    }
}

pub fn pipe(pipe_fd_ptr:&mut [usize;2],flag:usize) -> isize{
    sys_pipe(pipe_fd_ptr as *mut usize, flag)
}

pub fn kill(pid:isize,signal: usize) -> isize{
    sys_kill(pid,signal)
}


pub const SIG_BLOCK     : usize = 0;
pub const SIG_UNBLOCK   : usize = 1;
pub const SIG_SETMASK   : usize = 2;

//sa_flags
pub const SIGINFO    :usize = 0x00000004;//目前只用到这个
#[repr(C)]
pub struct SigInfo{
    si_signo:usize,
    si_errno:usize,
    si_code:usize,
    //si_trapno:usize,
}
#[repr(C)]
pub struct Sigaction{
    pub sa_handler:usize,//默认信号处理函数
    pub sa_sigaction:usize,//自定义信号处理函数,暂不支持传参
    pub sa_mask:usize,//信号处理函数执行期间的mask码
    pub sa_flags:usize,//标志位
    pub sa_restorer:usize,//也许用不上,保存用户进程trampoline地址?
    pub sig_info_ptr:usize,
}
pub fn sigprocmask(how:usize,set:*mut usize,oldset:*mut usize) -> isize{
    sys_sigprocmask(how, set, oldset)
}
pub fn sigaction(signum:usize,act:*mut Sigaction,old_act:*mut Sigaction) -> isize{
    sys_sigaction(signum,act,old_act)
}