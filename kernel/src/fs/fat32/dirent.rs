use super::{BytesTable, FAT32FileSystem, SECTOR_SIZE, DirentTable};
use crate::fs::{FileType, BlockFile, get_block_cache};
use crate::utils::Error;
use crate::utils::mem_buffer::MemBuffer;
use alloc::{
    vec::Vec,
    sync::Arc,
    string::String,
};
use spin::RwLock;
use log::*;
use crate::config::MAX_FILE_SIZE;
use crate::syscall::time::Timespec;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attribute(pub u8);

#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub struct RawDiskDirEntry(pub [u8;32]);


#[repr(packed)]
#[derive(Clone, Copy, Debug, Default)]
#[allow(unused)]
pub struct DiskDirEntry {
    name: [u8; 8],
    ext:  [u8; 3],
    attribute   :u8,
    reserved    :u8,
    omit        :u8,
    create_time :u16,
    create_date :u16,
    access_date :u16,
    cluster_high:u16,
    modify_time :u16,
    modify_date :u16,
    cluster_low :u16,
    size        :u32,      
}


#[repr(packed)]
#[derive(Clone, Copy, Debug, Default)]
#[allow(unused)]
pub struct DiskLongDirEntry {
    seq_num     :u8,
    name_1      :[u8; 10],
    attribute   :u8,        //always 0x0F
    dir_type    :u8,        //always 0x00 for VFAT LFN
    check_sum   :u8,
    name_2      :[u8; 12],
    cluster     :u16,       //always 0x0000
    name_3      :[u8; 4],
}

#[allow(unused)]
impl RawDiskDirEntry {
    pub fn new() -> Self {
        Self([0;32])
    }

    pub fn is_lfn(&self) -> bool {
        pub const ATTRIBUTE_OFFSET: usize = 0xB;
        pub const LFN: u8 = 0xf;
        Attribute::from(self.0[ATTRIBUTE_OFFSET]).is_lfn()
    }

    //是否是有效目录项（没有被删除，不是空闲）
    pub fn is_valid(&self) -> bool {
        self.is_lfn() || !(self.0[0] == 0x00 || self.0[0] == 0xE5)
    }

    pub fn is_delete(&self) -> bool {
        !self.is_lfn() && (self.0[0] == 0xE5)
    }

    pub fn is_empty(&self) -> bool {
        !self.is_lfn() && (self.0[0] == 0x00)
    }
}


#[allow(unused)]
impl DiskDirEntry {
    pub fn new() -> Self {
        let mut new: Self = Default::default();
        new.name[0] = 0xab; //将第一个字节设置为随机数，防止当成空闲页表项
        new
    }

    pub fn is_dir(&self) -> bool {
        Attribute::from(self.attribute).is_dir()
    }

    pub fn is_free(&self) -> bool {
        self.name[0] == 0xE5 || self.name[0] == 0x00
    }   

    pub fn is_delete(&self) -> bool {
        self.name[0] == 0xE5
    }

    pub fn is_empty(&self) -> bool {
        self.name[0] == 0x00
    }

    pub fn get_name(&self) -> Result<String, Error> {
        pub const SPACE: u8 = 0x20;

        let mut name = String::new();
        let mut ext =  String::new();
        match core::str::from_utf8(&self.name) {
            Ok(name1) => name += name1.trim(),
            Err(_) => return Err(Error::ESTRING),
        }

        match core::str::from_utf8(&self.ext) {
            Ok(ext1) => ext += ext1.trim(),
            Err(_) => return Err(Error::ESTRING),
        } 
        
        if ext.len() > 0 {
            name += ".";
            name += &ext;
        }
        
        Ok(name)
    }

    pub fn set_name(&mut self, base: &[u8], ext: &[u8]) {
        assert!(base.len() == 8);
        assert!(ext.len() == 3);
        self.name.copy_from_slice(base);
        self.ext.copy_from_slice(ext);
    }

