mod bpb;
mod dirent;
mod file;

use spin::{RwLock, Mutex};
use log::*;
use alloc::{
    sync::Arc, 
    collections::{BTreeMap, VecDeque}, 
};
use self::{
    dirent::{RawDiskDirEntry, Dirent, Attribute, DiskDirEntry},
    bpb::{BPB, EBPB}, 
    file::Fat32File,
};
use crate::config::*;
use crate::utils::{Error, Path};
use super::{get_block_cache, BlockFile, VFS, File, FSid, FileOpenMode, DirFile, Statvfs};


const CLUSTER_END: u32 = 0x0FFF_FFF8;
pub const SECTOR_SIZE: usize =  512;

pub type ClusterTable = [u32; 128];
pub type DirentTable = [RawDiskDirEntry; 16];
pub type BytesTable = [u8; 512];

/// 简化的FAT32文件系统：
/// 1，假定文件系统只有1个分区
/// 2，忽略info sector，不维护valid number of free clusters等信息
/// 3，只使用第一个FAT表，并且不维护其它的FAT表
/// 4，将BPB和EBPB中一些次要的信息忽略，比如：Volume ID, Drive description, Version等
/// 5，对一些信息进行假定：比如bytes_per_sector = 512, root_dir_cluster = 2，否则报错
/// 6, 不支持时间信息
#[allow(unused)]
pub struct FAT32FileSystem {
    id: FSid,
    mount_path: Path,
    block_file: Arc<dyn BlockFile>,
    cluster_cache: Mutex<VecDeque<u32>>,
    dirent_cache: Mutex<DirentCache>,
    root_dirent: Arc<RwLock<Dirent>>,

    //belows are meta data
    sectors_number: usize,        //totol sectors_number
    sectors_per_clusters: usize,
    
    //reserved field 
    res_sector_num: usize,
    
    //file allocation table field
    table_start_sector: usize,
    table_end_sector: usize,
    sectors_per_table: usize,     //sectors of per fail allocation table
    table_num: usize,
    
    //cluster field
    data_start_sector: usize,     //the sector number of the first cluster
}

impl FAT32FileSystem {
    #[allow(unaligned_references)]
    pub fn init(block_file: Arc<dyn BlockFile>, id: FSid, mount_path: Path) -> Arc<Self> {
        const BPB_OFFSET: usize = 0x0B;
        const EBPB_OFFSET: usize = 0x24;

        info!("FAT32 file system initing");

        let bpb = get_block_cache(0, block_file.clone())
        .read()
        .read(BPB_OFFSET, |bpb: &BPB|{
            *bpb
        });


        let ebpb = get_block_cache(0, block_file.clone())
        .read()
        .read(EBPB_OFFSET, |ebpb: &EBPB|{
            *ebpb
        });
        

        let flag = get_block_cache(0, block_file.clone())
        .read()
        .read(BLOCK_SIZE - 2, |flag: &u16|{
            *flag
        });
        
        assert_eq!(ebpb.root_dir_cluster, 2);
        assert_eq!(bpb.bytes_per_sector, 512);
        assert_eq!(flag, 0xAA55);        
        
        let data_start_sector = bpb.reserved_sectors_num as usize 
            + bpb.fat_num as usize * ebpb.sectors_per_table as usize;
        let table_end_sector = bpb.reserved_sectors_num as usize 
            + ebpb.sectors_per_table as usize;
        
        let fat32 = Self {
            id,
            mount_path,
            block_file: block_file.clone(),
            dirent_cache: Mutex::new(DirentCache::new()),
            cluster_cache: Mutex::new(VecDeque::new()),
            root_dirent: Arc::new(RwLock::new(Dirent::new())),

            sectors_number: bpb.totol_sectors_num as usize,
            sectors_per_clusters: bpb.sectors_per_cluster as usize,

            res_sector_num: bpb.reserved_sectors_num as usize,

            table_start_sector: bpb.reserved_sectors_num as usize,
            table_end_sector,
            sectors_per_table: ebpb.sectors_per_table as usize, 
            table_num: bpb.fat_num as usize,
            
            data_start_sector,
        };
        
        let arc_fat32 = Arc::new(fat32);

        /* 初始化root_dirent */
        let mut root_dirent = arc_fat32.root_dirent.write();
        root_dirent.root_init(arc_fat32.clone());
        drop(root_dirent);
        
        /* 将root_dirent 加入 dirent_cache */
        let mut cache = arc_fat32.dirent_cache.lock();
        cache.insert(2, arc_fat32.root_dirent.clone());
        drop(cache);

        arc_fat32
    }

