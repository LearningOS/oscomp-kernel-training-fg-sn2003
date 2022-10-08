use crate::fs::{FileOpenMode, File, open, SeekMode};
use crate::proc::{TaskControlBlock, TASK_MANAGER, add_task};
use crate::utils::Error;
use crate::utils::allocator::{StackAllocator, RecyledAllocator};
use crate::config::*;
use alloc::vec::Vec;
use riscv::addr::page;
use spin::Mutex;
use spin::lazy::Lazy;
use alloc::sync::Arc;
use core::ops::Deref;
use log::*;

use super::{PhysPageNum, get_available_frame_num};
type SwapAllocator = StackAllocator;

pub static SWAP_ALLOCATOR: Lazy<Mutex<SwapAllocator>> = Lazy::new(||{
    let end = 10000;
    Mutex::new(SwapAllocator::new(0, end))
});

pub static SWAP_FILE: Lazy<Arc<dyn File>> = Lazy::new(||{
    open("/buf".into(), FileOpenMode::SYS).unwrap()
 });

pub struct SwapFrame {
    pub inner: Arc<SwapFrameInner>,
}

pub struct SwapFrameInner(usize);

impl SwapFrame {
    pub fn new(a: usize) -> Self {
        Self {
            inner: Arc::new(SwapFrameInner(a))
        }
    }

    pub fn pos(&self) -> usize {
        self.inner.0 * PAGE_SIZE
    }

    pub fn onwer_num(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}

impl Deref for SwapFrame {
    type Target = Arc<SwapFrameInner>;
    fn deref(&self) -> &Self::Target{
        &self.inner
    }
}

impl Clone for SwapFrame {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone()
        }
    }
}

impl Drop for SwapFrameInner {
    fn drop(&mut self) {
        info!("swapframe drop: {:x}", self.0);
        SWAP_ALLOCATOR.lock().dealloc(self.0);
    }
}

 /* 将物理页缓存到磁盘文件 */
pub fn swap_out_page(ppn: PhysPageNum, frame: &SwapFrame) {
    debug!("swap_out_page: ppn = {:x}, frame = {:x}", ppn.0, frame.0);
    SWAP_FILE.seek(frame.pos(), SeekMode::SET).unwrap();
    SWAP_FILE.write(ppn.get_byte_array().to_vec()).unwrap();
    debug!("exit");
}

/* 从磁盘文件取出内存数据 */
pub fn swap_in_page(ppn: PhysPageNum, frame: SwapFrame) {
    debug!("swap_in_page: ppn = {:x}, frame = {:x}", ppn.0, frame.0);
    SWAP_FILE.seek(frame.pos(), SeekMode::SET).unwrap();
    let data = SWAP_FILE.read(PAGE_SIZE).unwrap();
    ppn.get_byte_array().copy_from_slice(data.as_slice());
}

pub fn alloc_swap_frame() -> Option<SwapFrame> {
    SWAP_ALLOCATOR.lock().alloc().map(SwapFrame::new)
}

pub fn pick_task_to_swap() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.lock().pop_task()
}

pub fn swap_out_task() -> Result<(), Error>{
    let task = pick_task_to_swap().ok_or(Error::ENOMEM).unwrap();
    let mut memory_set = task.get_memory();
    let mut swap = task.swap.lock();
    let pagetable = &mut memory_set.pagetable;
    
    let mut vpns = Vec::new();
    for (vpn, frame) in pagetable.data_frames.iter() {
        if Arc::strong_count(frame) == 1 {
            vpns.push(*vpn);
        }
    }
    let frame_num1 = get_available_frame_num();
    for vpn in vpns {
        pagetable.swap_out(vpn, &mut swap);
    }
    let frame_num2 = get_available_frame_num();
    debug!("swap_out_task: get frame = {}", frame_num2 - frame_num1);
    drop(memory_set);
    drop(swap);
    add_task(task);
    Ok(())
}