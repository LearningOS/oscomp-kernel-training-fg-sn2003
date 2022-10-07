#![allow(dead_code)]
pub mod fs;
pub mod memory;
pub mod process;
pub mod other;
pub mod time;
pub mod signal;
pub mod net;

use fs::*;
use memory::*;
use process::*; 
use other::*;
use time::*;
use signal::*;
use net::*;
use log::*;

use crate::fs::vfs::Statvfs;
use crate::{fs::FileStat, proc::Rlimit};
use crate::proc::{get_current_task, get_current_trap_context};
use alloc::{
    collections::BTreeMap
};
use spin::lazy::Lazy;

pub static SYSCALL_NAMES: Lazy<BTreeMap<usize, &'static str>> = Lazy::new(||{
    let mut map = BTreeMap::new();
        map.insert(17,  "getcwd");
        map.insert(23,  "dup");
        map.insert(24,  "dup3");
        map.insert(25,  "fcntl");
        map.insert(29,  "ioctl");
        map.insert(34,  "mkdirat");
        map.insert(35,  "unlinkat");
        map.insert(37,  "linkat");
        map.insert(39,  "umount");
        map.insert(40,  "mount");
        map.insert(43,  "statfs");
        map.insert(48,  "faccessat");
        map.insert(49,  "chdir");
        map.insert(56,  "openat");
        map.insert(57,  "close");
        map.insert(59,  "pipe2");
        map.insert(61,  "getdents");
        map.insert(62,  "lseek");
        map.insert(63,  "read");
        map.insert(64,  "write");
        map.insert(65,  "readv");
        map.insert(66,  "writev");
        map.insert(67,  "pread");
        map.insert(68,  "pwrite");
        map.insert(72,  "pselect");
        map.insert(73,  "ppoll");
        map.insert(78,  "readlinkat");
        map.insert(79,  "newfstatat");
        map.insert(80,  "fstat");
        map.insert(88,  "utimensat");
        map.insert(93,  "exit");
        map.insert(94,  "exit_group");
        map.insert(96,  "set_tid_address");
        map.insert(98,  "futex");
        map.insert(99,  "set_robust_list");
        map.insert(100, "get_robust_list");
        map.insert(101, "nanosleep");
        map.insert(103, "setitimer");
        map.insert(113, "clock_gettime");
        map.insert(129, "kill");
        map.insert(130, "tkill");
        map.insert(134, "sigaction");
        map.insert(135, "sigprocmask");
        map.insert(137, "sigtimedwait");
        map.insert(139, "sigreturn");
        map.insert(155, "getpgid");
        map.insert(160, "uname");
        map.insert(165, "getrusage");
        map.insert(169, "gettimeofday");
        map.insert(172, "getpid");
        map.insert(173, "getppid");
        map.insert(174, "getuid");
        map.insert(175, "geteuid");
        map.insert(177, "getegid");
        map.insert(178, "gettid");
        map.insert(214, "brk");
        map.insert(215, "munmap");
        map.insert(220, "clone");
        map.insert(221, "execve");
        map.insert(222, "mmap");
        map.insert(226, "mprotect");
        map.insert(261, "prlimit");
        map.insert(998, "stop");
        map.insert(999, "shutdown");
        map
    }
);


