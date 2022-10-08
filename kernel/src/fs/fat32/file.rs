use alloc::{sync::Arc, vec::Vec, string::String};
use spin::{RwLock, Mutex};
use log::*;

use crate::fs::{
    FileOpenMode, DirFile, File, FileStat, FilePerm, FileType, SeekMode,
    get_block_cache, FileIndex, Fileid, Dentry, StMode, PollType};
use crate::syscall::time::Timespec;
use crate::utils::mem_buffer::MemBuffer;
use crate::utils::{Error, UPSafeCell};
use super::{Dirent, DiskDirEntry, Attribute};

pub struct Fat32File {
    pub mode: FileOpenMode,     
    pub file_type: FileType,
    pub dirent: Arc<RwLock<Dirent>>,

    // 可变的数据放在inner里
    // 因为Fat32File不会在进程间共享，所以用UPSafeCell
    pub inner: Mutex<Fat32FileInner>,
}

pub struct Fat32FileInner{
    pub cursor: usize,
}

impl Fat32File {
    pub fn new(dirent: Arc<RwLock<Dirent>>, mode: FileOpenMode) -> Arc<Self> {
        let inner = Fat32FileInner {
            cursor: 0
        };

        let dirent_read = dirent.read();
        let attr = dirent_read.attribute;
        drop(dirent_read);
        
        let new = Self {
            mode,
            file_type: Attribute::from(attr).into(),
            dirent,
            inner: Mutex::new(inner),
        };

        Arc::new(new)
    }
}

impl File for Fat32File {
    //可以考虑删掉，用drop来实现相关功能
    fn get_index(&self) -> Result<FileIndex, Error> {
        let dirent_read = self.dirent.read();
        let fs_id = dirent_read.get_fs().id;
        let file_id = Fileid(dirent_read.start_cluster as usize);
        Ok(FileIndex(fs_id, file_id))
    }

    fn read(&self, len: usize) -> Result<Vec<u8>, Error> {
        assert!(self.readable());

        if self.file_type != FileType::RegularFile {
            warn!("fat32file_read: this is not a regular file");
            return Err(Error::EACCES);
        }

        let mut inner = self.inner.lock();
        let dirent = self.dirent.read();
        if dirent.is_dir() {
            return Err(Error::EPERM);
        }

        let mut data: Vec<u8> = Vec::with_capacity(len);
        data.resize(len, 0);

        let len = dirent.read_at(inner.cursor, data.as_mut_slice())?;
        inner.cursor += len;

        data.truncate(len);

        Ok(data)
    }

    fn write(&self, data: Vec<u8>) -> Result<usize, Error> {
        assert!(self.writable());
        if self.file_type != FileType::RegularFile {
            warn!("fat32file_write: this is not a regular file");
            return Err(Error::EACCES);
        }
        let mut inner = self.inner.lock();
        let mut dirent = self.dirent.write();
        if dirent.is_dir() {
            return Err(Error::EPERM);
        }
        let len = dirent.write_at(inner.cursor, data.as_slice())?;
        inner.cursor += len;
        drop(dirent);
        Ok(len)
    }

    fn read_to_buffer(&self, buf: MemBuffer) -> Result<usize, Error> {
        assert!(self.readable());
        info!("fat32file_read_buffer");
        if self.file_type != FileType::RegularFile {
            warn!("fat32file_read: this is not a regular file");
            return Err(Error::EACCES);
        }

        let mut inner = self.inner.lock();
        let dirent = self.dirent.read();
        if dirent.is_dir() {
            return Err(Error::EPERM);
        }
        let len = dirent.read_to_buffer(inner.cursor, buf)?;
        inner.cursor += len;
        Ok(len)
    }

    fn write_from_buffer(&self, buf: MemBuffer) -> Result<usize, Error> {
        assert!(self.writable());
        if self.file_type != FileType::RegularFile {
            warn!("fat32file_write: this is not a regular file");
            return Err(Error::EACCES);
        }

        let mut inner = self.inner.lock();
        let mut dirent = self.dirent.write();
        if dirent.is_dir() {
            return Err(Error::EPERM);
        }
        let len = dirent.write_from_buffer(inner.cursor, buf)?;
        inner.cursor += len;
        Ok(len)
    }