    pub fn set_size(&mut self, size: u32) {
        self.size = size;
    }

    pub fn set_attr(&mut self, attr: u8) {
        self.attribute = attr;
    } 

    pub fn set_delete(&mut self) {
        *self = Default::default();
        self.name[0] = 0xE5;
    }

    // 只有在创建文件的时候才会调用
    pub fn set_cluster(&mut self, cluster: u32) {
        self.cluster_high = ((cluster & 0xFFFF0000) >> 16) as u16;
        self.cluster_low = (cluster & 0x0000FFFF) as u16;
    }

    pub fn get_cluster(&self) -> u32 {
        ((self.cluster_high as u32) << 16) + (self.cluster_low as u32)
    }   
}   


impl DiskLongDirEntry {
    const END: u8 = 0x40;

    pub fn new() -> Self {
        let mut dentry: Self = Default::default();
        dentry.attribute = 0x0F;
        dentry.dir_type = 0x00;
        dentry.cluster = 0x0000;
        dentry
    }

    pub fn is_end(&self) -> bool {
        self.seq_num & DiskLongDirEntry::END != 0
    }

    pub fn set_end(&mut self) {
        self.seq_num |= DiskLongDirEntry::END;
    }

    pub fn get_name(&self) -> Vec<u8> {
        const END: u8 = 0xff;
        let mut name = Vec::with_capacity(26);
        for c in self.name_1 {
            if c == END {
                return name;
            } else {
                name.push(c);
            }
        }

        for c in self.name_2 {
            if c == END {
                return name;
            } else {
                name.push(c);
            }
        }

        for c in self.name_3 {
            if c == END {
                return name;
            } else {
                name.push(c);
            }
        }

        return name
    }

    pub fn set_name(&mut self, name: &[u8]) {
        assert!(name.len() == 26);
        self.name_1.copy_from_slice(&name[0..10]);
        self.name_2.copy_from_slice(&name[10..22]);
        self.name_3.copy_from_slice(&name[22..26]);
    }


}

pub struct Dirent {
    pub name: String,
    pub sector: usize,              //direntry所在sector
    pub offset: usize,              //direntry在sector的offset
    pub attribute: u8,
    pub start_cluster: u32,
    pub long_direntry_num: usize,   //该dirent对应的long_name_dir_entry的数目
    pub size: usize,
    pub atime: Timespec,
    pub mtime: Timespec,
    pub ctime: Timespec,
    pub delete: bool,       
    pub cluster_list: RwLock<Vec<u32>>,  
    pub fs: Option<Arc<FAT32FileSystem>>,
    //pub path
}

// clusterlist是文件的簇缓存
// list[i]保存了文件第i个块的簇号
// list是动态生成的, 如果要获取文件的第i个簇, 先在clusterlist中寻找,
// 如果没找到，就去FAT表中遍历，并把结果写入ClusterList中。
// 然而k210只有8M的内存，如果文件过大，那么clusterlist会占用过多内存
// 所以如果list.len()过大，那么就暂停缓存。
// 而是用last记录上一次的访问的块号及其簇号
struct ClusterList {
    pub list: Vec<u32>,
    pub last: Option<(u32, u32)>,
}



impl Dirent {    
    pub fn new() -> Self {
        Self {
            name: String::new(),
            sector: 0,
            offset: 0,
            attribute: 0,      
            start_cluster: 0,
            long_direntry_num: 0,
            delete: false,
            size: 0,
            atime: Timespec::ZERO,
            mtime: Timespec::ZERO,
            ctime: Timespec::ZERO,
            cluster_list: RwLock::new(Vec::new()),
            fs: None,
        }
    } 