pub const SYSCALL_GETCWD            :usize = 17;
pub const SYSCALL_DUP               :usize = 23;
pub const SYSCALL_DUP3              :usize = 24;
pub const SYSCALL_FCNTL             :usize = 25;
pub const SYSCALL_IOCTL             :usize = 29;
pub const SYSCALL_MKDIRAT           :usize = 34;
pub const SYSCALL_UNLINKAT          :usize = 35;
pub const SYSCALL_LINKAT            :usize = 37;
pub const SYSCALL_UMOUNT            :usize = 39;
pub const SYSCALL_MOUNT             :usize = 40;
pub const SYSCALL_STATFS            :usize = 43;
pub const SYSCALL_FACCESSAT         :usize = 48;
pub const SYSCALL_CHDIR             :usize = 49;
pub const SYSCALL_OPENAT            :usize = 56;
pub const SYSCALL_CLOSE             :usize = 57;
pub const SYSCALL_PIPE2             :usize = 59;
pub const SYSCALL_GETDENTS          :usize = 61;
pub const SYSCALL_LSEEK             :usize = 62;
pub const SYSCALL_READ              :usize = 63;
pub const SYSCALL_WRITE             :usize = 64;
pub const SYSCALL_READV             :usize = 65;
pub const SYSCALL_WRITEV            :usize = 66;
pub const SYSCALL_PREAD             :usize = 67;
pub const SYSCALL_PWRITE            :usize = 68;
pub const SYSCALL_SENDFILE          :usize = 71;
pub const SYSCALL_PSELECT6          :usize = 72;
pub const SYSCALL_PPOLL             :usize = 73;
pub const SYSCALL_READLINKAT        :usize = 78;
pub const SYSCALL_NEWFSTATAT        :usize = 79;
pub const SYSCALL_FSTAT             :usize = 80;
pub const SYSCALL_FSYNC             :usize = 82;
pub const SYSCALL_UTIMENSAT         :usize = 88;
pub const SYSCALL_EXIT              :usize = 93;
pub const SYSCALL_EXIT_GROUP        :usize = 94;
pub const SYSCALL_SET_TID_ADDRESS   :usize = 96;
pub const SYSCALL_FUTEX             :usize = 98;
pub const SYSCALL_SET_ROBUST_LIST   :usize = 99;
pub const SYSCALL_GET_ROBUST_LIST   :usize = 100;
pub const SYSCALL_NANOSLEEP         :usize = 101;
pub const SYSCALL_SETITIMER         :usize = 103;
pub const SYSCALL_CLOCKGETTIME      :usize = 113;
pub const SYSCALL_SYSLOG            :usize = 116;
pub const SYSCALL_SCHED_YIELD       :usize = 124;
pub const SYSCALL_KILL              :usize = 129;
pub const SYSCALL_TKILL             :usize = 130;
pub const SYSCALL_TGKILL            :usize = 131;
pub const SYSCALL_SIGACTION         :usize = 134;
pub const SYSCALL_SIGPROCMASK       :usize = 135;
pub const SYSCALL_RT_SIGTIMEDWAIT   :usize = 137;
pub const SYSCALL_SIGRETURN         :usize = 139;
pub const SYSCALL_TIMES             :usize = 153;
pub const SYSCALL_GETPGID           :usize = 155;
pub const SYSCALL_UNAME             :usize = 160;
pub const SYSCALL_GETRUSAGE         :usize = 165;
pub const SYSCALL_UMASK             :usize = 166;
pub const SYSCALL_GETTIMEOFDAY      :usize = 169;
pub const SYSCALL_GETPID            :usize = 172;
pub const SYSCALL_GETPPID           :usize = 173;
pub const SYSCALL_GETUID            :usize = 174;
pub const SYSCALL_GETEUID           :usize = 175;
pub const SYSCALL_GETEGID           :usize = 177;
pub const SYSCALL_GETTID            :usize = 178;
pub const SYSCALL_SYSINFO           :usize = 179;
pub const SYSCALL_SOCKET            :usize = 198;
pub const SYSCALL_BIND              :usize = 200;
pub const SYSCALL_LISTEN            :usize = 201;
pub const SYSCALL_ACCEPT            :usize = 202;
pub const SYSCALL_CONNECT           :usize = 203;
pub const SYSCALL_GETSOCKNAME       :usize = 204;
pub const SYSCALL_SENDTO            :usize = 206;
pub const SYSCALL_RECVFROM          :usize = 207;
pub const SYSCALL_SETSOCKOPT        :usize = 208;
pub const SYSCALL_BRK               :usize = 214;
pub const SYSCALL_MUNMAP            :usize = 215;
pub const SYSCALL_CLONE             :usize = 220;
pub const SYSCALL_EXECVE            :usize = 221;
pub const SYSCALL_MMAP              :usize = 222;
pub const SYSCALL_MPROTECT          :usize = 226;
pub const SYSCALL_MSYNC             :usize = 227;
pub const SYSCALL_MADVISE           :usize = 233;
pub const SYSCALL_WAIT              :usize = 260;
pub const SYSCALL_PRLIMIT           :usize = 261;
pub const SYSCALL_RENAMEAT2         :usize = 276;
pub const SYSCALL_MEMBARRIER        :usize = 283;
pub const SYSCALL_STOP              :usize = 998;
pub const SYSCALL_SHUTDOWN          :usize = 999;

pub fn strace(id: usize) -> bool {
    let tid = get_current_task().unwrap().tid;
    
    if id != SYSCALL_WAIT && id != SYSCALL_SCHED_YIELD
    && id != SYSCALL_GETTIMEOFDAY && id != SYSCALL_GETRUSAGE
    && id != SYSCALL_CLOCKGETTIME && id != SYSCALL_CLOSE {
        let name = SYSCALL_NAMES.get(&id).unwrap_or(&"UNKONWN");
        warn!("{} syscall_{}: tid = {}",id, name, tid);
        true
    } else {
        false
    }
}

