use alloc::sync::Arc;
use alloc::vec::Vec;
use super::{FileOpenMode, File, FileStat, StMode, FileIndex, DeviceFile};
use crate::{utils::Error};
use log::*;
pub struct Null {
    mode: FileOpenMode
}

impl Null {
    pub fn new(mode: FileOpenMode) -> Arc<Self> {
        Arc::new(
            Self { mode }
        )
    }
}

impl File for Null {
    fn readable(&self) -> bool {
        self.mode.contains(FileOpenMode::READ) || self.mode.contains(FileOpenMode::RDWR)
        || self.mode.contains(FileOpenMode::SYS)
    }   
    fn writable(&self) -> bool {
        self.mode.contains(FileOpenMode::WRITE) || self.mode.contains(FileOpenMode::RDWR)
        || self.mode.contains(FileOpenMode::SYS)
    }
    fn get_index(&self) -> Result<FileIndex, Error> {
        Err(Error::EINDEX)
    }
    fn read(&self, _len : usize) -> Result<Vec<u8>, Error> {
        Err(Error::EINVAL)
    }
    fn write(&self, data: Vec<u8>) -> Result<usize, Error> {
        Ok(data.len())
    }
    fn read_stat(&self) -> Result<FileStat, Error> {
        let mut fstat: FileStat = Default::default();
        fstat.st_nlink = 1;
        fstat.st_mode = StMode::CHR as u32;
        Ok(fstat)
    }
    fn as_device<'a>(self: Arc<Self>) -> Result<Arc<dyn DeviceFile + 'a>, Error> where Self: 'a {
        Ok(self)
    }
    fn as_file<'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a {
        self
    }
}

impl DeviceFile for Null {}