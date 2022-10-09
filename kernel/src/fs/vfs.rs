use core::{any::Any, sync::atomic::{AtomicUsize, Ordering}};
use alloc::{sync::Arc};
use super::{FileOpenMode, File, DirFile};
use crate::utils::{Error, Path};
use log::*;

pub static CURRENT_FSID: AtomicUsize =  AtomicUsize::new(0);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct FSid(pub usize);

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Statvfs {
    pub bsize:      usize,      /* Filesystem block size */
    pub frsize:     usize,      /* Fragment size */
    pub blocks:     usize,      /* Size of fs in f_frsize units */
    pub bfree:      usize,      /* Number of free blocks */
    pub bavail:     usize,      /* Number of free blocks for
                                   unprivileged users */
    pub files:      usize,      /* Number of inodes */
    pub ffree:      usize,      /* Number of free inodes */
    pub favail:     usize,      /* Number of free inodes for
                                   unprivileged users */
    pub namemax:    usize,      /* Maximum filename length */
    pub fsid:       usize,      /* Filesystem ID */
    pub flag:       usize,      /* Mount flags */
}

impl FSid {
    pub fn new() -> Self {
        let id = CURRENT_FSID.fetch_add(1, Ordering::Relaxed);
        trace!("alloc id = {}", id);
        Self(id)
    }
}



pub trait VFS: Send + Sync + Any {
    fn link(&self, _path: Path, _dst_path: Arc<dyn File>) -> Result<Arc<dyn File>, Error> {
        warn!("no implement, something went wrong");
        Err(Error::EACCES)
    }
    fn as_vfs<'a>(self: Arc<Self>) -> Arc<dyn VFS + 'a> where Self: 'a {
        panic!("no implement");
    }
    fn mount_path(&self) -> Path {
        panic!("no implement");
    }
    fn root_dir(&self, _mode: FileOpenMode) -> Result<Arc<dyn DirFile>, Error> {
        panic!("no implement");
    }
    fn statvfs(&self) -> Result<Statvfs, Error> {
        panic!("no implement");
    }
}

