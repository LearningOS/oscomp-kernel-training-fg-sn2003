use super::{FileOpenMode, File, BlockFile, FileStat, DeviceFile, StMode, FileIndex, Fileid, FSid};
use crate::utils::{Error};
use crate::driver::{BLOCK_DEVICE};
use alloc::{
    sync::Arc,
};
use log::*;

pub struct SDA2 {
    pub mode: FileOpenMode
}

impl SDA2 {
    pub fn new(mode: FileOpenMode) -> Arc<dyn BlockFile> {
        Arc::new(
            Self {
                mode
            }
        )
    }
}

impl File for SDA2 {
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
    fn as_block<'a>(self: Arc<Self>) -> Result<Arc<dyn BlockFile + 'a>, Error> where Self: 'a {
        Ok(self)
    }
    fn as_device<'a>(self: Arc<Self>) -> Result<Arc<dyn DeviceFile + 'a>, Error> where Self: 'a {
        Ok(self)
    }
    fn get_index(&self) -> Result<FileIndex, Error> {
        Ok(FileIndex(FSid(0), Fileid(0)))   //tofix: level 1
    }
    fn read_stat(&self) -> Result<FileStat, Error> {
        let mut fstat: FileStat = Default::default();
        fstat.st_nlink = 1;
        fstat.st_mode = StMode::BLK as u32;
        Ok(fstat)
    }
    fn as_dir<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::DirFile + 'a>, Error> where Self: 'a {
        Err(Error::ENOTDIR)
    }
}

impl DeviceFile for SDA2 {
    fn get_id(&self) -> usize {
        BLOCK_DEVICE.id.0
    }

    fn ioctl(&self, _request: usize, _arg: usize) -> Result<isize, Error> {
        panic!("no implement");
    }
}

impl BlockFile for SDA2 {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        if self.readable() {
            BLOCK_DEVICE.read_block(block_id, buf);
        } else {
            warn!("read_block: read fail");
        }
    }
    fn write_block(&self, block_id: usize, buf: &[u8]) {
        if self.writable() {
            BLOCK_DEVICE.write_block(block_id, buf);
        }else {
            warn!("write_block: write fail");
        }
    }
}