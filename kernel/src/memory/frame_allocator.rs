use alloc::vec::Vec;
use spin::lazy::Lazy;
use spin::Mutex;
use core::fmt::{self, Debug, Formatter};
use alloc::sync::Arc;
use core::ops::Deref;
use crate::config::{MEMORY_END};
use super::{PhysAddr, PhysPageNum};
use core::mem::drop;
use crate::utils::allocator::{StackAllocator, RecyledAllocator};

type FrameAllocator = StackAllocator;

pub static FRAME_ALLOCATOR: Lazy<Mutex<FrameAllocator>> = Lazy::new(||{
    extern "C" {
        fn ekernel();
    }
    Mutex::new(FrameAllocator::new(
        PhysAddr::from(ekernel as usize).ceil_page_num().into(),
        PhysAddr::from(MEMORY_END).floor_page_num().into()
        )
    )
});

pub struct FrameTracker {
    pub inner: Arc<FrameTrackerInner>,
}

pub struct FrameTrackerInner{
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    pub fn new(ppn: usize) -> Self {
        let ppn: PhysPageNum = ppn.into();
        ppn.get_byte_array().fill(0);
        Self {
            inner:Arc::new(
                FrameTrackerInner {ppn}
            )
        }
    }

    pub fn is_only_owner(&self) -> bool {
        self.onwer_num() == 1
    }

    pub fn onwer_num(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}

impl Drop for FrameTrackerInner {
    fn drop(&mut self) {
        //frame_dealloc(self);
        FRAME_ALLOCATOR.lock().dealloc(self.ppn.0);
    }
}

impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker: PPN = {:#x}", self.ppn.0))
    }
}

impl Deref for FrameTracker {
    type Target = Arc<FrameTrackerInner>;
    fn deref(&self) -> &Self::Target{
        &self.inner
    }
}

impl Clone for FrameTracker {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone()
        }
    }
}

pub fn frame_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR.lock().alloc().map(FrameTracker::new)
}

pub fn frame_safe_alloc() -> Option<FrameTracker> {
    if is_frame_adequate() {
        frame_alloc()
    } else {
        None
    }
}

pub fn is_frame_adequate() -> bool {
    get_available_frame_num() > 16
}

pub fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.lock().dealloc(ppn.0);
}

pub fn get_available_frame_num() -> usize {
    FRAME_ALLOCATOR.lock().available_num()
}

#[allow(unused)]
pub fn frame_allocator_test() {
    println!("1:{}", get_available_frame_num());
    let mut v: Vec<FrameTracker> = Vec::new();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        println!("2:{}", get_available_frame_num());
        v.push(frame);
    }
    println!("3:{}", get_available_frame_num());
    v.clear();
    println!("4:{}", get_available_frame_num());
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        v.push(frame);
    }
    println!("5:{}", get_available_frame_num());
    drop(v);
    println!("6:{}", get_available_frame_num());
    println!("frame_allocator_test passed!");
}
