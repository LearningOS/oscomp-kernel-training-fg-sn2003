use crate::utils::Error;

use super::*;
use alloc::{collections::BTreeMap, string::String, sync::Arc, vec::Vec};
use spin::{RwLock, Mutex};

// VirtDir用来表示存在与内存中的目录文件，比如：
//    /dev : 设备文件系统的根目录
//    /proc: procfs文件系统的根目录
//    /dev/misc
pub struct VirtDir {
    file: BTreeMap<String, Arc<dyn File>>,
}

impl VirtDir {
    pub fn new() -> Self {
        Self { 
            file: BTreeMap::new(),
        }
    }

    pub fn add_file(
        &mut self,
        name: String, 
        file: Arc<dyn File>) 
    {
        self.file.insert(name, file);
    }

    pub fn get_file(&self, name: &String) -> Result<Arc<dyn File>, Error> {
        self.file.get(name).map_or(
            Err(Error::ENOENT),
            |f| Ok(f.clone())
        )
    }

    pub fn dentrys(&self) -> Vec<Dentry> {
        self.file
        .iter()
        .map(|(name, f)| Dentry {
            d_ino: 0,
            d_type: f.get_type().unwrap(),
            d_name: name.clone()
        })
        .collect::<Vec<Dentry>>()
    }
}

 // VirtDirWrapper指向一个VirtDir实例，
 // 实现了File和DirFile trait.
 // 可以放入文件描述符表内
pub struct VirtDirWrapper {
    mode: FileOpenMode,
    cursor: Mutex<usize>,
    inner: Arc<RwLock<VirtDir>>,
}

impl VirtDirWrapper {
    pub fn new(dir: Arc<RwLock<VirtDir>>, mode: FileOpenMode) -> Arc<dyn File> {
        Arc::new(
            Self {
                mode: mode,
                cursor: Mutex::new(0),
                inner: dir,
            }
        )
    }
}

impl File for VirtDirWrapper {
    fn read_stat(&self) -> Result<FileStat, Error> {
        panic!()
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
    fn read(&self, _len : usize) -> Result<Vec<u8>, Error> {
        Err(Error::EPERM)
    }
    fn write(&self, _data: Vec<u8>) -> Result<usize, Error> {
        Err(Error::EPERM)
    }
    fn seek(&self, _pos : usize, _mode: SeekMode) -> Result<isize, Error> {
        Err(Error::EINVAL)
    }
}

impl DirFile for VirtDirWrapper {
    fn openat(&self, name: String, mode: FileOpenMode) -> Result<Arc<dyn File>, Error> {
        let dir = self.inner.read();
        dir.get_file(&name)
    }
    fn mknod(&self, _: String, _: FilePerm, _: FileType) -> Result<Arc<dyn File>, Error> {
        Err(Error::EPERM)
    }
    fn delete(&self, _: String) -> Result<(), Error> {
        Err(Error::EPERM)
    }
    fn getdent(&self) -> Result<Vec<Dentry>, Error> {
        let cursor = self.cursor.lock();
        if *cursor == 0 {
            return Ok(Vec::new())
        }
        let dentrys = self.inner.read().dentrys();
        *self.cursor.lock() = dentrys.len();
        Ok(dentrys)
    }
}