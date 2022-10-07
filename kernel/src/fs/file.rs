use core::any::Any;

use super::{
    vfs::{VFS, FSid}
};
use crate::utils::{Error, Path, mem_buffer::MemBuffer};
use alloc::{string::String, sync::Arc, vec::Vec};
use log::*;

#[derive(Debug, PartialEq, Clone, Copy)]
#[allow(unused)]
pub enum FileType {
    UNKNOWN     = 0,
    FIFOFile    = 1,
    CharDevice  = 2,
    Directory   = 4,
    BlockDevice = 6,
    RegularFile = 8,
    LinkFile    = 10,
    SocketFile  = 12,
}

pub enum PollType {
    READ,
    WRITE,
    ERR,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct FileIndex(pub FSid, pub Fileid);


#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Fileid(pub usize);


pub trait File: Send + Sync {
    fn close(&self) -> Result<(), Error> {
        unimplemented!(); 
    }
    fn get_index(&self) -> Result<FileIndex, Error> {
        unimplemented!();
    }
    fn read(&self, _len : usize) -> Result<Vec<u8>, Error> {
        unimplemented!();
    }
    fn write(&self, _data: Vec<u8>) -> Result<usize, Error> {
        unimplemented!();
    }
    fn read_to_buffer(&self, mut buf: MemBuffer) -> Result<usize, Error> {
        let vec = self.read(buf.len())?;
        let len = vec.len();
        buf.read_data_to_buffer(vec.as_slice());
        Ok(len)
    }
    fn write_from_buffer(&self, buf: MemBuffer) -> Result<usize, Error> {
        let vec = buf.to_vec();
        self.write(vec)
    }
    fn readable(&self) -> bool {
        unimplemented!();
    }   
    fn writable(&self) -> bool {
        unimplemented!();
    }
    fn seek(&self, _pos : usize, _mode: SeekMode) -> Result<isize, Error> {
        unimplemented!();
    }
    fn get_size(&self) -> Result<usize, Error> {
        unimplemented!();
    }
    fn get_name(&self) -> Result<String, Error> {
        unimplemented!();
    }
    fn get_type(&self) -> Result<FileType, Error> {
        unimplemented!();
    }
    fn write_stat(&self, _stat: &FileStat) -> Result<(), Error> {
        unimplemented!();
    }
    fn read_stat(&self) -> Result<FileStat, Error> {
        unimplemented!();
    }
    fn poll(&self, ptype: PollType) -> Result<bool, Error> {
        unimplemented!();
    }
    fn copy(&self) -> Arc<dyn File> {
        unimplemented!();
    }
    fn vfs(&self) -> Arc<dyn VFS> {
        unimplemented!();
    }
    fn as_dir<'a>(self: Arc<Self>) -> Result<Arc<dyn DirFile + 'a>, Error> where Self: 'a {
        Err(Error::EPERM)
    }
    fn as_link<'a>(self: Arc<Self>) -> Result<Arc<dyn LinkFile + 'a>, Error> where Self: 'a {
        Err(Error::EPERM)
    }
    fn as_device<'a>(self: Arc<Self>) -> Result<Arc<dyn DeviceFile + 'a>, Error> where Self: 'a {
        Err(Error::EPERM)
    }
    fn as_block<'a>(self: Arc<Self>) -> Result<Arc<dyn BlockFile + 'a>, Error> where Self: 'a {
        Err(Error::EPERM)
    }
    fn as_char<'a>(self: Arc<Self>) -> Result<Arc<dyn CharFile + 'a>, Error> where Self: 'a {
        Err(Error::EPERM)
    }
    fn as_fifo<'a>(self: Arc<Self>) -> Result<Arc<dyn FIFOFile + 'a>, Error> where Self: 'a {
        Err(Error::EPERM)
    }
    fn as_file<'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a {
        unimplemented!();
    }
    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn Any + Send + Sync + 'a> where Self: 'a {
        unimplemented!();
    }
    fn as_socket<'a>(self: Arc<Self>) -> Result<Arc<dyn SocketFile + 'a>, Error> where Self: 'a, {
        Err(Error::EPERM)
    }
}

pub trait DirFile: File {
    fn openat(&self, _name: String, _mode: FileOpenMode) -> Result<Arc<dyn File>, Error> {
        unimplemented!();
    }
    fn mknod(&self, _name: String, _perm: FilePerm, _file_type: FileType) -> Result<Arc<dyn File>, Error> {
        unimplemented!();
    }
    fn delete(&self, _name: String) -> Result<(), Error> {
        unimplemented!();
    }
    fn getdent(&self) -> Result<Vec<Dentry>, Error> {
        unimplemented!();
    }
    fn rename(&self, old: String, new: String) -> Result<(), Error> {
        unimplemented!();
    }
}