    pub fn get_root_dirent(&self) -> Arc<RwLock<Dirent>> {
        self.root_dirent.clone()
    }

    pub fn get_root_file(&self, mode: FileOpenMode) -> Arc<dyn File> {
        Fat32File::new(self.get_root_dirent(), mode)
    }

    pub fn get_bytes_per_cluster(&self) -> usize {
        self.sectors_per_clusters * SECTOR_SIZE
    }

    //返回cluster项在FAT表的位置
    pub fn get_cluster_entry_pos(&self, cluster: u32) -> Result<(usize, usize), Error> {
        let sector = self.table_start_sector + (cluster / 128) as usize;
        if sector > self.table_end_sector {
            return Err(Error::ENFILE)
        }
        let offset = (cluster % 128) * 4;
        Ok((sector, offset as usize))
    }

    //返回cluster的第一个sector
    pub fn get_cluster_start_sector(&self, cluster: u32) -> usize {
        self.data_start_sector + (cluster - 2) as usize * self.sectors_per_clusters
    }

    pub fn alloc_free_cluster(&self) -> Result<u32, Error> {
        let cluster = self.alloc_cluster_in_cache()?;

        let (sector, offset) = self.get_cluster_entry_pos(cluster).unwrap();
        get_block_cache(sector, self.block_file.clone())
        .write()
        .modify(offset, |u: &mut u32|{
            assert!(cluster_type(*u) == ClusterType::Free);
            *u = CLUSTER_END;
        });
        Ok(cluster)
    }

    pub fn free_cluster_chain(&self, mut start: u32) -> Result<usize, Error> {
        trace!("free_cluster_chain from {}", start);

        let mut len = 0;
        loop {
            if let Ok(cluster) = self.get_next_cluster(start) {
                self.free_cluster(cluster).unwrap();
                start = cluster;
                len += 1;
            } else {
                break
            }
        }
        Ok(len)
    }

    pub fn free_cluster(&self, cluster: u32) -> Result<(), Error> {
        let (sector, offset) = self.get_cluster_entry_pos(cluster)?;
        get_block_cache(sector, self.block_file.clone())
        .write()
        .modify(offset, |u: &mut u32|{
            if cluster_type(*u) == ClusterType::Free {
                warn!("free a free cluster, might be an error");
            } 
            *u = 0;
        });
        Ok(())
    }

    // 从cluster cache中分配一个cluster
    // 如果cache为空，则扫描FAT，将空闲cluster装载到cache中
    // 目前的实现如果在块设备打开多个文件系统就会出错，需要修改一下
    // 修改：alloc_cluster_in_cache, 装载空闲cluster时，向cluster写入1，占用cluster
    // 在unmount时，把cache中未使用的cluster释放
    fn alloc_cluster_in_cache(&self) -> Result<u32, Error> {
        let mut cache = self.cluster_cache.lock();
        if let Some(cluster) = cache.pop_front() {
            return Ok(cluster);
        }

        assert!(cache.len() == 0);
        const CLUSTER_NUM: usize = 128;

        let start_sector = self.table_start_sector;
        let end_sector = self.table_end_sector;

        for sector_id in start_sector ..end_sector {
            get_block_cache(sector_id, self.block_file.clone())
            .read()
            .read(0, |table: &ClusterTable|{
                for (idx, cluster) in table.iter().enumerate() {
                    if cluster_type(*cluster) == ClusterType::Free {
                        let id = (sector_id - start_sector) * CLUSTER_NUM + idx;
                        cache.push_back(id as u32);
                    }
                }
            });

            if cache.len() > CLUSTER_CACHE_SIZE {
                break;       
            }
        }
        
        if let Some(cluster) = cache.pop_front() {
            Ok(cluster)
        } else {
            Err(Error::NOSPACE)
        }

    }