    fn readable(&self) -> bool {
        self.mode.contains(FileOpenMode::READ) || self.mode.contains(FileOpenMode::RDWR)
        || self.mode.contains(FileOpenMode::SYS) || self.mode.contains(FileOpenMode::LARGE)
    }

    fn writable(&self) -> bool {
        self.mode.contains(FileOpenMode::WRITE) || self.mode.contains(FileOpenMode::RDWR)
        || self.mode.contains(FileOpenMode::SYS) || self.mode.contains(FileOpenMode::LARGE)
    }

    fn seek(&self, pos: usize, mode: SeekMode) -> Result<isize, Error> {
        let mut inner = self.inner.lock();
    
        match mode {
            SeekMode::SET => {
                inner.cursor = pos;
                Ok(pos as isize)
            }
            SeekMode::CUR => {
                let cur = inner.cursor;
                let new = cur + pos;
                inner.cursor = new;
                Ok(new as isize)
            }
            SeekMode::END => {
                let size = self.get_size().unwrap();
                inner.cursor = size;
                Ok(size as isize)
            }
        }
    }

    fn get_size(&self) -> Result<usize, Error> {
        let dirent = self.dirent.read();
        trace!("fat32_file: get_size: {}", dirent.size);
        Ok(dirent.size)
    }

    fn write_stat(&self, stat: &FileStat) -> Result<(), Error> {
        trace!("fat32file.write_stat: stat = {:?}", stat);        
        let mut dirent = self.dirent.write();
        /* 通过write_stat，只能修改FAT32文件的时间 */
        dirent.atime = Timespec {tv_sec: stat.st_atime_sec as isize, tv_nsec: stat.st_atime_nsec as isize};
        dirent.mtime = Timespec {tv_sec: stat.st_mtime_sec as isize, tv_nsec: stat.st_mtime_nsec as isize};
        dirent.ctime = Timespec {tv_sec: stat.st_ctime_sec as isize, tv_nsec: stat.st_ctime_nsec as isize};
        Ok(())
    }

    fn read_stat(&self) -> Result<FileStat, Error> {
        let dirent = self.dirent.read();
        let fs = dirent.get_fs();
        let st_mode = match self.file_type {
            FileType::Directory => StMode::DIR,
            FileType::RegularFile => StMode::REG,
            _ => panic!()
        };
        let nlink = match dirent.delete {
            true => 0,
            false => 1
        };

        let fstat = FileStat {
            st_dev: fs.block_file.get_id() as u64,
            st_ino: dirent.start_cluster as u64,
            st_mode: st_mode as u32 | 0o7777,
            st_nlink:   nlink,  //fat32不支持
            st_uid:     0,      //fat32不支持
            st_gid:     0,      //fat32不支持
            st_size: dirent.size as _,
            st_blksize: 512,
            st_blocks: (dirent.size + 511 / 512) as u64,
            // 暂时不支持时间
            st_atime_sec : dirent.atime.tv_sec as _, 
            st_atime_nsec: dirent.atime.tv_nsec as _,  
            st_mtime_sec : dirent.mtime.tv_sec as _,  
            st_mtime_nsec: dirent.mtime.tv_nsec as _,   
            st_ctime_sec : dirent.ctime.tv_sec as _,  
            st_ctime_nsec: dirent.ctime.tv_nsec as _,
            // st_rdev: 0,
            // __pad1:0,
            // __pad2:0,
            // __pad3:0  
        };
        Ok(fstat)
    }
    
    fn poll(&self, ptype: PollType) -> Result<bool, Error> {
        match ptype {
            PollType::READ => Ok(self.readable()),
            PollType::WRITE => Ok(self.writable()),
            PollType::ERR => Ok(false)
        }
    }

    fn copy(&self) -> Arc<dyn File> {
        let new_file = Fat32File::new(self.dirent.clone(), self.mode);
        let mut inner = self.inner.lock();
        let mut new_inner = new_file.inner.lock();
        new_inner.cursor = inner.cursor;
        drop(new_inner);
        new_file
    }
    
    fn as_dir<'a>(self: Arc<Self>) -> Result<Arc<dyn DirFile + 'a>, Error> where Self: 'a {
        if self.file_type == FileType::Directory {
            Ok(self)
        } else {
            Err(Error::ENOTDIR)
        }
    }

    fn as_file<'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a {
        self
    }
}

