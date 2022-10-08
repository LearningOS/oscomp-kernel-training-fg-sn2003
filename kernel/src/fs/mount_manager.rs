use alloc::{
    sync::Arc, 
    collections::{BTreeMap}, 
};
use spin::RwLock;
use super::*;
use crate::config::MAX_LINK_RECURSE;
use crate::utils::{Error, Path};
use spin::lazy::Lazy;
use log::*;

pub fn build_fs(
    fstype: &str, 
    blockfile: Option<Arc<dyn BlockFile>>,
    path: Path
) -> Result<Arc<dyn VFS>, Error> {
    match fstype {
        "fat32"
        |"vfat" => {
            if blockfile.is_none() {
                return Err(Error::ENOTBLK)
            }
            let blockfile = blockfile.unwrap();
            return Ok(FAT32FileSystem::init(blockfile, FSid::new(), path));
        }
        "devfs" => {
            Ok(DevFS::init(FSid::new(), path))
        }
        "procfs" => {
            Ok(ProcFS::init(FSid::new(), path))
        }
        _ => return Err(Error::ENODEV)
    }
}

pub static MOUNT_MANAGER: Lazy<MountManager> = Lazy::new(||{
    println!("[kernel] fs: initing mountmanager, mount root_fs(FAT32) to '/'");
    MountManager {
        root_fs: FAT32FileSystem::init(
            SDA2::new(FileOpenMode::SYS), 
            FSid::new(), 
            "/".into()
        ),
        map: RwLock::new(BTreeMap::new())
    }
});


pub struct MountManager {
    pub  root_fs: Arc<dyn VFS>,
    pub  map  : RwLock<BTreeMap<FileIndex, Arc<dyn VFS>>>,
}

impl MountManager {
    fn mount(&self, path: Path, dev: Path, fstype: &str) -> Result<(), Error> {
        trace!("mount manager mount: path = {:?}, dev = {:?}, fstype = {}", path, dev, fstype);
        
        let dir = open(path.clone(), FileOpenMode::SYS)?.as_dir()?;
        let index = dir.get_index()?;
        trace!("mount manager mount: get mountpoint dir");

        let blockfile;
        if dev.len() == 0 {
            blockfile = None;
        } else {
            blockfile = Some(open(dev, FileOpenMode::SYS)?.as_block()?);
        }
        trace!("mount manager mount: get blockfile");

        let fs = build_fs(fstype, blockfile, path)?;
        trace!("mount manager mount: get fs");

        let mut map = self.map.write();
        if map.get(&index).is_none() {
            map.insert(index, fs);
            Ok(())
        } else {
            Err(Error::ESRMNT)
        }
    }

    //todo
    fn umount(&self, path: Path) -> Result<(), Error> {
        let mountpoint = open(path,FileOpenMode::READ)?;
        let index = mountpoint.get_index().unwrap();
        let mut map = self.map.write(); 
        if map.remove(&index).is_some(){
            Ok(())
        } else{
            Err(Error::ENOENT)
        }
    }

    //从根目录开始解析路径
    pub fn open(&self, path: Path, mode: FileOpenMode) -> Result<Arc<dyn File>, Error> {
        self.open_path(self.root_fs.root_dir(mode)?.as_file(), path, mode, 0)
    }
    //从指定的目录文件src开始解析路径
    pub fn open_at(&self, src: Arc<dyn File>, path: Path, mode: FileOpenMode) -> Result<Arc<dyn File>, Error> {
        self.open_path(src, path, mode, 0)
    }

    fn open_path(
        &self, 
        mut current: Arc<dyn File>, 
        mut path: Path, 
        mode: FileOpenMode, 
        recurse_count: usize
    ) -> Result<Arc<dyn File>, Error> {
        if recurse_count >= MAX_LINK_RECURSE {
            return Err(Error::EMLINK)
        }

        while !path.is_root() {
            /* 检查current是否是挂载点 */
            if let Ok(index) = current.get_index() {
                let map = self.map.read(); 
                if map.contains_key(&index){
                    let vfs = map.get(&index).unwrap();
                    current = vfs.root_dir(FileOpenMode::SYS)?.as_file();
                }
            } 
            
            if let Ok(dir) = current.clone().as_dir() {
                /* 如果current是目录，那么在目录下打开或创建指定文件 */
                let name = path.pop_front().unwrap();
                current = dir.openat(name, mode)?;
            } else if let Ok(link) = current.clone().as_link() {
                /* 如果current是软连接，那么跳转到连接指定的文件 */
                if mode.contains(FileOpenMode::NOFOLLOW) {
                    return Err(Error::ENOENT)
                }
                current = self.open_path(
                    self.root_fs.root_dir(mode)?.as_file(), 
                    link.read_link()?, 
                    mode, 
                    recurse_count + 1)?;
            } else {
                return Err(Error::ENOTDIR)
            }
        }

        if let Ok(index) = current.get_index() {
            let map = self.map.read();
            if map.contains_key(&index){
                let vfs = map.get(&index).unwrap();
                current = vfs.root_dir(FileOpenMode::SYS)?.as_file();
            }
        }

        Ok(current)
    }

