#![allow(dead_code)]
use core::arch::asm;
use crate::{Timespec, Timeval, Tms , Sigaction};

use super::Utsname;

const SYSCALL_GETCWD        :usize = 17;
const SYSCALL_DUP           :usize = 23;
const SYSCALL_DUP3          :usize = 24;
const SYSCALL_MKDIRAT       :usize = 34;
const SYSCALL_UNLINKAT      :usize = 35;
const SYSCALL_LINKAT        :usize = 37;
const SYSCALL_UMOUNT        :usize = 39;
const SYSCALL_MOUNT         :usize = 40;
const SYSCALL_CHDIR         :usize = 49;
const SYSCALL_OPENAT        :usize = 56;
const SYSCALL_CLOSE         :usize = 57;
const SYSCALL_PIPE2         :usize = 59;
const SYSCALL_GETDENTS      :usize = 61;
const SYSCALL_READ          :usize = 63;
const SYSCALL_WRITE         :usize = 64;
const SYSCALL_FSTAT         :usize = 80;
const SYSCALL_EXIT          :usize = 93;
const SYSCALL_NANOSLEEP     :usize = 101;
const SYSCALL_SCHED_YIELD   :usize = 124;
const SYSCALL_KILL          :usize = 129;
const SYSCALL_TGKILL        :usize = 131;
const SYSCALL_SIGACTION     :usize = 134;
const SYSCALL_SIGPROCMASK   :usize = 135;
const SYSCALL_SIGRETURN     :usize = 139;
const SYSCALL_TIMES         :usize = 153;
const SYSCALL_UNAME         :usize = 160;
const SYSCALL_GETTIMEOFDAY  :usize = 169;
const SYSCALL_GETPID        :usize = 172;
const SYSCALL_GETPPID       :usize = 173;
const SYSCALL_BRK           :usize = 214;
const SYSCALL_MUNMAP        :usize = 215;
const SYSCALL_CLONE         :usize = 220;
const SYSCALL_EXECVE        :usize = 221;
const SYSCALL_MMAP          :usize = 222;
const SYSCALL_WAIT          :usize = 260;
const SYSCALL_SHUTDOWN      :usize = 999;

fn syscall(id:usize, args: [usize; 6]) -> isize {
    let mut ret: isize;
    unsafe{
        asm!{
            "ecall",
            inlateout("x10") args[0] => ret,
            in("x11") args[1],
            in("x12") args[2],
            in("x13") args[3],
            in("x14") args[4],
            in("x15") args[5],
            in("x17") id
        };
    }
    ret
}

pub fn sys_getcwd(buffer: &mut [u8]) -> isize {
    syscall(SYSCALL_GETCWD, [buffer.as_ptr() as usize, buffer.len(), 0, 0, 0, 0])
}

pub fn sys_pipe(pipe_fd_ptr:*mut usize,flag:usize) -> isize{
    syscall(SYSCALL_PIPE2,[pipe_fd_ptr as usize,flag,0,0,0,0])
}

pub fn sys_dup(fd: usize) -> isize {
    syscall(SYSCALL_DUP, [fd, 0, 0, 0, 0, 0])
}

pub fn sys_dup3(old: usize, new: usize) -> isize {
    syscall(SYSCALL_DUP3, [old, new, 0, 0, 0, 0])
}

pub fn sys_chdir(path: &str) -> isize {
    syscall(SYSCALL_CHDIR, [path.as_ptr() as usize, 0, 0, 0, 0, 0])
}

pub fn sys_open(fd: isize, filename: &str, flags: u32, mode: u32) -> isize {
    syscall(SYSCALL_OPENAT, [fd as usize, filename.as_ptr() as usize, flags as usize, mode as usize, 0, 0])
}

pub fn sys_close(fd: usize) -> isize {
    syscall(SYSCALL_CLOSE, [fd, 0, 0, 0, 0, 0])
}

pub fn sys_getdents(fd: usize, buffer: &mut [u8]) -> isize {
    syscall(SYSCALL_GETDENTS, [fd, buffer.as_ptr() as usize, buffer.len(), 0, 0, 0])
}

pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
    syscall(SYSCALL_WRITE, [fd, buffer.as_ptr() as usize, buffer.len(), 0, 0, 0])
}

pub fn sys_read(fd: usize, buffer: &mut [u8]) -> isize {
    syscall(SYSCALL_READ, [fd, buffer.as_mut_ptr() as usize, buffer.len(), 0, 0, 0])
}

pub fn sys_linkat(
    oldfd: isize, 
    oldpath: &str, 
    newfd: isize, 
    newpath: &str, 
    flags: u32
) -> isize {
    syscall(SYSCALL_LINKAT, 
        [oldfd as usize, oldpath.as_ptr() as usize, newfd as usize, newpath.as_ptr() as usize, flags as usize, 0])
}