    pub fn free_cluster_num(&self) -> usize {
        let start_sector = self.table_start_sector;
        let end_sector = self.table_end_sector;
        let mut num = 0;
        for sector_id in start_sector ..end_sector - 2 {    
            get_block_cache(sector_id, self.block_file.clone())
            .read()
            .read(0, |table: &ClusterTable|{
                for (_idx, cluster) in table.iter().enumerate() {
                    if cluster_type(*cluster) == ClusterType::Free {
                        num += 1;
                    }
                }
            });
        }
        num
    }

    // set the next cluster of 'curren't to 'next'
    // 注：在调用这个函数之前，current和next必须已经被分配出去
    pub fn set_next_cluster(&self, current: u32, next: u32) {
        let (sector, offset) = self.get_cluster_entry_pos(current).unwrap();
        get_block_cache(sector, self.block_file.clone())
        .write()
        .modify(offset, |cluster: &mut u32|{
            assert!(cluster_type(*cluster) != ClusterType::Free);
            *cluster = next;
        });
    }

    pub fn get_next_cluster(&self, cluster: u32) -> Result<u32, Error> {
        let (sector, offset) = self.get_cluster_entry_pos(cluster)?;
        get_block_cache(sector, self.block_file.clone())
        .read()
        .read(offset as usize, |cluster: &u32|{
            let cluster_type = cluster_type(*cluster);
            assert!(cluster_type != ClusterType::Free);
            if cluster_type == ClusterType::End {
                Err(Error::CLUSTEREND)
            } else {
                Ok(*cluster)
            }   
        })
    }

    #[allow(unused)]
    pub fn cluster_chain_len(&self, start_cluster: u32) -> usize {
        let mut len = 1;
        let mut start_cluster = start_cluster;

        loop {
            if let Ok(cluster) =self.get_next_cluster(start_cluster) {
                start_cluster = cluster;
                len += 1
            } else {
                return len
            }
        }
    }

    #[allow(unused)]
    pub fn get_dirent(&self, cluster: u32) -> Option<Arc<RwLock<Dirent>>> {
        self.dirent_cache.lock().get(cluster)
    }

    #[allow(unused)]
    pub fn pop_dirent(&self, cluster: u32) {
        self.dirent_cache.lock().remove(cluster);
    }

    pub fn try_insert_dirent(&self, dirent: Dirent) -> Arc<RwLock<Dirent>> {
        self.dirent_cache.lock().try_insert(dirent)
    }

    #[allow(unused)]
    pub fn info_print_for_debug(&self) {
        println!("data_start_sector = {:x}", self.data_start_sector);
        println!("reser_sector = {:x}", self.res_sector_num);
        println!("table_start_sec = {:x}", self.table_start_sector);
        println!("fat_num = {:x}", self.table_num);
        println!("fat_sectors = {:x}", self.sectors_per_table);
        println!("table_end_sec = {:x}", self.table_end_sector);
    }