    pub fn root_init(&mut self, fs: Arc<FAT32FileSystem>) {
        trace!("init fat32 root dirent");
        assert!(self.sector == 0 && self.offset == 0);

        let mut root_cluster_list = Vec::new();
        root_cluster_list.push(2);

        self.fs = Some(fs);
        self.name = String::from("/");
        self.start_cluster = 2;
        self.attribute = 0x10;      //dir
        self.cluster_list.write().push(2);
        self.size = usize::MAX;
        
        let mut offset = 0;
        loop {
            if let Ok((_dirent, next_start_offset)) = self.get_dentry_by_offset(offset) {
                offset = next_start_offset;
            } else {
                break;
            }
        }
        self.size = offset;
    }

    pub fn get_fs(&self) -> Arc<FAT32FileSystem> {
        self.fs.clone().unwrap()
    }

    pub fn block_file(&self) -> Arc<dyn BlockFile> {
        self.get_fs().block_file.clone()
    }

    pub fn is_dir(&self) -> bool {
        Attribute::from(self.attribute).is_dir()
    }

    /// 返回组成一个目录项的dirent vector，以及下一次寻找的开始地址
    /// 如果找不到（比如到了文件的末尾），返回None
    /// 在fat32中，一个dirent由一个short name dirent 和 0-n个 long name dirent组成
    /// 该函数以返回目录项Vector的形式返回地址在abs_offset的dirent的所有目录项
    /// 目录项Vector中，最后一个数据一定是short name dirent
    /// abs_offset是相对于整个文件的开始的偏移地址
    /// todo: 理顺逻辑，太难看了
    fn get_raw_dentry(&self, mut abs_offset: usize) -> Result<(Vec<RawDiskDirEntry>, usize), Error> {
        const DIRENT_SIZE: usize = 0x20;
        assert!(abs_offset % DIRENT_SIZE == 0);

        let mut raw_direntry_buf: Vec<RawDiskDirEntry> = Vec::new();
        let mut is_finish: bool = false;

        //将构成完成dirent的所有shortdirentry和longdirentry加入到raw_direnty_buf
        loop {
            let (sector, mut sector_offset) = self.pos_of_offset_byte(abs_offset)?;

            get_block_cache(sector, self.get_fs().block_file.clone())
            .read()
            .read(0, |dirent_table: &DirentTable|{
                let start_idx = sector_offset / DIRENT_SIZE;

                for idx in start_idx..16 {  
                    sector_offset += 0x20;
                    //trace!("idx = {}", idx);
                    if dirent_table[idx].is_lfn() {
                        raw_direntry_buf.push(dirent_table[idx]);
                    } else if dirent_table[idx].is_delete() {
                        raw_direntry_buf.clear();      //如果找到删除的目录项，重新构建raw_direntry_buf           
                    } else if dirent_table[idx].is_empty() {
                        raw_direntry_buf.clear();      //如果找到空闲目录项，即到了文件末尾，终止
                        is_finish = true;
                        break;
                    } else {
                        raw_direntry_buf.push(dirent_table[idx]);
                        is_finish = true;
                        break;
                    }
                }
            });

            if is_finish == true {
                if raw_direntry_buf.len() == 0 {
                    return Err(Error::DENTRYEND);
                } else {
                    let sector_start_offsrt = (abs_offset / SECTOR_SIZE) * SECTOR_SIZE;
                    return Ok((raw_direntry_buf, sector_start_offsrt + sector_offset));
                }
            }

            //将abs_offset设置为下一个sector的起始位置
            abs_offset = ((abs_offset / SECTOR_SIZE) + 1) * SECTOR_SIZE;
        }
    }

