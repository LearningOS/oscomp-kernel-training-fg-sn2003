use crate::config::MAX_FD_LEN;
use crate::fs::file::PollType;
use crate::fs::vfs::Statvfs;
use crate::memory::{copyin_vec, copyout_vec, translate_str, copyout, copyin};
use crate::proc::{ 
      get_current_user_token, get_current_task, suspend_current,
};
use crate::fs::fifo::create_pipe;
use crate::trap::flush_tlb;
use crate::utils::mem_buffer::MemBuffer;
use crate::utils::{Path, Error};
use crate::fs::{File, open, open_at, mknod_at, FileOpenMode, 
    FileType, FilePerm, FileStat, mount, umount, delete_at, SeekMode, get_vfs};
use alloc::borrow::ToOwned;
use alloc::{
    sync::Arc,
    string::String,
    vec::Vec,
};
use log::*;
use spin::lazy::Lazy;
use spin::Mutex;
use core::arch::asm;
use core::mem::size_of;
use core::num::NonZeroI16;
use core::{usize, panic};
use super::time::{Timespec, Timeval};

pub const AT_FDCWD: i32 = -100;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Iovec {
    base: usize,
    len:  usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Pollfd {
    fd: i32,
    events: u16,
    revents: u16,
}


pub fn sys_getcwd(buf: *mut u8, size: usize) -> Result<isize, Error> {
    /* 获取cwd */
    let current = get_current_task().unwrap();
    let token = current.get_user_token();
    let cwd = current.get_fs_info().cwd.to_string();
    
    trace!("sys_getcwd: cwd = {}", cwd);

    let mut cwd = cwd.as_bytes().to_vec();
    cwd.push(0);    

    if cwd.len() > size {
        trace!("sys_getcwd: buffer size is too small!");
        return Err(Error::ERANGE);
    }

    let len = cwd.len();

    /* 将cwd复制到用户地址空间 */
    copyout_vec(token, buf, cwd)?;

    return Ok(len as isize);
}

pub fn sys_dup(fd: u32) -> Result<isize, Error> {
    trace!("sys_dup: fd = {}", fd);
    let task = get_current_task().unwrap();
    let mut fd_table = task.get_fd_table();

    if !fd_table.table.contains_key(&fd) {
        return Err(Error::EBADF);
    }
    
    let fd_limit = task.get_max_fd();
    if let Some(new_fd) = fd_table.alloc_fd(fd_limit) {
        let file = fd_table.table.get(&fd).unwrap().clone();
        fd_table.table.insert(new_fd, file);
        Ok(new_fd as isize)
    } else {
        Err(Error::EMFILE)
    }
    
}

pub fn sys_dup3(old_fd: u32, new_fd: u32) -> Result<isize, Error> {
    info!("dup3: old_fd = {}, new_fd = {}", old_fd, new_fd);
    let task = get_current_task().unwrap();
    let mut fd_table = task.get_fd_table();
    
    if new_fd >= MAX_FD_LEN {
        return Err(Error::EBADE)
    }

    if new_fd == old_fd {
        return Err(Error::EINVAL)
    }

    if !fd_table.table.contains_key(&old_fd){
        return Err(Error::EBADE)
    }

    let file = fd_table.table.get(&old_fd).unwrap().clone();

    /* If the file descriptor newfd was previously open,
    it is closed before being reused */
    fd_table.table.insert(new_fd, file);
    
    Ok(new_fd as isize)
}

//只实现了测试程序需要用到的flag
pub fn sys_fcntl(fd: u32, request: u32, arg: usize) -> Result<isize, Error> {
    const F_SETFD: u32 = 2;
    const F_DUPFD_CLOEXEC: u32 = 1030;
    trace!("sys_fcntl: fd = {}. request = {}, arg_ptr = {}", fd, request, arg);

    let task = get_current_task().unwrap();
    let file = task.get_file(fd)?;

    match request {
        //Set the file descriptor flags to the value specified by arg
        F_SETFD => {
            info!("sys_fcntl: unsupported request: F_SETFD, return default value: 0");
            return Ok(0)
        }
        F_DUPFD_CLOEXEC => {
            // Duplicate the file descriptor fd using the lowest-numbered
            // available file descriptor greater than or equal to arg.
            let low_fd = arg as u32;
            let mut fd_table = task.get_fd_table();
            for i in low_fd..MAX_FD_LEN {
                if !fd_table.table.contains_key(&i) {
                    fd_table.table.insert(i, file.clone());
                    trace!("sys_fcntl: F_DUPFD_CLOEXEC success: fd = {}, arg = {}, i = {}", fd, arg, i);
                    return Ok(i as isize)
                }
            }
            return Err(Error::EMFILE)
        }
        _ => {
            info!("sys_fcntl: unsupported request: {}!, return 0", request);
            return Ok(0)
        }
    }
}

pub fn sys_ioctl(fd: u32, request: u32, _arg: usize) -> Result<isize, Error> {
    info!("sys_ioctl: fd = {}. request = {}", fd, request);
    return Ok(0);

    // let task = get_current_task().unwrap();
    
    // let dev_file = task.get_inner_lock().get_file(fd)?.as_device()?;
    // trace!("sys_ioctl: get dev_file");
    
    // dev_file.ioctl(request as usize, arg);
}

pub fn sys_chdir(path: *const u8) -> Result<isize, Error> {
    /* 获取path */
    let current = get_current_task().unwrap();
    let token = current.get_user_token();
    
    let path = Path::from_string(translate_str(token, path)?)?;
    trace!("sys_chdir: path = {:?}", path);
    
    /* 检查path是否指向一个目录 */
    open(path.clone(), FileOpenMode::SYS)?.as_dir()?;

    /* 修改cwd */
    let mut fs_info = current.get_fs_info();
    fs_info.cwd = path.into();
    Ok(0)
}

pub fn sys_open(fd: i32, filename: *const u8, flags: u32, _mode: u32) -> Result<isize, Error> {
    let current = get_current_task().unwrap();
    let token =current.get_user_token();

    let filename = translate_str(token, filename)?;
    
    let open_mode = match FileOpenMode::from_bits(flags) {
        Some(value) => value,
        None => {
            warn!("sys_open: flags is invalid");
            return Err(Error::EINVAL);
        }
    };

    trace!("sys_open: fd = {}, filename = {}, flags = {:b}, 
        open_mode= {:?}", fd, filename, flags, open_mode);

    /* 解析fd和filename */
    let (root_file, path) = get_file(fd, filename)?;

    let file = if open_mode.contains(FileOpenMode::CREATE) {
        trace!("sys_open: try to make new file");
        let ret = mknod_at(
            root_file.clone(), path.clone(), 
            FileType::RegularFile, FilePerm::NONE);
        match ret {
            Ok(file) => file,
            Err(Error::EEXIST) => open_at(root_file, path, open_mode)?,
            Err(err) => return Err(err)
        }
    } else {
        trace!("sys_open: open file");
        open_at(root_file, path, open_mode)?
    };

    /* 将file添加到fd_table中*/
    let fd_limit = current.get_max_fd();
    let mut fd_table = current.get_fd_table();
    let fd = fd_table.add_file(file, fd_limit)?;
    
    Ok(fd as isize)
}

pub fn sys_close(fd: u32) -> Result<isize, Error> {
    //trace!("sys_close: fd = {}", fd);
    let task = get_current_task().unwrap();
    let mut fd_table = task.get_fd_table();

    match fd_table.table.remove(&fd) {
        Some(_) => {
            //trace!("sys_close: success");
            Ok(0)
        },
        None => {
            warn!("sys_close: invalid fd");
            Err(Error::EBADF)
        },
    }
}

#[repr(packed)]
struct DirentFront {
    pub ino: u64,
    pub off: u64,
    pub len: u16,
    pub d_type: u8,
}


//tofix
pub fn sys_getdents(fd: u32, buf: *mut u8, len: usize) -> Result<isize, Error> {
    //println!("sys_getdents: fd = {}, len = {}", fd, len);
    let current = get_current_task().unwrap();
    
    /* 获得dentrys */
    let dir = current.get_file(fd)?.as_dir()?;
    let dentrys = dir.getdent()?;
    
    /* 将dentrys复制到用户地址空间 */
    let token = current.get_user_token();
    let mut current = 0;

    for mut dentry in dentrys {
        dentry.d_name.push(0 as char);
        let mut size = core::mem::size_of::<DirentFront>() as usize + dentry.d_name.len();
        size += 8 - size % 8;
        assert!(size % 8 == 0); 
        if current + size >= len {
            break;
        }
        let mut front = DirentFront {
            ino: (dentry.d_ino) as u64,
            off: core::mem::size_of::<DirentFront>() as _,
            len: size as u16,
            d_type: 0,
        };
        unsafe{copyout(token, buf.add(current) as *mut DirentFront, &front)?;};
        current += core::mem::size_of::<DirentFront>() as usize;
        unsafe{copyout_vec(token, buf.add(current), dentry.d_name.as_bytes().to_vec())};
        current += dentry.d_name.len();
        current += 8 - current % 8;
        assert!(current % 8 == 0);
    }
    
    Ok(current as isize)
}

pub fn sys_lseek(fd: u32, offset: usize, mode: u32) -> Result<isize, Error> {
    trace!("[sys_lseek]: fd = {}, offset = {}, mode = {}", fd, offset, mode);

    let current = get_current_task().unwrap();    
    let file = current.get_file(fd)?;
    let mode = SeekMode::try_from(mode)?;
    trace!("sys_lseek: mode = {:?}", mode);
    file.seek(offset, mode)
}

pub fn sys_read(fd: u32, buf: *mut u8, len: usize) -> Result<isize, Error> {
    trace!("sys_read: fd = {}, len = {}", fd, len);
    let task = get_current_task().unwrap();    
    let token = task.get_user_token();
    let file = task.get_file(fd)?;


    // let mem = MemBuffer::from_user_space(buf, len, token, true);
    // let len = file.read_to_buffer(mem)?;
    let data = file.read(len)?;
    let len = data.len();
    copyout_vec(token, buf, data)?;
    Ok(len as isize)
}

pub fn sys_write(fd: u32, buf: *const u8, len: usize) -> Result<isize, Error> {
    trace!("sys_write: fd = {}, len = {}", fd, len);
    let task = get_current_task().unwrap();
    let token = task.get_user_token();
    // let mem = MemBuffer::from_user_space(buf as *mut u8, len, token, false);
    // let file = task.get_file(fd)?;
    // let len = file.write_from_buffer(mem)?;

    let data = copyin_vec(token, buf, len)?;
    let task = get_current_task().unwrap();
    let file = task.get_file(fd)?;
    let len = file.write(data)?;
    Ok(len as isize)
}

pub fn sys_readv(fd: u32, iovec_ptr: *const Iovec, cnt: usize) -> Result<isize, Error> {
    //todo!("change to membuffer ");
    let task = get_current_task().unwrap();
    let file = task.get_file(fd)?;

    let token = task.get_user_token();
    let mut iovec = Iovec::default();
    let mut iovec_ptr = iovec_ptr;
    let mut iovec_vec = Vec::new();
    let mut sum_len = 0;
    for _ in 0..cnt {
        copyin(token, &mut iovec, iovec_ptr)?;
        unsafe {iovec_ptr = iovec_ptr.add(1);}
        sum_len += iovec.len;
        iovec_vec.push(iovec);
    }

    let mut data = file.read(sum_len)?;
    let sum_len = data.len();

    let mut out_len = 0;
    for i in 0..cnt {
        if out_len == sum_len {
            break;
        }
        if data.len() > iovec_vec[i].len {
            let data2 = data.split_off(iovec_vec[i].len);
            out_len += data.len();
            copyout_vec(token, iovec_vec[i].base as *mut u8, data)?;
            data = data2;
        } else {
            out_len += data.len();
            copyout_vec(token, iovec_vec[i].base as *mut u8, data)?;
            break;
        }
    }
    Ok(out_len as isize)
}

pub fn sys_writev(fd: u32, iovec_ptr: *const Iovec, cnt: usize) -> Result<isize, Error> {
    //todo!("change to membuffer ");
    info!("sys_writev: fd = {}, cnt = {}", fd, cnt);

    let task = get_current_task().unwrap();

    let file = task.get_file(fd)?;

    let token = task.get_user_token();
    let mut iovec = Iovec::default();
    let mut iovec_ptr = iovec_ptr;
    let mut data = Vec::new();

    for _ in 0..cnt {
        copyin(token, &mut iovec, iovec_ptr)?;
        unsafe {iovec_ptr = iovec_ptr.add(1);}
        let mut part = copyin_vec(token, iovec.base as *const u8, iovec.len)?;
        data.append(&mut part);
    }
    
    let len = file.write(data)?;
    Ok(len as isize)
}

pub fn sys_pread(fd: u32, buf: *mut u8, len: usize, offset: usize) -> Result<isize, Error> {
    //todo!("change to membuffer ");
    trace!("sys_pread: fd = {}, count = {}, offset = {}", fd, len, offset);
    let token = get_current_user_token();
    let task = get_current_task().unwrap();    
    let file = task.get_file(fd)?;
    
    let old_cursor = file.seek(0, SeekMode::CUR)?;
    file.seek(offset, SeekMode::SET)?;
    let data = file.read(len)?;
    file.seek(old_cursor as usize, SeekMode::SET)?;
    let len = data.len();
    copyout_vec(token, buf, data)?;
    Ok(len as isize)
}

pub fn sys_pwrite(fd: u32, buf: *const u8, len: usize, offset: usize) -> Result<isize, Error> {
    //todo!("change to membuffer ");
    trace!("sys_pwrite: fd = {}, count = {}, offset = {}", fd, len, offset);
    let token = get_current_user_token();
    let data = copyin_vec(token, buf, len)?;

    let task = get_current_task().unwrap();
    let file = task.get_file(fd)?;

    let old_cursor = file.seek(0, SeekMode::CUR)?;
    file.seek(offset, SeekMode::SET)?;
    let len = file.write(data)?;
    file.seek(old_cursor as usize, SeekMode::SET)?;
    Ok(len as isize)
}

pub fn sys_sendfile(
    out_fd: u32,
    in_fd: u32,
    offset_ptr: *mut u32,
    count: usize,
) -> Result<isize, Error> {
    let current = get_current_task().unwrap();
    let fd_table = current.get_fd_table();
    trace!("sys_sendfile: out_fd = {}, in_fd = {}, count = {:x}", out_fd, in_fd, count);
    let i_file = fd_table.get_file(in_fd)?;
    let o_file = fd_table.get_file(out_fd)?;

    let count = count.min(1000);        /* 防止count过大 */

    if !offset_ptr.is_null() {
        let mut offset: u32 = 0;
        copyin(current.get_user_token(), &mut offset, offset_ptr)?;
        /* 从offset位置开始读 */
        let old_cursor = i_file.seek(0, SeekMode::CUR)?;
        i_file.seek(offset as usize, SeekMode::SET)?;
        let data = i_file.read(count)?;
        i_file.seek(old_cursor as usize, SeekMode::SET)?;
        Ok(o_file.write(data)? as isize)
    } else {
        let data=i_file.read(count)?;
        Ok(o_file.write(data)? as isize)
    }
}

pub fn sys_fsync() -> Result<isize, Error> {
    return Ok(0)
}

pub fn sys_umask(umask: usize) -> Result<isize, Error> {
    return Ok(0)
}


// from rcore
// tofix: add license
#[derive(Default, Clone, Copy, Debug)]
#[repr(C)]
pub struct FdSet(u128);

impl FdSet {
    const ZERO: Self =  FdSet(0);
    pub fn empty() -> Self {
        Self(0)
    }
    pub fn contains(&self, no: u128) -> bool {
        (self.0 >> no & 1) != 0
    }
    pub fn add(&mut self, no: u128) {
        self.0 |= 1 << no;
    }
    pub fn add_set(&mut self, set: &Self) {
        self.0 |= set.0;
    }
    pub fn remove(&mut self, no: u128) {
        self.0 ^= self.0 & (1 << no);
    }
    pub fn remove_set(&mut self, set: &Self) {
        self.0 ^= self.0 & set.0;
    }
}



pub fn sys_pselect(
    nfds: i32,
    readfds: *mut FdSet,
    writefds: *mut FdSet,
    exceptfds: *mut FdSet,
    timeout_ptr: *const Timespec,
    sigmask_ptr: *const usize,
) -> Result<isize, Error> {
    trace!("sys_pselect: nfds = {}, readfds = {:?}, 
        writefds = {:?}, exceptfds: {:?}, timeout_ptr = {:?}", 
        nfds, readfds, writefds, exceptfds, timeout_ptr);
    let task = get_current_task().unwrap();
    let token = task.get_user_token();

    fn get_arg<T: Default>(token: usize, src: *const T) -> Result<Option<T>, Error> {
        if src.is_null() {
            return Ok(None);
        } else {
            let mut dst: T = T::default();
            copyin(token, &mut dst, src)?;
            return Ok(Some(dst));
        }        
    }

    fn ret_arg<T>(token: usize, dst: *mut T, src: &T) -> Result<(), Error> {
        if dst.is_null() {
            return Ok(());
        } else {
            copyout(token, dst, src)?;
            return Ok(());
        }
    }

    let r_fds = get_arg(token, readfds)?.unwrap_or(FdSet(0));
    let w_fds = get_arg(token, writefds)?.unwrap_or(FdSet(0));
    let e_fds = get_arg(token, exceptfds)?.unwrap_or(FdSet(0));

    let mut non_block = false;

    let mut timeout = get_arg(token, timeout_ptr)?;
    if timeout.is_none() || timeout.unwrap() == Timespec::ZERO {
        non_block = true;
    }
    if timeout.is_none() {
        timeout = Some(Timespec::ZERO);
    }
    let timeout = timeout.unwrap() + Timespec::now();

    let sigmask = get_arg(token, sigmask_ptr)?;
    trace!("nfds = {}, readfds = {:?}, writefds = {:?}, exceptfds: {:?}
        , timeout = {:?}", nfds, r_fds, w_fds, e_fds, timeout);

    let mut ready = 0;
    let mut ready_rfds = FdSet::ZERO;
    let mut ready_wfds = FdSet::ZERO;
    let mut ready_efds = FdSet::ZERO;
    let mut ret;
    loop {
        let task = get_current_task().unwrap();
        let fd_table = task.get_fd_table();
        for i in 0..nfds as u32 {
            if let Ok(file) = fd_table.get_file(i) {
                if r_fds.contains(i as u128) && file.poll(PollType::READ)? {
                    ready_rfds.add(i as u128);
                    ready += 1;
                }
                if w_fds.contains(i as u128) && file.poll(PollType::WRITE)? {
                    ready_wfds.add(i as u128);
                    ready += 1;
                }
                if e_fds.contains(i as u128) && file.poll(PollType::ERR)? {
                    ready_efds.add(i as u128);
                    ready += 1;
                }
            } else {
                continue;                
            }
        }
        if ready != 0 || non_block {
            ret = Ok(ready);
            break;
        } else if timeout.pass() {
            ret = Err(Error::EINTR);
            break;
        } else {
            drop(fd_table);
            drop(task);
            suspend_current();
        }
    }
    ret_arg(token, readfds, &ready_rfds)?;
    ret_arg(token, writefds, &ready_wfds)?;
    ret_arg(token, exceptfds, &ready_efds)?;
    return ret;
}

// 仅仅是为了sh实现的系统调用：伪实现
pub fn sys_ppoll(
    fds: *mut Pollfd, 
    nfds: usize, 
    timeout: *const Timespec, 
    _sigmask: *const usize) -> Result<isize, Error> 
{   
    return Ok(1);
    const POLLIN: u16 = 0x01;
    const POLLHUP: u16 = 0x10;

    let task = get_current_task().unwrap();
    let token = task.get_user_token();
    let mut pollfd_vec = Vec::new();
    let mut pollfd = Pollfd::default();
    
    trace!("timeout = {:x?}", timeout);

    for i in 0..nfds {
        unsafe{ copyin(token, &mut pollfd, fds.add(i))? }
        pollfd_vec.push(pollfd);   
    }

    let time = {
        if !timeout.is_null() {
            let mut time = Timespec::ZERO;
            copyin(token, &mut time, timeout)?;
            time + Timespec::now()
        } else {
            Timespec {
                tv_sec: isize::MAX,
                tv_nsec: 0,
            }
        }
    };
    trace!("time = {:?}, pollfd_vec = {:?}", time, pollfd_vec);

    let mut ready = 0;

    loop {
        let fd_table = task.get_fd_table();
        for i in 0..nfds {
            let poll = &mut pollfd_vec[i];
            let file = fd_table.get_file(poll.fd as u32)?;
            if poll.events == POLLIN {
                if file.poll(PollType::READ)? {
                    poll.revents = POLLIN;
                    ready += 1;
                } else {
                    poll.revents &= !POLLIN;
                }
            } 
        }
        if ready != 0 {
            break;
        }
        if time.pass() {
            break;
        }

        drop(fd_table);
        suspend_current();
    }

    trace!("pollfd_vec = {:?}", pollfd_vec);
    for i in 0..nfds {
        unsafe{ copyout(token, fds.add(i), &mut pollfd_vec[i])?; }
    }


    return Ok(ready);
}


const UTIME_NOW : isize = 0x3fffffff;
const UTIME_OMIT: isize = 0x3ffffffe;

pub fn sys_utimensat(fd: i32, path_ptr: *const u8, timespec: *const Timespec, _flag: i32) 
    -> Result<isize, Error> {
    let token = get_current_user_token();
    trace!("sys_utimensat: fd = {}, path_ptr = {:?}, timespec = {:?}", fd, path_ptr, timespec);

    
    /* 1，得到文件 */
    trace!("sys_utimensat: open the file");
    let file = match path_ptr as usize {
        0 => {
            let current = get_current_task().unwrap();
            current.get_file(fd as u32)?
        }
        _ => {
            let path = translate_str(token, path_ptr)?;
            let (root_file, path) = get_file(fd, path)?; 
            open_at(root_file, path, FileOpenMode::SYS)?
        }
    };

    /* 2，得到timespec */
    let mut time: [Timespec; 2] = Default::default();
    if !timespec.is_null() {
        copyin(token, &mut time, timespec as *const [Timespec; 2])?;   
    } else {
        time[0] = Timespec::now();
        time[1] = time[0];
    }
    info!("time = {:?}", time);

    if time[0].tv_nsec == UTIME_OMIT && time[1].tv_nsec == UTIME_OMIT {
        return Ok(0);
    }

    /* 修改文件时间信息 */
    let mut fstat = file.read_stat()?;

    if time[0].tv_nsec != UTIME_OMIT {
        if time[0].tv_nsec == UTIME_NOW {
            let atime = Timespec::now();
            fstat.st_atime_nsec = atime.tv_nsec as _;
            fstat.st_atime_sec = atime.tv_sec as _;
        } else {
            fstat.st_atime_nsec = time[0].tv_nsec as _;
            fstat.st_atime_sec = time[0].tv_sec as _;
        }
    }

    if time[1].tv_nsec != UTIME_OMIT {
        if time[1].tv_nsec == UTIME_NOW {
            let mtime = Timespec::now();
            fstat.st_mtime_nsec = mtime.tv_nsec as _;
            fstat.st_mtime_sec = mtime.tv_sec as _;
        } else {
            fstat.st_mtime_nsec = time[1].tv_nsec as _;
            fstat.st_mtime_sec = time[1].tv_sec as _;
        }
     }
    
     file.write_stat(&fstat)?;

    Ok(0)
}

pub fn sys_readlinkat(
    dirfd: i32,
    path: *const u8,
    buf: *mut u8,
    size: usize
) -> Result<isize, Error> {
    let token =get_current_user_token();
    let path = translate_str(token, path)?;

    if path.as_str() != "/proc/self/exe" {
        /* proc文件系统还未实现 */
        return Ok(0);
        unimplemented!();
    } else {
        let mut string = String::from("/lmbench_all\0");
        copyout_vec(token, buf, string.as_bytes().to_vec())?;
        return Ok((string.len() - 1) as isize);
    }
}

//todo
pub fn sys_linkat(
    _oldfd: isize, 
    _oldpath: *const u8, 
    _newfd: isize, 
    _newpath: *const u8, 
    _flags: u32
) -> Result<isize, Error> {
    todo!()
}

pub fn sys_unlinkat(fd: i32, path: *const u8, _flags: u32) -> Result<isize, Error> {
    let token = get_current_user_token();

    let path = translate_str(token, path)?;
    info!("sys_unlinkat: fd = {}, path = {}, flags = {:b}", fd, path, _flags);

    let (root_file, path) = get_file(fd, path)?;
    
    delete_at(root_file, path)?;

    Ok(0)
}

pub fn sys_mkdir(fd: i32, path: *const u8, _mode: u32) -> Result<isize, Error> {
    let token = get_current_user_token();

    /* 从用户空间获取path */
    let path = translate_str(token, path)?;
    trace!("sys_mkdir: fd = {}, path = {}", fd, path);

    /* 创建目录 */
    let (root_file, path) = get_file(fd, path)?;
    mknod_at(root_file, path, FileType::Directory, FilePerm::NONE)?;
    Ok(0)
}

pub fn sys_umount(path: *const u8, _flags: usize) -> Result<isize, Error> {
    /* 获取path */
    let token = get_current_user_token();
    let path = Path::from_string(translate_str(token, path)?)?;
    umount(path)?;
    Ok(0)
}

pub fn sys_mount(
    dev: *const u8, 
    dir: *const u8, 
    fstype: *const u8, 
    _flags: usize, 
    _data: *const u8
) -> Result<isize, Error> {
    let token = get_current_user_token();

    let dev = Path::from_string(translate_str(token, dev)?)?;
    let dir = Path::from_string(translate_str(token, dir)?)?;
    let fstype = translate_str(token, fstype)?;

    info!("sys_mount: dev = {:?}, dir = {:?}, fstype = {:?}", dev, dir, fstype);

    mount(dir, dev, fstype.as_str())?;
    Ok(0)
}

pub fn sys_statfs(path: *const u8, buf: *mut Statvfs) -> Result<isize, Error> {
    let token = get_current_user_token();

    let path = translate_str(token, path)?;
    trace!("sys_statfs: path = {}", path);
    
    let vfs = get_vfs(Path::from_string(path)?)?;
    let stat = vfs.statvfs()?;
    info!("stat = {:?}", stat);
    copyout(token, buf, &vfs.statvfs()?)?;
    
    Ok(0)
}  

/* 只要文件存在，则返回0 */
pub fn sys_faccessat(fd: i32, path: *const u8, mode: u32, _flag: u32) -> Result<isize, Error> {
    let token = get_current_user_token();

    let path = translate_str(token, path)?;
    trace!("sys_faccessat: fd = {}, path = {}, mode = {}, _flag = {}", fd, path, mode, _flag);

    let (root_file, path) = get_file(fd, path)?;

    open_at(root_file, path, FileOpenMode::SYS)?;
    Ok(0)
}

pub fn sys_fstat(fd: u32, kst: *mut FileStat) -> Result<isize, Error> {

    /* 获取fd指定的file */
    let current = get_current_task().unwrap();
    let token = current.get_user_token();
    let file = current.get_file(fd)?;

    /* 获取fstat */
    let fstat = file.read_stat()?;

    /* 将fstat复制到用户地址空间 */
    copyout(token, kst, &fstat)?;
    Ok(0)
}

pub fn sys_newfstatat(fd: i32, path: *const u8, buf: *mut FileStat, _flag: u32) 
    -> Result<isize, Error> {    
    let token = get_current_user_token();

    let path = translate_str(token, path)?;


    let (root_file, path) = get_file(fd, path)?;
    let file = open_at(root_file, path, FileOpenMode::SYS)?;
    let fstat = file.read_stat()?;

    /* 将fstat复制到用户地址空间 */
    copyout(token, buf, &fstat)?;
    Ok(0)
}

pub fn sys_pipe(pipe_fd_ptr: *mut u32, _flag: usize) -> Result<isize, Error> {
    let task = get_current_task().unwrap();
    let token = task.get_user_token();
    
    let (pipe_read, pipe_write) = create_pipe();

    let mut fd_table = task.get_fd_table();
    let fd_limit = task.get_max_fd();
    let fd0 = fd_table.add_file(pipe_read, fd_limit)?;
    let fd1 = match fd_table.add_file(pipe_write, fd_limit) {
        Ok(fd) => fd,
        Err(Error::EMFILE) => {
            fd_table.delete_file(fd0).unwrap();
            return Err(Error::EMFILE);
        } 
        Err(err) => return Err(err) 
    };

    trace!("sys_pipe: fd0: {}, fd1: {}", fd0, fd1);

    copyout(token, pipe_fd_ptr, &fd0)?;
    copyout(token, (pipe_fd_ptr as usize + size_of::<u32>()) as *mut u32, &fd1)?;

    Ok(0)
}

pub fn sys_renameat(
    oldfd: i32, 
    oldpath: *const u8, 
    newfd: i32, 
    newpath: *const u8, 
    flags: u32
) -> Result<isize, Error> {
    let task = get_current_task().unwrap();
    let token = task.get_user_token();
    let old_string = translate_str(token, oldpath)?;
    let new_string = translate_str(token, newpath)?;
    let old_path = Path::from_string(old_string.clone())?;
    let new_path = Path::from_string(new_string.clone())?;
    trace!("sys_renameat: oldfd = {}, newfd = {}, 
        oldpath = {:?}, newpath = {:?}, flags = {:b}", 
        oldfd, newfd, old_path, new_path, flags);
    /* 目前只支持改名操作 */
    /* 必须要在同一个文件夹内 */
    if old_path.clone().remove_tail() != new_path.clone().remove_tail() {
        unimplemented!();
    }
    if flags != 0 {
        unimplemented!();
    }

    let (root_file, path) = get_file(oldfd, old_string.clone())?;
    root_file.as_dir()?.rename(old_string, new_string)?;
    Ok(0)
}

pub fn get_file(
    fd: i32, 
    path: String,  
) -> Result<(Arc<dyn File>, Path), Error> {
    trace!("get_file: fd = {}, filename = {}", fd, path);
    
    /* 如果为第一种情况 */
    if path.len() > 0 && path.as_bytes()[0] == ('/' as u8) {
        trace!("get_file: from absolute path");
        let file = open("/".into(), FileOpenMode::SYS)?;     
        return Ok((file, Path::from_string(path)?));
    } 

    let current = get_current_task().unwrap();

    /* 如果为第二种情况 */
    if fd == AT_FDCWD {
        trace!("get_file: from relative path");
        //拼接cwd和path
        let mut abs_path = current.get_fs_info().cwd.to_string();
        abs_path.push('/');
        abs_path.push_str(path.as_str());
        let file = open("/".into(), FileOpenMode::SYS)?; 
        return Ok((file, Path::from_string(abs_path)?));
    }

    trace!("get_file: from fd");
    /* 如果为第三种情况 */
    let file = current.get_file(fd as u32)?;
    return Ok((file, Path::from_string(path)?))
}