    fn mknod(&self, path: Path, filetype: FileType, perm: FilePerm) -> Result<Arc<dyn File>, Error> {
        self.mknod_at(self.root_fs.root_dir(FileOpenMode::SYS)?.as_file(), path, filetype, perm)
    }

    fn mknod_at(
        &self, 
        src: Arc<dyn File>, 
        path: Path, 
        filetype: FileType, 
        perm: FilePerm
    ) -> Result<Arc<dyn File>, Error> {
        let mut current = src;
        let mut path = path.clone();
        let mut level = 0;

        trace!("mount_manager_mknod: start, path = {:?}, len = {}", path, path.len());
        loop {
            if path.len() == 0 {
                return Err(Error::EEXIST);
            } else if path.len() == 1 {
                // 创建文件并返回
                let file_name = path.pop_front().unwrap();
                trace!("mount_manager_mknod: to create file: {}", file_name);
                current = current.as_dir()?.mknod(file_name, perm, filetype)?;
                break;
            } else {
                // 递归创建中间路径的目录
                let dir_name = path.pop_front().unwrap();
                level += 1;
    
                let ret = self.open_at(
                    current.clone(), Path::from_string(dir_name.clone())?, FileOpenMode::SYS);
                    
                current = match ret {
                    // 如果中间目录已经存在，直接返回
                    Ok(file) => {
                        trace!("mount_manager_mknod: to open dir: {}, depth = {}", dir_name, level);
                        file
                    }
                    // 如果中间目录不存在，就创建
                    Err(Error::ENOENT) => {
                        trace!("mount_manager_mknod: to creat dir: {}, depth = {}", dir_name, level);
                        current.as_dir()?.mknod(dir_name, FilePerm::NONE, FileType::Directory)?
                    }
                    Err(err) => {
                        warn!("mount_manager_mknod: return err");
                        return Err(err)
                    }
                };
            }
        }
        Ok(current)
    }

    fn delete_at(&self, src: Arc<dyn File>, path: Path) -> Result<(), Error> {
        let dir = self.open_at(src, path.remove_tail(), FileOpenMode::SYS)?.as_dir()?;
        dir.delete(path.last().clone())?;
        Ok(())
    }

    #[allow(unused)]
    fn link(&self, path: Path, dst_path: Path) -> Result<Arc<dyn File>, Error> {
        let dest_file = self.open(dst_path, FileOpenMode::SYS)?;
        let dest_vfs = dest_file.vfs();
        let link_dir = self.open(path.remove_tail(), FileOpenMode::READ | FileOpenMode::WRITE)?.as_dir()?;
        let link_vfs = link_dir.vfs();
        if Arc::ptr_eq(&dest_vfs, &link_vfs) {
            link_vfs.link(path.without_prefix(&link_vfs.mount_path()),dest_file )
        } else {
            Err(Error::EXDEV)
        }
    }

    fn get_vfs(&self, path: Path) -> Result<Arc<dyn VFS>, Error> {
        let mounted_file = self.open(path, FileOpenMode::SYS)?;
        let map = self.map.read();
        let index = mounted_file.get_index()?;
        
        if let Some(vfs) = map.get(&index) {
            Ok(vfs.clone())
        } else {
            Err(Error::ENOENT)
        }
    }
}



pub fn mount(path: Path, dev: Path, fstype: &str) -> Result<(), Error> {
    MOUNT_MANAGER.mount(path, dev, fstype)
}

pub fn umount(path: Path) -> Result<(), Error> {
    MOUNT_MANAGER.umount(path)
}

pub fn open(path: Path, mode: FileOpenMode) -> Result<Arc<dyn File>, Error> {
    MOUNT_MANAGER.open(path, mode)
}

pub fn open_at(src: Arc<dyn File>, path: Path, mode: FileOpenMode) -> Result<Arc<dyn File>, Error> {
    MOUNT_MANAGER.open_at(src, path, mode)
}

pub fn mknod(path: Path, filetype: FileType, perm: FilePerm) -> Result<Arc<dyn File>, Error> {
    MOUNT_MANAGER.mknod(path, filetype, perm)
}

pub fn mknod_at(src: Arc<dyn File>, path: Path, filetype: FileType, perm: FilePerm) -> Result<Arc<dyn File>, Error> {
    MOUNT_MANAGER.mknod_at(src, path, filetype, perm)
}

pub fn delete_at(src: Arc<dyn File>, path: Path) -> Result<(), Error> {
    MOUNT_MANAGER.delete_at(src, path)
}

#[allow(unused)]
pub fn link(path: Path, dst_path: Path) -> Result<Arc<dyn File>, Error> {
    MOUNT_MANAGER.link(path, dst_path)
}

pub fn get_vfs(path: Path) -> Result<Arc<dyn VFS>, Error> {
    MOUNT_MANAGER.get_vfs(path)
}

pub fn init() -> Result<(), Error> {
    println!("[kernel] fs: initing monut_manager");
    let index = MOUNT_MANAGER.root_fs.root_dir(FileOpenMode::SYS)?.get_index()?;
    let mut map = MOUNT_MANAGER.map.write();
    map.insert(index, MOUNT_MANAGER.root_fs.clone());
    Ok(())
}
