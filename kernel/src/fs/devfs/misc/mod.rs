mod rtc;

use rtc::*;
use super::*;
use crate::{
    fs::file::{FIFOFile, LinkFile},
    utils::{Error, Path},
};
use alloc::{string::String, sync::Arc, vec::Vec};
use log::*;
use spin::{lazy::Lazy, Mutex};



pub struct MiscDir {
    pub cursor: Mutex<usize>,
    pub mode: FileOpenMode,
}

impl MiscDir {
    pub fn new(mode: FileOpenMode) -> Arc<Self> {
        Arc::new(Self {
            cursor: Mutex::new(0),
            mode: mode
        })
    }}

impl File for MiscDir {
    fn read_stat(&self) -> Result<FileStat, Error> {
        let mut fstat: FileStat = Default::default();
        fstat.st_nlink = 1;
        fstat.st_mode = StMode::DIR as u32;
        Ok(fstat)
    }
    fn as_dir<'a>(self: Arc<Self>) -> Result<Arc<dyn DirFile + 'a>, Error> where Self: 'a {
        Ok(self)
    }
    fn as_file<'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a {
        self
    }
    fn get_index(&self) -> Result<super::FileIndex, Error> {
        Ok(FileIndex(FSid(0), Fileid(0)))
    }
}

impl DirFile for MiscDir {
    fn openat(&self, name: String, mode: FileOpenMode) -> Result<Arc<dyn File>, Error> {
        match name.as_str() {
            "rtc" => Ok(Rtc::new(mode).as_file()),
            _ => Err(Error::ENODEV),
        }
    }
    fn getdent(&self) -> Result<Vec<Dentry>, Error> {
        let mut cursor = self.cursor.lock();
        let mut dentrys = Vec::new();
        if *cursor == 0 {
            return Ok(dentrys)   
        }
        dentrys.push(Dentry {
            d_ino: 0,
            d_type: FileType::BlockDevice,
            d_name: String::from("rtc"),
        });
        *cursor = dentrys.len();
        Ok(dentrys)
    }
}