    /// 如果name符合8.3filename要求，就返回一个short name dentry
    /// 否则将name拆分到多个long name dentry里, 返回多个long name dentry 和 1个short name dentry
    /// todo: long dentry的seq字段需要符合FAT32要求（不是很重要）
    fn create_raw_dentry(&self, name: &str, cluster: u32, attr: u8) -> Result<Vec<RawDiskDirEntry>, Error> {
        let mut dentrys = Vec::new();
        
        let mut short_dentry = DiskDirEntry::new();
        short_dentry.set_cluster(cluster);
        short_dentry.set_attr(attr);

        if let Some((base, ext)) = split_shortname(name) {
            trace!("{} is splited into {} and {}", name, base, ext);

            short_dentry.set_name(base.as_bytes(), ext.as_bytes());

            let raw_short_dentry = 
                unsafe{*(&short_dentry as *const DiskDirEntry as *const RawDiskDirEntry)};
            //info!("{:?}", raw_short_dentry);
            dentrys.push(raw_short_dentry);
        } else {
            let mut names = split_longname(name);
            let len = names.len();

            trace!("{} is splited into a vec, len = {}", name, len);
            
            for i in 0..len {
                let name = names.pop().unwrap();
                let mut long_dentry = DiskLongDirEntry::new();
                long_dentry.set_name(name.as_slice());
                if i == 0 {
                    long_dentry.set_end();
                }
                let raw_long_dentry = 
                    unsafe{*(&long_dentry as *const DiskLongDirEntry as *const RawDiskDirEntry)};
                dentrys.push(raw_long_dentry);
            }

            let raw_short_dentry = 
                unsafe{*(&short_dentry as *const DiskDirEntry as *const RawDiskDirEntry)};
            dentrys.push(raw_short_dentry);
        }
        
        Ok(dentrys)
    }

    //将raw_dentry组写入目录文件，返回写入的地址
    fn write_raw_dentry(
        &mut self, 
        dentrys: Vec<RawDiskDirEntry>,
    ) -> Result<usize, Error> {
        let offset = self.find_empty_slot(dentrys.len())?;
        let mut write_offset = offset;
    
        for dentry in dentrys {
            //todo: 如果中途出错了，可以把已经写入了清除（不着急）
            self.write_at(write_offset, &dentry.0)?;
            write_offset += 0x20;
        }
        Ok(offset)
    }

    fn get_dentry_by_offset(&self, abs_offset: usize) -> Result<(Dirent, usize), Error> {
        
        let (mut raw_dentry_vec, next_start_offset) = self.get_raw_dentry(abs_offset)?;
        //info!("ret = {:?}", raw_dentry_vec);
        
        let len = raw_dentry_vec.len();
        assert!(len > 0);

        /*从raw_dentry_vec中构造Dirent的名字*/
        let mut name = String::new();
        let mut long_name = Vec::new();

        let raw_short_dentry = raw_dentry_vec.pop().unwrap();
        let short_dentry = 
            unsafe{&*(&raw_short_dentry as * const RawDiskDirEntry as *const DiskDirEntry)};
        assert!(!raw_short_dentry.is_lfn());
    
        //如果vec长度为1，那么名字仅由一个short_name_dentry构成。否则由多个long_name_dentry构成
        //println!("len = {}", len);
        if len == 1 {
           name.push_str(short_dentry.get_name()?.as_str());
        } else {
            //提取long_name_dentry中的名字，存储在long_name中
            loop {
                let raw_long_dentry = raw_dentry_vec.pop().unwrap();
                assert!(raw_long_dentry.is_lfn());
                let long_dentry = 
                    unsafe{&*(&raw_long_dentry as * const RawDiskDirEntry as *const DiskLongDirEntry)};
                long_name.append(&mut long_dentry.get_name());
                if long_dentry.is_end() {
                    //assert!(raw_dentry_vec.len() == 0);
                    break;
                }
            }

            //将utf-16的long_name转化成String
            let u16_len = long_name.len() / 2;
            let mut temp =  Vec::new();
            for i in 0..u16_len {
                let high = long_name[2 * i + 1] as u16;
                let low = long_name[2 * i] as u16;
                let ch = (high << 8) | low;
                if ch == 0 {
                    break;
                }
                temp.push(ch);
            }
            name.push_str(&String::from_utf16(&temp).unwrap_or_default());
        }

        /* 构造名字完毕 */
        let short_dentry_offset = next_start_offset - 0x20;    //short_dentry的起始地址
        let (sector, offset) = self.pos_of_offset_byte(short_dentry_offset).unwrap();
        
        let start_cluster = short_dentry.get_cluster();
        let mut cluster_list = Vec::new();
        cluster_list.push(start_cluster);    
        let dirent = Dirent {
            name,
            sector,
            offset,
            attribute: short_dentry.attribute,
            start_cluster: start_cluster,
            long_direntry_num: (len - 1) as usize,
            size: short_dentry.size as usize,
            atime: Timespec::ZERO,
            mtime: Timespec::ZERO,
            ctime: Timespec::ZERO,
            delete: false,
            cluster_list: RwLock::new(cluster_list),
            fs: Some(self.fs.clone().unwrap())
        };
        
        Ok((dirent, next_start_offset))
    }