impl DirFile for Fat32File {
    // 可能需要创建文件 ques：那不就跟mknod作用重叠了吗
    fn openat(&self, name: String, mode: FileOpenMode) -> Result<Arc<dyn File>, Error> {
        let dirent = self.dirent.read();
        let open_dirent = dirent.open_at(&name)?;
        Ok(Fat32File::new(open_dirent, mode))
    }

    // 不管FilePerm
    fn mknod(&self, name: String, _perm: FilePerm, file_type: FileType) -> Result<Arc<dyn File>, Error> {
        let mut attr = Attribute::new();
        match file_type {
            FileType::RegularFile => {},
            FileType::Directory => attr.set_dir(),
            FileType::LinkFile => attr.set_link(),
            _ => return Err(Error::TYPEWRONG)
        }
        let mut dirent = self.dirent.write();
        let new_dirent = dirent.create_file(name.as_str(), attr.into())?;
        Ok(Fat32File::new(new_dirent, FileOpenMode::SYS))
    }

    // deletes a name from the file system
    // If that name was the last link to a file
    // and no processes have the file open the 
    // file is deleted and the space it was 
    // using is made available for reuse.
    fn delete(&self, name: String) -> Result<(), Error> {
        let dirent = self.dirent.write();
        let sub_dirent = dirent.open_at(name.as_str())?;
        
        trace!("fat32file_delete: at dir: {}, try to delete: {}", dirent.name, name);
        if Arc::strong_count(&sub_dirent) > 3 {
            trace!("fat32file_delete: this file is opened by other process, conut = {}", 
                Arc::strong_count(&sub_dirent));
        }

        /* 在fat32中，只要删除了名字，那么文件肯定将要被删除 */
        // 在此处只做一个标记，并不释放该文件占用的空间, 再文件被drop的时候再删除
        let mut sub_dirent_lock = sub_dirent.write();
        sub_dirent_lock.delete = true;
        
        // 因为打开文件必须获取父目录的锁，而这里已经持有了父目录的锁
        // 所以能够保证其它进程不可能在此时打开要删除的文件

        /* 在父目录中删除文件的目录项 */
        // todo: 这里只删除了short entry，没有删除long entry，虽然能正确运行，需要修改
        let sector = sub_dirent_lock.sector;
        let offset = sub_dirent_lock.offset;
        get_block_cache(sector, dirent.block_file())
        .write()
        .modify(offset, |dentry: &mut DiskDirEntry|{
            dentry.set_delete();
        });
        
        Ok(())
    }

    fn getdent(&self) -> Result<Vec<Dentry>, Error> {
        trace!("fat32_dirfile_getdents");
        let dirent_read = self.dirent.read();
        /* 目前的实现要求一次性读完整个目录 */
        
        let mut inner = self.inner.lock();
        let dirents = dirent_read.get_dirents(&mut inner)?;
        drop(inner);
        
        let mut dentrys  = Vec::new();
        
        /* 将fat32的Dirent 转化成kernel通用的Dentry */
        for dirent in dirents {
            dentrys.push(
                Dentry{
                    d_ino: dirent.start_cluster as usize,
                    d_type: Attribute::from(dirent.attribute).into(),
                    d_name: dirent.name,
                }
            )
        }
        Ok(dentrys)
    }

    fn rename(&self, name: String, new_name: String) -> Result<(), Error> {
        let mut dirent = self.dirent.write();
        let oldfile = dirent.open_at(name.as_str())?;
        let attr = oldfile.read().attribute;
        let newfile = dirent.create_file(new_name.as_str(), attr)?;
        drop(oldfile);
        Ok(())
    }
}

impl Drop for Fat32File {
    // 如果dirent的引用计数为2，那么dirent只存在cache和这个将要被drop的File中
    // 可以将dirent从cache中删除, 并且释放空间
    // 判断引用计数和删除cache中的dirent时，要全程拥有cache的锁
    fn drop(&mut self) {        
        if Arc::strong_count(&self.dirent) == 2 {
            /* 将文件从cache中删除 */
            let mut dirent = self.dirent.write();
            let fs = dirent.get_fs();
            let cluster = dirent.start_cluster;
            fs.dirent_cache.lock().remove(cluster);
            /* 如果文件被通过delete从文件系统中删除了，清除文件内容并释放空间 */
            if dirent.delete == true {
                dirent.clear_at(0, usize::MAX).unwrap();
                fs.free_cluster_chain(cluster).unwrap();
            }
        } 
    }
}
