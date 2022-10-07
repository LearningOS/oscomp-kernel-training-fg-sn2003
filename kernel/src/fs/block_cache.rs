use crate::{
    config::*,
};
use super::{
    BlockFile
};
use log::*;
use spin::{RwLock, Mutex};
use alloc::sync::Arc;
use alloc::collections::{VecDeque};  
use spin::lazy::Lazy;

pub struct BlockCache {
    cache: [u8; BLOCK_SIZE],
    block_id: usize,            
    block_file: Arc<dyn BlockFile>,
    modified: bool,
}

impl BlockCache {
    pub fn new(block_id: usize, block_file: Arc<dyn BlockFile>) -> Self {
        let mut cache = [0u8; BLOCK_SIZE];
        block_file.read_block(block_id, &mut cache);
        Self {
            cache,
            block_id,
            block_file,
            modified: false
        }
    }

    pub fn get_ref<T>(&self, offset: usize) -> &T where T: Sized {
        assert!(offset + core::mem::size_of::<T>() <= BLOCK_SIZE);
        let addr = &self.cache[offset] as *const u8 as *const T;
        unsafe{&*(addr)}
    }

    pub fn get_mut<T>(&mut self, offset: usize) -> &mut T where T: Sized {
        assert!(offset + core::mem::size_of::<T>() <= BLOCK_SIZE);
        self.modified = true;
        let addr = &mut self.cache[offset] as *mut u8 as *mut T;
        unsafe{&mut *(addr)}
    }

    pub fn read<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get_ref(offset))
    }

    pub fn modify<T, V>(&mut self, offset:usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }

    #[allow(unused)]
    pub fn debug_print(&self) {
        info!("print cache: addr = {} - {}", 
            self.block_id * BLOCK_SIZE, (self.block_id + 1) * BLOCK_SIZE - 1);
        for i in 0..BLOCK_SIZE {
            if i % 16 == 0 {
                print!("{:<3x} - {:<3x}: ", i, i+ 15);
            }
            print!("{:<3x} ", self.cache[i]);
            if i % 16 == 15 {
                println!("");
            }
        }
    }

    //sync方法仅仅会在BlockCache被drop时被调用
    pub fn sync(&mut self) {
        if self.modified {
            self.modified = false;
            self.block_file.write_block(self.block_id, &self.cache);
        }
    }
}

impl Drop for BlockCache {
    fn drop(&mut self) {
        //info!("Blockcache: {} is dropped", self.block_id);
        self.sync()
    }
}

pub struct BlockCacheManager {
    //tuple = (block_id, block_dev_id, BlockCache)
    caches: VecDeque<(usize, usize, Arc<RwLock<BlockCache>>)>,
}

impl BlockCacheManager {
    pub fn new() -> Self {
        Self {
            caches: VecDeque::new(),
        }
    }

    pub fn get_block_cache(
        &mut self, 
        block_id: usize,
        block_file: Arc<dyn BlockFile>,
    ) -> Arc<RwLock<BlockCache>> {
        let find_pair = self
            .caches
            .iter()
            .find(|pair| pair.0 == block_id && pair.1 == block_file.get_id());
        
        if let Some(pair) = find_pair {
            Arc::clone(&pair.2)
        } else {
            if self.caches.len() == BLOCK_CACHE_SIZE {
                if let Some((idx, _)) = self
                .caches
                .iter()
                .enumerate()
                .find(|(_, pair)| Arc::strong_count(&pair.2) == 1) 
                {
                    self.caches.drain(idx..=idx);        
                } else {
                panic!("Run out of BlockCache");
                }
            }
            let block_cache = Arc::new(RwLock::new(BlockCache::new(
                block_id,
                Arc::clone(&block_file)
            )));
            self.caches.push_back((block_id, block_file.get_id(), Arc::clone(&block_cache)));
            block_cache
        }
    }
}

pub static BLOCK_CACHE_MANAGER: Lazy<Mutex<BlockCacheManager>> = Lazy::new(||{
    Mutex::new(BlockCacheManager::new())
});

pub fn get_block_cache (
    block_id: usize,
    block_file: Arc<dyn BlockFile>
) -> Arc<RwLock<BlockCache>> {
    BLOCK_CACHE_MANAGER
        .lock()
        .get_block_cache(block_id, block_file)
}

#[allow(unused)]
pub fn block_cache_sync_all() {
    let manager = BLOCK_CACHE_MANAGER.lock();
    for (_, _, cache) in manager.caches.iter() {
        cache.write().sync();
    }
}