//note: 所有sys_开头的实现函数的返回类型应该为 isize 或 !
pub fn syscall(id: usize, args: [usize; 6]) -> isize {
    //let isstrace = strace(id);
    let isstrace = false;
    let ret = match id {
        SYSCALL_GETCWD          => sys_getcwd(args[0] as *mut u8, args[1]),
        SYSCALL_PIPE2           => sys_pipe(args[0] as *mut u32,args[1]),
        SYSCALL_DUP             => sys_dup(args[0] as u32),
        SYSCALL_DUP3            => sys_dup3(args[0] as u32, args[1] as u32),
        SYSCALL_FCNTL           => sys_fcntl(args[0] as u32, args[1] as u32, args[3]),
        SYSCALL_IOCTL           => sys_ioctl(args[0] as u32, args[1] as u32, args[2] as usize),
        SYSCALL_CHDIR           => sys_chdir(args[0] as *const u8),
        SYSCALL_OPENAT          => sys_open(args[0] as i32, args[1] as *const u8, args[2] as u32, args[3] as u32),
        SYSCALL_CLOSE           => sys_close(args[0] as u32),
        SYSCALL_GETDENTS        => sys_getdents(args[0] as u32, args[1] as *mut u8, args[2]),
        SYSCALL_LINKAT          => sys_linkat(args[0] as isize, args[1] as *const u8, args[2] as isize, args[3] as *const u8, args[4] as u32),
        SYSCALL_UNLINKAT        => sys_unlinkat(args[0] as i32, args[1] as *const u8, args[3] as u32),
        SYSCALL_MKDIRAT         => sys_mkdir(args[0] as i32, args[1] as *const u8, args[3] as u32),
        SYSCALL_UMOUNT          => sys_umount(args[0] as *const u8, args[1]),
        SYSCALL_MOUNT           => sys_mount(args[0] as *const u8, args[1] as *const u8, args[2] as *const u8, args[3], args[4] as *const u8),
        SYSCALL_STATFS          => sys_statfs(args[0] as *const u8, args[1] as *mut Statvfs),
        SYSCALL_FACCESSAT       => sys_faccessat(args[0] as i32, args[1] as *const u8, args[2] as u32, args[3] as u32),
        SYSCALL_LSEEK           => sys_lseek(args[0] as u32, args[1], args[2] as u32),
        SYSCALL_READ            => sys_read(args[0] as u32, args[1] as *mut u8, args[2]),
        SYSCALL_WRITE           => sys_write(args[0] as u32, args[1] as *const u8, args[2]),
        SYSCALL_READV           => sys_readv(args[0] as u32, args[1] as *const Iovec, args[2]),
        SYSCALL_WRITEV          => sys_writev(args[0] as u32, args[1] as *const Iovec, args[2]),
        SYSCALL_PREAD           => sys_pread(args[0] as u32, args[1] as *mut u8, args[2], args[3]),
        SYSCALL_PWRITE          => sys_pwrite(args[0] as u32, args[1] as *const u8, args[2], args[3]),
        SYSCALL_SENDFILE        => sys_sendfile(args[0] as u32, args[1] as _, args[2] as _, args[3] as _),
        SYSCALL_PSELECT6        => sys_pselect(args[0] as _, args[1] as _, args[2] as _, args[3] as _, args[4] as _, args[5] as _),
        SYSCALL_PPOLL           => sys_ppoll(args[0] as *mut Pollfd, args[1], args[2] as *const Timespec, args[3] as *const usize),
        SYSCALL_READLINKAT      => sys_readlinkat(args[0] as _, args[1] as _, args[2] as _, args[3] as _),
        SYSCALL_UTIMENSAT       => sys_utimensat(args[0] as i32, args[1] as *const u8, args[2] as *const Timespec, args[3] as i32),
        SYSCALL_NEWFSTATAT      => sys_newfstatat(args[0] as i32, args[1] as *const u8, args[2] as *mut FileStat, args[3] as u32),
        SYSCALL_FSTAT           => sys_fstat(args[0] as u32, args[1] as *mut FileStat),
        SYSCALL_FSYNC           => sys_fsync(),
        SYSCALL_SET_TID_ADDRESS => sys_set_tid_address(args[0] as *mut i32),
        SYSCALL_FUTEX           => sys_futex(args[0] as *const u32, args[1] as i32, args[2], args[3], args[4] as *const u32),
        SYSCALL_SET_ROBUST_LIST => sys_set_robust_list(args[0], args[1]),
        SYSCALL_GET_ROBUST_LIST => sys_get_robust_list(args[0] as i32, args[1], args[2]),
        SYSCALL_EXIT            => sys_exit(args[0] as i32),
        SYSCALL_EXIT_GROUP      => sys_exit_group(args[0] as i32),
        SYSCALL_NANOSLEEP       => sys_nanosleep(args[0] as *const Timespec, args[1] as *mut Timespec),
        SYSCALL_SETITIMER       => sys_setitimer(),
        SYSCALL_CLOCKGETTIME    => sys_clock_gettime(args[0] as i32, args[1] as *mut Timespec),
        SYSCALL_SYSLOG          => sys_syslog(args[0] as _, args[1] as _, args[2] as _),
        SYSCALL_SCHED_YIELD     => sys_sched_yield(),
        SYSCALL_TIMES           => sys_times(args[0] as *mut Tms),
        SYSCALL_GETPGID         => sys_getpgid(args[0] as _),
        SYSCALL_UNAME           => sys_uname(args[0] as *mut Utsname),
        SYSCALL_GETRUSAGE       => sys_getrusage(args[0] as _, args[1] as _),
        SYSCALL_UMASK           => sys_umask(args[0] as _),
        SYSCALL_GETTIMEOFDAY    => sys_gettimeofday(args[0] as *mut Timeval),
        SYSCALL_GETPID          => sys_getpid(),
        SYSCALL_GETPPID         => sys_getppid(),
        SYSCALL_GETUID          => sys_getuid(),
        SYSCALL_GETEUID         => sys_geteuid(),
        SYSCALL_GETEGID         => sys_getegid(),
        SYSCALL_GETTID          => sys_gettid(),
        SYSCALL_SOCKET          => sys_socket(args[0],args[1],args[2]),
        SYSCALL_BIND            => sys_bind(args[0],args[1],args[2]),
        SYSCALL_LISTEN          => sys_listen(args[0],args[1]),
        SYSCALL_ACCEPT          => sys_accept(args[0],args[1],args[2]),
        SYSCALL_CONNECT         => sys_connect(args[0],args[1],args[2]),
        SYSCALL_GETSOCKNAME     => sys_getsockname(args[0],args[1],args[2]),
        SYSCALL_SENDTO          => sys_sendto(args[0],args[1] as _,args[2],args[3],args[4] ,args[5]),
        SYSCALL_RECVFROM        => sys_recvfrom(args[0],args[1] as _,args[2],args[3],args[4],args[5]),
        SYSCALL_SETSOCKOPT      => sys_setsockopt(args[0],args[1],args[2],args[3] as _,args[4]),
        SYSCALL_BRK             => sys_brk(args[0] as usize),
        SYSCALL_MUNMAP          => sys_munmap(args[0] as usize,args[1] as usize),
        SYSCALL_CLONE           => sys_clone( args[0] as u32, args[1], args[2] as *mut i32, args[3] as *mut usize, args[4] as *mut i32),
        SYSCALL_EXECVE          => sys_execve(args[0] as *const u8, args[1] as *const usize, args[2] as *const usize),
        SYSCALL_MMAP            => sys_mmap(args[0],args[1],args[2] as u32,args[3] as u32,args[4] as i32,args[5]),
        SYSCALL_MSYNC           => sys_msync(),
        SYSCALL_WAIT            => sys_wait4(args[0] as i32, args[1] as *mut i32, args[2] as i32),
        SYSCALL_KILL            => sys_kill(args[0] as i32, args[1] as usize),
        SYSCALL_TKILL           => sys_tkill(args[0] as i32, args[1] as usize),
        SYSCALL_SIGRETURN       => sys_sigreturn(),
        SYSCALL_SIGPROCMASK     => sys_sigprocmask(args[0] as usize,args[1] as *mut usize,args[2] as *mut usize),
        SYSCALL_RT_SIGTIMEDWAIT => sys_sigtimedwait(),
        SYSCALL_MADVISE         => sys_madvise(args[0] as _,args[1] as _,args[2]as _),
        SYSCALL_SIGACTION       => sys_sigaction(args[0] as usize,args[1] as *mut Sigaction,args[2] as *mut Sigaction),
        SYSCALL_SYSINFO         => sys_sysinfo(args[0] as _),
        SYSCALL_PRLIMIT         => sys_prlimit(args[0] as i32, args[1] as i32, args[2] as *const Rlimit, args[3] as *mut Rlimit),
        SYSCALL_MPROTECT        => sys_mprotect(args[0] as usize,args[1] as usize,args[2] as _),
        SYSCALL_RENAMEAT2       => sys_renameat(args[0] as _, args[1] as _, args[2] as _, args[3] as _, args[4] as _),
        SYSCALL_MEMBARRIER      => sys_membarrier(args[0] as i32, args[1] as u32, args[2] as u32),
        SYSCALL_STOP            => sys_stop(),
        SYSCALL_SHUTDOWN        => sys_shutdown(),
        SYSCALL_GETPGID         => sys_getpgid(args[0] as _),
        _ => panic!("Unsupported syscall_id: {}", id),
    };

    let task = get_current_task().unwrap();
    match ret {
        Ok(r) => {
            if isstrace {info!("syscall_ret: {:x}, tid = {}", r, task.tid);}
            r
        },
        Err(err) => {
            if isstrace {warn!("syscall_ret: {:?}, tid = {}", err, task.tid);}
            -(err as isize)
        }
    }
}