use alloc::{sync::Arc, vec::Vec};

use crate::{
    fs::{File, FileOpenMode, FileIndex, FileStat, StMode, DirFile, DeviceFile},
    utils::Error,
};

pub struct Zero {
    mode: FileOpenMode,
}

impl Zero {
    pub fn new(mode: FileOpenMode) -> Arc<Self> {
        Arc::new(Self { mode })
    }
}
impl File for Zero {
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
    
    fn as_file<'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a, {
        self
    }
    
    fn get_index(&self) -> Result<FileIndex, Error> {
        Err(Error::EINDEX)
    }
    fn read(&self, len : usize) -> Result<Vec<u8>, Error> {
        let mut zero = Vec::with_capacity(len);
        for _ in 0..len {
            zero.push(0);
        }
        Ok(zero)
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
}


impl DeviceFile for Zero{

}
