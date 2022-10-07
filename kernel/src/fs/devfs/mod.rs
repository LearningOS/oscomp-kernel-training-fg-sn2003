pub mod sda2;
pub mod pts;
pub mod null;
pub mod zero;
pub mod misc;

pub use sda2::SDA2;
pub use pts::PTS;
pub use null::Null;
pub use zero::Zero;
pub use misc::MiscDir;
use super::*;
use crate::utils::{Error, Path};
use alloc::{
    sync::Arc,
    string::String,
    vec::Vec,
};
use spin::{lazy::Lazy, Mutex};
use log::*;
pub struct DevFS{
    pub id: FSid,
    pub mount_path: Path,
}

impl DevFS {
    pub fn init(id: FSid, path: Path) -> Arc<Self> {
        let _result = mknod("/shm".into(), FileType::Directory, FilePerm::NONE);
        Arc::new(
            Self {
                id,
                mount_path: path,
            }
        )
    }
}

pub struct DevDir {
    cursor: Mutex<usize>,
    mode: FileOpenMode,
}

impl DevDir {
    pub fn new(mode: FileOpenMode) -> Arc<Self> {
        Arc::new(Self {
            cursor: Mutex::new(0),
            mode: mode
        })
    }
}

impl VFS for DevFS {
    fn as_vfs<'a>(self: Arc<Self>) -> Arc<dyn VFS + 'a> where Self: 'a {
        self
    }
    fn mount_path(&self) -> Path {
        self.mount_path.clone()
    }
    fn root_dir(&self, mode: FileOpenMode) -> Result<Arc<dyn DirFile>, Error> {
        Ok(DevDir::new(mode).as_dir()?)
    }
}

impl File for DevDir {
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
    fn seek(&self, _pos : usize, _mode: SeekMode) -> Result<isize, Error> {
        Ok(_pos as isize)
    }
}

impl DirFile for DevDir {
    fn openat(&self, name: String, mode: FileOpenMode) -> Result<Arc<dyn File>, Error> {
        match name.as_str() {
            "vda2"
            |"sda2" => {
                Ok(SDA2::new(mode).as_file())
            }
            "tty"
            |"pts" => {
                Ok(PTS::new(mode).as_file())
            }
            "null" => {
                Ok(Null::new(mode).as_file())
            }
            "zero" => {
                Ok(Zero::new(mode).as_file())
            }
            "shm" => {  
                open("/shm".into(), FileOpenMode::SYS)
            }
            "misc" => {
                Ok(MiscDir::new(mode).as_file())
            }
            _ => {
                Err(Error::ENODEV)
            }
        }
    }

    fn getdent(&self) -> Result<Vec<Dentry>, Error> {
        let mut cursor = self.cursor.lock();
        if *cursor != 0 {
            return Ok(Vec::new())
        }

        let mut dentrys = Vec::new();
        dentrys.push(
            Dentry {
                d_ino: 0,
                d_type: FileType::BlockDevice,
                d_name: String::from("sda2"),
            }
        );

        dentrys.push(
            Dentry {
                d_ino: 0,
                d_type: FileType::CharDevice,
                d_name: String::from("pts"),
            }
        );

        dentrys.push(
            Dentry {
                d_ino: 0,
                d_type: FileType::CharDevice,
                d_name: String::from("null"),
            }
        );

        dentrys.push(
            Dentry {
                d_ino: 0,
                d_type: FileType::CharDevice,
                d_name: String::from("zero"),
            }
        );

        dentrys.push(
            Dentry {
                d_ino: 0,
                d_type: FileType::Directory,
                d_name: String::from("misc"),
            }
        );

        *cursor = dentrys.len();
        Ok(dentrys)
    }
}