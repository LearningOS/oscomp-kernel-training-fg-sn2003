use alloc::{sync::Arc, vec::Vec};
use log::*;

use crate::{
    driver::serial::STDIO,
    fs::{BlockFile, CharFile, DeviceFile, File, FileOpenMode, FileStat, StMode},
    utils::Error,
};

pub struct Rtc {
    mode: FileOpenMode,
}

impl Rtc {
    pub fn new(mode: FileOpenMode) -> Arc<Self> {
        Arc::new(Self { mode })
    }
}

impl File for Rtc {
    fn get_index(&self) -> Result<crate::fs::FileIndex, crate::utils::Error> {
        Err(Error::EINDEX)
    }
    /// TODO:这里是zero的实现记得更改
    fn read(&self, len: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::Error> {
        let mut zero = Vec::with_capacity(len);
        for _ in 0..len {
            zero.push(0);
        }
        Ok(zero)
    }
    fn readable(&self) -> bool {
        self.mode.contains(FileOpenMode::READ)
            || self.mode.contains(FileOpenMode::RDWR)
            || self.mode.contains(FileOpenMode::SYS)
    }

    fn writable(&self) -> bool {
        self.mode.contains(FileOpenMode::WRITE)
            || self.mode.contains(FileOpenMode::RDWR)
            || self.mode.contains(FileOpenMode::SYS)
    }

    fn read_stat(&self) -> Result<crate::fs::FileStat, crate::utils::Error> {
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

impl DeviceFile for Rtc {}