pub fn sys_unlinkat(
    dirfd: isize, 
    path: &str,
    flags: u32
) -> isize {
    syscall(SYSCALL_UNLINKAT, [dirfd as usize, path.as_ptr() as usize, flags as usize, 0, 0, 0])
}

pub fn sys_mkdir(
    dirfd: isize,
    path: &str,
    mode: u32,
) -> isize {
    syscall(SYSCALL_MKDIRAT, [dirfd as usize, path.as_ptr() as usize, mode as usize, 0, 0, 0])
}

pub fn sys_umount(
    path: &str,
    flags: usize,
) -> isize {
    syscall(SYSCALL_UMOUNT, [path.as_ptr() as usize, flags, 0, 0, 0, 0])
}

pub fn sys_mount(
    dev: &str,
    dir: &str,
    fstype: &str,
    flags: usize,
    data: &str
) -> isize {
    syscall(SYSCALL_MOUNT, 
        [
                dev.as_ptr() as usize, 
                dir.as_ptr() as usize, 
                fstype.as_ptr() as usize, 
                flags, 
                data.as_ptr() as usize, 0
            ])
}


pub fn sys_exit(exit_code: i32) -> !{
    syscall(SYSCALL_EXIT, [exit_code as usize, 0, 0, 0, 0, 0]);
    panic!("sys_exit never return");
}

pub fn sys_sched_yield() -> isize {
    syscall(SYSCALL_SCHED_YIELD, [0, 0, 0, 0, 0, 0])
}

pub fn sys_clone(flags: usize, stack: usize, ptid: usize, tls: usize, ctid: usize) -> isize {
    syscall(SYSCALL_CLONE, [flags, stack, ptid, tls, ctid, 0])
}

pub fn sys_execve(path: &str, argv: &[*const u8], envp: &[*const u8]) -> isize {
    syscall(SYSCALL_EXECVE, [path.as_ptr() as usize, argv.as_ptr() as usize, envp.as_ptr() as usize, 0, 0, 0])
}

pub fn sys_getpid() -> isize {
    syscall(SYSCALL_GETPID, [0, 0, 0, 0, 0, 0])
}

pub fn sys_getppid() -> isize {
    syscall(SYSCALL_GETPPID, [0, 0, 0, 0, 0, 0])
}

/// pid = -1 表示等待任意一个进程结束
/// 如果等待的进程退出，则返回退出进程的pid。 否则返回 - 2
pub fn sys_wait(pid: isize, exit_code: *mut i32) -> isize {
    syscall(SYSCALL_WAIT, [pid as usize, exit_code as usize, 0, 0, 0, 0])
}

pub fn sys_nanosleep(req: *const Timespec, rem: *mut Timespec) -> isize {
    syscall(SYSCALL_NANOSLEEP, [req as usize, rem as usize, 0, 0, 0, 0])
}

pub fn sys_gettimeofday(timeval: *mut Timeval, _timezone: usize) -> isize {
    syscall(SYSCALL_GETTIMEOFDAY, [timeval as usize, 0, 0, 0, 0, 0])
}

pub fn sys_times(tms: *mut Tms) -> isize {
    syscall(SYSCALL_TIMES, [tms as usize, 0, 0, 0, 0, 0])
}

pub fn sys_uname(utsname: *mut Utsname) -> isize {
    syscall(SYSCALL_UNAME, [utsname as *mut Utsname as usize, 0, 0, 0, 0, 0])
}

pub fn sys_brk(addr:usize) -> isize{
    syscall(SYSCALL_BRK,[addr,0,0,0,0,0])
}

pub fn sys_mmap(start:usize,len:usize,prot:usize,flags:usize,fd:usize,off:usize) -> isize{
    syscall(SYSCALL_MMAP,[start,len,prot,flags,fd,off])
}

pub fn sys_unmmap(start:usize,len:usize) -> isize{
    syscall(SYSCALL_MUNMAP,[start,len,0,0,0,0])
}

pub fn sys_kill(pid:isize,signal: usize) -> isize{
    syscall(SYSCALL_KILL,[pid as usize,signal,0,0,0,0])
}

pub fn sys_sigprocmask(how:usize,set:*mut usize,oldset:*mut usize) -> isize{
    syscall(SYSCALL_SIGPROCMASK,[how,set as usize,oldset as usize,0,0,0])
}
pub fn sys_sigaction(signum:usize,act:*mut Sigaction,old_act:*mut Sigaction) -> isize{
    syscall(SYSCALL_SIGACTION,[signum,act as usize,old_act as usize,0,0,0])
}

pub fn sys_shutdown() -> isize {
    syscall(SYSCALL_SHUTDOWN, [0,0,0,0,0,0])
}