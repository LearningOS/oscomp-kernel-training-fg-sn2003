use alloc::collections::VecDeque;
use alloc::{vec::Vec, sync::Arc};
use spin::Mutex;
use crate::fs::{FileIndex, Fileid, PollType};
use crate::fs::vfs::FSid;
use crate::utils::mem_buffer::MemBuffer;
use crate::{memory::copyout};
use crate::proc::get_current_user_token;
use crate::utils::Error;
use crate::proc::suspend_current;
use crate::driver::serial::STDIO;
use super::{FileOpenMode, File, FileStat, CharFile, DeviceFile, StMode};
use lazy_static::*;

pub const TIOCGWINSZ    :usize = 0x5413;
//pub const TCGETS        :usize = 0x5401;

pub struct PTS {
    pub mode: FileOpenMode
}


#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct Winsize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

impl PTS {
    pub fn new(mode: FileOpenMode) -> Arc<dyn CharFile> {
        Arc::new(
            Self {
                mode
            }
        )
    }
}

lazy_static! {
    pub static ref STDIO_BUF: Mutex<VecDeque<u8>> = {
        Mutex::new(VecDeque::new())
    };
}


impl File for PTS {
    fn read(&self, len: usize) -> Result<Vec<u8>, Error> {
        if !self.readable() {
            return Err(Error::EACCES);
        }
        let mut buf = Vec::new();
        for _ in 0..len {
            let mut c: u8;
            loop {
                if let Some(ch) = STDIO_BUF.lock().pop_front() {
                    c = ch;
                    break;
                }
                c = STDIO.lock().getchar();
                if c == 0 || c == 255 {
                    suspend_current();
                    continue;
                } else {
                    break;
                }
            }
            let ch = c as u8;
            if ch == b'\n' || ch == b'\r' {
                buf.push(b'\n');
                break
            }
            buf.push(ch);
        }
        Ok(buf)
    }

    fn write(&self, data: Vec<u8>) -> Result<usize, Error> {
        if !self.writable() {
            return Err(Error::EACCES);
        }
        //info!("write: data = {:?}", data);
        let mut stdio = STDIO.lock();
        for ch in data.iter() {
            stdio.putchar(*ch);
        }
        drop(stdio);
        Ok(data.len())
    }

    fn readable(&self) -> bool {
        self.mode.contains(FileOpenMode::READ) || self.mode.contains(FileOpenMode::RDWR)
        || self.mode.contains(FileOpenMode::SYS)
    }   
    fn writable(&self) -> bool {
        self.mode.contains(FileOpenMode::WRITE) || self.mode.contains(FileOpenMode::RDWR)
        || self.mode.contains(FileOpenMode::SYS)
    }
    fn as_file<'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a {
        self
    }
    fn as_char<'a>(self: Arc<Self>) -> Result<Arc<dyn CharFile + 'a>, Error> where Self: 'a {
        Ok(self)
    }
    fn as_device<'a>(self: Arc<Self>) -> Result<Arc<dyn DeviceFile + 'a>, Error> where Self: 'a {
        Ok(self)
    }
    fn get_index(&self) -> Result<FileIndex, Error> {
        Ok(FileIndex(FSid(0), Fileid(0)))
    }
    fn read_stat(&self) -> Result<FileStat, Error> {
        let mut fstat: FileStat = Default::default();
        fstat.st_nlink = 1;
        fstat.st_mode = StMode::CHR as u32;
        Ok(fstat)
    }
    fn as_dir<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::DirFile + 'a>, Error> where Self: 'a {
        Err(Error::ENOTDIR)
    }
    fn poll(&self, ptype: PollType) -> Result<bool, Error> {
        let ret = match ptype {
            PollType::READ => {
                let mut buf = STDIO_BUF.lock();
                if !buf.is_empty() {
                    true;
                }
                let c = STDIO.lock().getchar(); 
                if c == 0 || c == 255 {
                    false
                } else {
                    buf.push_back(c);
                    true
                }
            },
            PollType::WRITE => true,
            PollType::ERR => false
        };
        Ok(ret)
    }
}

impl DeviceFile for PTS {
    fn get_id(&self) -> usize {
        STDIO.lock().get_id()
    }

    fn ioctl(&self, request: usize, arg: usize) -> Result<isize, Error> {
        match request {
            TIOCGWINSZ => {
                let winsize = Winsize::default();
                let token = get_current_user_token();
                copyout(token, arg as *mut Winsize, &winsize)?;
                Ok(0)
            }
            _ => Ok(0)
        }
    }
}

impl CharFile for PTS {
    
}