    pub fn open_at(&self, name: &str) -> Result<Arc<RwLock<Dirent>>, Error> {
        trace!("serch_entry_by_name: from: {}, find = {}", self.name, name);
        //println!("serch_entry_by_name: from: {}, find = {}", self.name, name);
        assert!(self.is_dir());
        let mut offset = 0;

        loop {
            if let Ok((dirent, next_start_offset)) = self.get_dentry_by_offset(offset) {
                if dirent.name == name {
                    return Ok(self.get_fs().try_insert_dirent(dirent))
                } 
                offset = next_start_offset;
            } else {
                break;
            }
        }
        Err(Error::ENOENT)
    }

    pub fn is_contain(&self, name: &str) -> bool {
        assert!(self.is_dir());
        let mut offset = 0;

        loop {
            if let Ok((dirent, next_start_offset)) = self.get_dentry_by_offset(offset) {
                if dirent.name == name {
                    return true;
                } 
                offset = next_start_offset;
            } else {
                break;
            }
        }
        false
    }

    pub fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, Error> {
        let mut start = offset;
        let end = (offset + buf.len()).min(self.size);
        let mut read_size = 0;
        loop {
            let (sector, block_start) = self.pos_of_offset_byte(start).unwrap();
            let block_end = (block_start + end - start).min(512);
            get_block_cache(sector, self.get_fs().block_file.clone())
            .read()
            .read(0, |byte_table: &BytesTable|{
                let src = &byte_table[block_start..block_end];
                let dst = &mut buf[read_size..(read_size + block_end - block_start)];
                dst.copy_from_slice(src);
            });
            start += block_end - block_start;
            read_size += block_end - block_start;

            if start == end {
                break;
            }
        }

        Ok(read_size)
    }

    pub fn write_at(&mut self, offset: usize, buf: &[u8]) -> Result<usize, Error> {
        let mut start = offset;
        self.increase_size_to(offset + buf.len())?;

        let end = (offset + buf.len()).min(self.size);
        let mut write_size = 0;
        
        loop {
            let (sector, block_start) = self.pos_of_offset_byte(start).unwrap();
            let block_end = (block_start + end - start).min(512);

            get_block_cache(sector, self.get_fs().block_file.clone())
            .write()
            .modify(0, |byte_table: &mut BytesTable|{
                let src = &buf[write_size..(write_size + block_end - block_start)];
                let dst = &mut byte_table[block_start..block_end];
                dst.copy_from_slice(src);
            });

            start += block_end - block_start;
            write_size += block_end - block_start;

            if start == end {
                break;
            }
        }

        Ok(write_size)
    }

    pub fn read_to_buffer(&self, offset: usize, mut buf: MemBuffer) -> Result<usize, Error> {
        let mut start = offset;
        let end = (offset + buf.len()).min(self.size);
        let mut read_size = 0;
        loop {
            let (sector, block_start) = self.pos_of_offset_byte(start).unwrap();
            let block_end = (block_start + end - start).min(512);

            get_block_cache(sector, self.get_fs().block_file.clone())
            .read()
            .read(0, |byte_table: &BytesTable|{
                let src = &byte_table[block_start..block_end];
                buf.read_data_to_buffer(src);
            });

            start += block_end - block_start;
            read_size += block_end - block_start;

            if start == end {
                break;
            }
        }
        Ok(read_size)
    } 