pub trait DeviceFile: File {
    fn get_id(&self) -> usize {
        unimplemented!();
    }
    fn ioctl(&self, _request: usize, _arg: usize) -> Result<isize, Error> {
        unimplemented!();
    }
}

pub trait BlockFile: DeviceFile {
    fn read_block(&self, _block_id: usize, _buf: &mut [u8]) {
        unimplemented!();
    }
    fn write_block(&self, _block_id: usize, _buf: &[u8]) {
        unimplemented!();
    }
}

pub trait CharFile: DeviceFile {
    fn getchar(&mut self) -> u8 {
        unimplemented!();
    } 
    fn putchar(&mut self, _ch: u8) {
        unimplemented!();
    }
}

pub trait LinkFile     : File {
    fn read_link(&self) -> Result<Path, Error>;
    fn write_link(&self, path: &Path) -> Result<(), Error>;
}
pub trait FIFOFile     : File {}

pub trait SocketFile : File{
    fn listen(&self)                    -> Result<usize, Error>;
    fn accept(&self)                    -> Result<usize, Error>;
    fn connect(&self)                   -> Result<usize, Error>;
    fn getsockname(&self)               -> Result<usize, Error>;
    fn sendto(&self)                    -> Result<usize, Error>;
    fn recvfrom(&self,len:usize)        -> Result<Vec<u8>, Error>;
    fn setsockopt(&self)                -> Result<usize, Error>;
}

bitflags! {
    pub struct FileOpenMode: u32 {
        const READ      = 0;
        const WRITE     = 0o0000001;
        const RDWR      = 0o0000002;  
        const SYS       = 0o0000004;   // read && write && exec
        const CREATE    = 0o0000100;
        const EXCL      = 0o0000200;
        const NOCTTY    = 0o0000400;
        const TRUNC     = 0o0001000;
        const APPEND    = 0o0002000;
        const NONBLOCK  = 0o0004000;
        const LARGE     = 0o0100000;
        const DIR       = 0o0200000;
        const NOFOLLOW  = 0o0400000;
        const CLOEXEC   = 0o2000000;
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
 
#[repr(C)]
#[derive(Debug, Default)]
pub struct FileStat {
	// pub st_dev      :u64,   
    // pub st_ino      :u64,   
    // pub st_mode     :u32,   
    // pub st_nlink    :u32,   
    // pub st_uid      :u32,
    // pub st_gid      :u32,
    // pub st_rdev     :u64,
    // pub __pad1      :u32,
    // pub st_size     :u32,
    // pub st_blksize   :u32,
    // pub __pad2       :u32,
    // pub st_blocks    :u64,
    // pub st_atime_sec :u32, 
    // pub st_atime_nsec:u32,  
    // pub st_mtime_sec :u32,  
    // pub st_mtime_nsec:u32,   
    // pub st_ctime_sec :u32,  
    // pub st_ctime_nsec:u32,  
    // pub __pad3  :u16,
    pub st_dev  :u64,   /* ID of device containing file */
    //__pad1  :u32,
    pub st_ino  :u64,   /* Inode number */
    pub st_mode :u32,   /* File type and mode */
    pub st_nlink:u32,   /* Number of hard links */
    pub st_uid  :u32,
    pub st_gid  :u32,
    //st_rdev :u64,   /* Device ID (if special file) */
    //__pad2  :u32,
    pub st_blksize   :u64,    /* Block size for filesystem I/O */
    pub st_blocks    :u64,    /* Number of 512B blocks allocated */
    pub st_size  :u64,         /* Total size, in bytes */ //????????????
    pub st_atime_sec :i64,    
    pub st_atime_nsec:i64,  
    pub st_mtime_sec :i64,  
    pub st_mtime_nsec:i64,   
    pub st_ctime_sec :i64,  
    pub st_ctime_nsec:i64,  
}

// refer to man/inode
#[allow(unused)]
pub enum StMode {
    REG = 0o100000,
    BLK = 0o060000,
    DIR = 0o040000,
    CHR = 0o020000,
    IFO = 0o010000,
}

#[derive(Debug)]
pub struct Dentry {
    pub d_ino: usize,       // 8bytes
    pub d_type: FileType,   // 2bytes
    pub d_name: String,
}

#[derive(Debug)]
pub enum SeekMode {
    SET = 0,
    CUR = 1,
    END = 2,
}

impl TryFrom<u32> for SeekMode {
    type Error = Error;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(SeekMode::SET),
            1 => Ok(SeekMode::CUR),
            2 => Ok(SeekMode::END),
            _ => Err(Error::EINVAL)
        }
    }
}