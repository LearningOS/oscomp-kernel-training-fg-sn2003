mod mounts;
mod meminfo;

pub use meminfo::*;
pub use mounts::*;
use super::*;
use crate::utils::{Error, Path};
use alloc::{string::String, sync::Arc, vec::Vec};
use log::*;
use spin::{lazy::Lazy, Mutex};

pub struct ProcFS {
    pub id: FSid,
    pub mount_path: Path,
}

impl ProcFS {
    pub fn init(id: FSid, path: Path) -> Arc<Self> {
        Arc::new(Self {
            id,
            mount_path: path,
        })
    }
}

impl VFS for ProcFS {
    fn as_vfs<'a>(self: Arc<Self>) -> Arc<dyn VFS + 'a> where Self: 'a,
    {
        self
    }
    fn mount_path(&self) -> Path {
        self.mount_path.clone()
    }
    fn root_dir(&self, mode: FileOpenMode) -> Result<Arc<dyn DirFile>, Error> {
        Ok(ProcDir::new(mode).as_dir()?)
    }
}


pub struct ProcDir {
    cursor: Mutex<usize>,
    mode: FileOpenMode,
}

impl ProcDir {
    pub fn new(mode: FileOpenMode) -> Arc<Self> {
        Arc::new(
            ProcDir { 
                cursor: Mutex::new(0), 
                mode: mode 
            }
        )
    }
}

impl File for ProcDir {
    fn read_stat(&self) -> Result<FileStat, Error> {
        let mut fstat: FileStat = Default::default();
        fstat.st_nlink = 1;
        fstat.st_mode = StMode::DIR as u32;
        Ok(fstat)
    }
    fn as_dir<'a>(self: Arc<Self>) -> Result<Arc<dyn DirFile + 'a>, Error> where Self: 'a, {
        Ok(self)
    }
    fn as_file<'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a, {
        self
    }
    fn get_index(&self) -> Result<super::FileIndex, Error> {
        Ok(FileIndex(FSid(0), Fileid(0)))
    }
}

impl DirFile for ProcDir {
    fn openat(&self, name: String, mode: FileOpenMode) -> Result<Arc<dyn File>, Error> {
        match name.as_str() {
            "mounts" => Ok(Mount::new(mode).as_file()),
            "meminfo" => Ok(MemInfo::new(mode).as_file()),
            _ => Err(Error::ENOENT),
        }
    }
    fn getdent(&self) -> Result<Vec<Dentry>, Error> {
        let mut cursor = self.cursor.lock();
        let mut dentrys = Vec::new();
        if *cursor == 0 {
            return Ok(dentrys);
        }
        dentrys.push(Dentry {
            d_ino: 0,
            d_type: FileType::RegularFile,
            d_name: String::from("mounts"),
        });
        dentrys.push(Dentry {
            d_ino: 0,
            d_type: FileType::RegularFile,
            d_name: String::from("meminfo"),
        });
        *cursor = dentrys.len();
        Ok(dentrys)
    }
}