    pub fn write_from_buffer(&mut self, offset: usize, mut buf: MemBuffer) -> Result<usize, Error> {
        let mut start = offset;
        self.increase_size_to(offset + buf.len())?;

        let end = (offset + buf.len()).min(self.size);
        let mut write_size = 0;
        
        loop {
            let (sector, block_start) = self.pos_of_offset_byte(start).unwrap();
            let block_end = (block_start + end - start).min(512);

            get_block_cache(sector, self.get_fs().block_file.clone())
            .write()
            .modify(0, |byte_table: &mut BytesTable|{
                let dst = &mut byte_table[block_start..block_end];
                buf.write_data_from_buffer(dst);
            });

            start += block_end - block_start;
            write_size += block_end - block_start;

            if start == end {
                break;
            }
        }
    
        Ok(write_size)
    }

    pub fn clear_at(&mut self, offset: usize, len: usize) -> Result<(), Error> {
        let mut start = offset;
        let end = (offset + len).min(self.size);

        loop {
            let (sector, block_start) = self.pos_of_offset_byte(start).unwrap();
            let block_end = (block_start + end - start).min(512);

            get_block_cache(sector, self.get_fs().block_file.clone())
            .write()
            .modify(0, |byte_table: &mut BytesTable|{
                byte_table[block_start..block_end].fill(0);
            });

            start += block_end - block_start;

            if start == end {
                break;
            }
        }
        Ok(())
    }

    pub fn increse_size(&mut self, size: usize) -> Result<(), Error> {
        if size <= 0 {
            return Ok(());
        }
        let cluster = self.cluster_of_offset_byte(self.size).unwrap();
        let bytes_per_cluster = self.get_fs().get_bytes_per_cluster();
        
        let origin = self.size / bytes_per_cluster + 1;
        let need = (self.size + size) / bytes_per_cluster + 1;
        
        let mut list = self.cluster_list.write();
        let mut old = cluster;
        for _ in origin..need {
            let new = self.get_fs().alloc_free_cluster()?;
            self.get_fs().set_next_cluster(old, new);
            list.push(new);
            old = new;
        }
        drop(list);

        self.set_size(self.size + size as usize);
        Ok(())
    }
    
    pub fn increase_size_to(&mut self, size: usize) -> Result<(), Error> {
        //todo: 是否需要限制文件的长度
        if size <= self.size {
            return Ok(())
        } 
        if size > MAX_FILE_SIZE {
            return Err(Error::EFBIG)
        }
        self.increse_size(size - self.size)
    }
    
    // 这是目前唯一的会改变自己的存储在磁盘上的目录项的方法
    pub fn set_size(&mut self, new_size: usize) {
        self.size = new_size;
        get_block_cache(self.sector, self.get_fs().block_file.clone())
        .write()
        .modify(self.offset, |dentry: &mut DiskDirEntry|{
            dentry.set_size(new_size as u32);
        });
    }

    // 找到目录文件中连续len个空闲目录项，返回第一个空闲目录项起始地址
    // 目前直接增大文件长度，返回位于文件末尾的空闲目录项
    // todo: 利用中间被删除的空闲目录项（不着急）
    pub fn find_empty_slot(&mut self, len: usize) -> Result<usize, Error> {
        assert!(self.size % 0x20 == 0);
        let old_size = self.size;
        self.increse_size(len * 0x20)?;
        Ok(old_size)
    }   
    
