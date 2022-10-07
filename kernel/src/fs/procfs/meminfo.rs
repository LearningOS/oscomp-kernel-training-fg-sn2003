use alloc::{sync::Arc, vec::Vec};
use super::*;

pub struct MemInfo {
    mode: FileOpenMode,
}

impl MemInfo {
    pub fn new(mode: FileOpenMode) -> Arc<Self> {
        Arc::new(Self { mode })
    }
}


impl File for MemInfo {
    fn get_index(&self) -> Result<FileIndex, crate::utils::Error> {
        Err(Error::EINDEX)
    }

    fn read(&self, len: usize) -> Result<Vec<u8>, Error> {
        Ok(Vec::new())
    }

    fn read_stat(&self) -> Result<FileStat, Error> {
        let mut fstat: FileStat = Default::default();
        fstat.st_nlink = 1;
        fstat.st_mode = StMode::REG as u32;
        Ok(fstat)
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

    fn as_file<'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a, {
        self
    }
}