    #[allow(unused)]
    pub fn test(&self) {
        let root_lock = self.get_root_dirent();
        let mut attr = Attribute::new();
        attr.set_dir();
        let mut root_write = root_lock.write();
        //aaaa
        let file1_lock = root_write.create_file("aaaa", attr.into()).unwrap();
        drop(root_write);
        let mut file1_write = file1_lock.write();
        file1_write.debug_show_subdentry();

        //bbbb
        let file2_lock = file1_write.create_file("aaaa_first_dir", attr.into()).unwrap();
        let file4_lock = file1_write.create_file("aaaa_second_dir", attr.into()).unwrap();
        let file5_lock = file1_write.create_file("thirdfile.txt", attr.into()).unwrap();
        let file6_lock = file1_write.create_file("forth.txt", attr.into()).unwrap();
        file1_write.debug_show_subdentry();
        drop(file1_write);

    }
}

impl VFS for FAT32FileSystem {
    fn root_dir(&self, mode: FileOpenMode) -> Result<Arc<dyn DirFile>, Error> {
        self.get_root_file(mode).as_dir()
    }
    fn as_vfs<'a>(self: Arc<Self>) -> Arc<dyn VFS + 'a> where Self: 'a {
        self
    }
    fn mount_path(&self) -> Path {
        self.mount_path.clone()
    }
    fn link(&self, _path: Path, _dst_path: Arc<dyn File>) -> Result<Arc<dyn File>, Error> {
        todo!()
    }
    fn statvfs(&self) -> Result<Statvfs, Error> {
        /* 为了方便，就不获取空闲块s数量了 */
        let num = self.free_cluster_num();
        //let num = 1908350;
        Ok(Statvfs {
            bsize: SECTOR_SIZE,
            frsize: self.sectors_per_clusters * SECTOR_SIZE,
            blocks: self.sectors_number / self.sectors_per_clusters,
            bfree: num,
            bavail: num,
            files: num,
            ffree: num,
            favail: num,
            fsid: self.id.0,
            flag: 255,
            namemax: 255,

        })
    }

}

#[derive(Debug, PartialEq)]
pub enum ClusterType {
    Free,   //0x?0000000
    Data,   //0x?0000002 - 0x?FFFFFEF
    End,    //0x?FFFFFF8 - 0x?FFFFFFF
    Other,  //reserved / unuesd / bad
}

pub fn cluster_type(cluster: u32) -> ClusterType {
    let cluster = cluster & 0x0fff_ffff;
    if cluster == 0 {
        ClusterType::Free
    } else if cluster >= 0x0000_0002 && cluster <= 0x0FFF_FFEF {
        ClusterType::Data
    } else if cluster >= 0x0FFF_FFF8 && cluster <= 0x0FFF_FFFF {
        ClusterType::End
    } else {
        ClusterType::Other
    }
}

// todo: 暂时先这么实现，后面可以换成BlockCache那样，限制打开文件的个数，并使用LRU算法
pub struct DirentCache(BTreeMap<u32, Arc<RwLock<Dirent>>>);

impl DirentCache {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn get(
        &self, 
        cluster: u32,
    ) -> Option<Arc<RwLock<Dirent>>> {
        if let Some(dirent)  = self.0.get(&cluster) {
            Some(dirent.clone())
        } else {
            None
        }
    }

    pub fn insert(
        &mut self, 
        cluster: u32,
        dirent: Arc<RwLock<Dirent>>) 
    {
        let ret = self.0.insert(cluster, dirent);
        assert!(ret.is_none());
    }

    // 如果Dirent已经存在Cache中，则返回Cache中的Dirent
    // 否则将Dirent加入到Cache中，并返回
    pub fn try_insert(
        &mut self,
        dirent: Dirent,
    ) -> Arc<RwLock<Dirent>> {
        let key = dirent.start_cluster;
        if self.0.contains_key(&key) {
            return self.0.get(&key).unwrap().clone();
        } else {
            let arc = Arc::new(RwLock::new(dirent));
            self.0.insert(key, arc.clone());
            return arc;
        }
    }

    pub fn remove(&mut self, cluster: u32) {
        self.0.remove(&cluster).unwrap();
    }
}