    pub fn create_file(&mut self, name: &str, attr: u8) -> Result<Arc<RwLock<Dirent>>, Error> {
        trace!("dirent.create_file: current_file = {}", self.name);
        trace!(" new_file: name = {}", name);
        
        if !self.is_dir() {
            return Err(Error::NOTDIR)
        }

        if self.is_contain(name) {
            return Err(Error::EEXIST)
        }
        
        if Attribute::from(attr).is_dir() {
            trace!("create a dir");
        }

        let cluster = self.get_fs().alloc_free_cluster()?;
        let raw_dentrys = self.create_raw_dentry(name, cluster, attr)?;
        let offset = self.write_raw_dentry(raw_dentrys)?;

        /* 得到刚刚创建的dirent,进行检查并初始化 */
        let (mut dirent, _) = self.get_dentry_by_offset(offset)?;
        assert_eq!(dirent.attribute, attr);
        assert_eq!(dirent.name, name);
        assert_eq!(dirent.size, 0);

        /* 在新目录文件中添加“.”和“..”目录项 */
        if dirent.is_dir() {
            let raw_dentrys = 
                dirent.create_raw_dentry(".", dirent.start_cluster, dirent.attribute)?;
            let offset = dirent.write_raw_dentry(raw_dentrys)?;
            assert_eq!(offset, 0); //第一个目录项必定写在目录文件的0x00处
            
            //创建“..”
            let raw_dentrys = 
                self.create_raw_dentry("..", self.start_cluster, self.attribute)?;
            let offset = dirent.write_raw_dentry(raw_dentrys)?;
            assert_eq!(offset , 0x20);
        }

        Ok(self.get_fs().try_insert_dirent(dirent))
    }

    //返回偏移地址为offset的字节的位置，返回（sector_id, offset_in_sector）
    pub fn pos_of_offset_byte(&self, offset: usize) -> Result<(usize, usize), Error> {
        let cluster = self.cluster_of_offset_byte(offset)?;
        let start_sector = self.get_fs().get_cluster_start_sector(cluster);
        let sector_offset = (offset % self.get_fs().get_bytes_per_cluster()) / 512;
        let sector = start_sector + sector_offset;
        let offset = offset % 512;
        Ok((sector, offset))
    }

    // 获取文件偏移地址为offset的字节所在的cluster号
    pub fn cluster_of_offset_byte(&self, offset: usize) -> Result<u32, Error> {
        let bytes_per_cluster = self.get_fs().get_bytes_per_cluster();
        let cluster_offset = offset / bytes_per_cluster;
        //trace!("offset = {}, cluster_offset = {}", offset, cluster_offset);
        self.get_cluster(cluster_offset)
    }

    // 获取文件第num个cluster的序号 (num从0开始数)
    pub fn get_cluster(&self, num: usize) -> Result<u32, Error> {
        let list = self.cluster_list.upgradeable_read();
        let list_len = list.len();

        if list_len > num {
            return Ok(list[num])
        }

        let mut list = list.upgrade();
        let mut start_cluster = list[list_len - 1];
        for _ in list_len..num + 1 {
            let cluster = self.get_fs().get_next_cluster(start_cluster)?;
            list.push(cluster);
            start_cluster = cluster;
        }
        Ok(start_cluster)
    }

    pub fn get_dirents(&self) -> Result<Vec<Dirent>, Error> {
        if !self.is_dir() {
            warn!("debug: not a dir");
            return Err(Error::TYPEWRONG);
        }

        let mut offset = 0;
        let mut dirents = Vec::new();
        loop {
            if let Ok((dirent, next_start_offset)) = self.get_dentry_by_offset(offset) {
                //println!("string: {}, dir = {}", dirent.name, dirent.is_dir());
                dirents.push(dirent);
                offset = next_start_offset;
            } else {
                break;
            }
        }
        Ok(dirents)
    }

    // just for debug
    #[allow(unused)]
    pub fn debug_show_subdentry(&self) {
        trace!("debug: show subdentry start");

        if !self.is_dir() {
            warn!("debug: not a dir");
            return;
        }
        let mut offset = 0;
        loop {
            if let Ok((dirent, next_start_offset)) = self.get_dentry_by_offset(offset) {
                println!("string: {}, dir = {}", dirent.name, dirent.is_dir());
                offset = next_start_offset;
            } else {
                break;
            }
        }

        trace!("debug: show subdentry end");
    }
}



impl From<u8> for Attribute {
    fn from(u: u8) -> Self {
        Self(u)
    } 
}

impl From<Attribute> for u8 {
    fn from(u: Attribute) -> u8 {
        u.0
    }
}

impl From<Attribute> for FileType {
    fn from(a: Attribute) -> FileType {
        if a.is_dir() {
            FileType::Directory
        } else if a.is_link() {
            FileType::LinkFile
        } else if !a.is_lfn() {
            FileType::RegularFile
        } else {
            panic!()
        }
    }
}


impl Attribute {
    const DIR   : u8 = 0b00010000;
    const LFN   : u8 = 0b00001111;
    const LINK  : u8 = 0b01000000;
    // dir  :0bxxx1xxxx
    // lfn  :0bxxxx1111
    // link :0bx1xxxxxx

    pub fn new() -> Self{
        Self(0)
    }

    pub fn clear(&mut self) {
        self.0 = 0
    }

    pub fn is_dir(&self) -> bool {
        (!self.is_lfn()) && (self.0 & Attribute::DIR != 0)
    }

    pub fn is_lfn(&self) -> bool {
        (self.0 & 0xf) == Attribute::LFN
    }

    pub fn is_link(&self) -> bool {
        (self.0 & Attribute::LINK) != 0
    }

    pub fn set_dir(&mut self) {
        // assert_eq!(self.0, 0);
        self.0 |= Attribute::DIR;
        //assert!(!self.is_lfn());
    }

    pub fn set_lfn(&mut self) {
        // assert_eq!(self.0, 0);
        self.0 |= Attribute::LFN;
    }

    pub fn set_link(&mut self) {
        // assert_eq!(self.0, 0);
        self.0 |= Attribute::LINK;
    }

}



//split long name into LFN vector
//name中的字符是utf-8格式，需要转换成utr-16格式
//name中的每一个字符对应两个u8
pub fn split_longname(name: &str) -> Vec<Vec<u8>> {
    const LONG_NAME_LEN: usize = 13;
    //info!("name_len = {}", name.len());
    let name = name.as_bytes();
    let len = (name.len() + LONG_NAME_LEN - 1) / LONG_NAME_LEN;
    let mut name_vec = Vec::new();    
    let end = name.len() * 2;

    //info!("len = {}", len);
    for i in 0..len {
        let mut vec = Vec::new();
        for j in i * LONG_NAME_LEN * 2 .. (i + 1) * LONG_NAME_LEN * 2 {
            let ch;
            if j >= end {
                ch = 0xff;
            } else if j % 2 == 0 {
                ch = name[j/2];
            } else {
                ch = 0;
            }
            vec.push(ch);
        }
        name_vec.push(vec);
    }

    name_vec
}

// if name is 8.3 filename, split it into (base, ext)
// otherwise, return None
pub fn split_shortname(name: &str) -> Option<(String, String)> {
    let mut base: String;
    let mut ext: String;

    if name == "." || name == ".." {
        base = String::from(name);
        ext = String::from("");
    } else {
        let idx = name.find('.');
        if let Some(idx) = idx {
            let ext_len = name.len() - idx - 1;
            if idx <= 8 && ext_len <= 3 {
                base = String::from(&name[..idx]);
                ext = String::from(&name[idx + 1..]);
            } else {
                return None;
            }
        } else {
            if name.len() < 8 {
                base = String::from(name);
                ext = String::from("");
            } else {
                return None;
            }
        }
    }

    for _ in base.len()..8 {
        base.push(' ');
    }

    for _ in ext.len()..3 {
        ext.push(' ');
    }

    Some((base, ext))
}




#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub struct TimeStamp {
    pub data    :u8,
    pub mouth   :u8,
    pub day     :u8,
    pub hour    :u8,
    pub minute  :u8,
    pub secound :u8,
}
