use crate::config::*;
use crate::memory::{KERNEL_SPACE, MapProt};

pub struct KernelStack {
    pub tid: i32,
    pub bottom: usize,
    pub top: usize,
}

impl KernelStack {
    pub fn new(tid: i32) -> Self {
        let (bottom, top) = kernel_stack_position(tid);
        KERNEL_SPACE.lock().insert_framed_area(
            bottom.into(),
             top.into(),
              MapProt::WRITE | MapProt::READ);
        KernelStack {tid, bottom, top}
    }
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        KERNEL_SPACE.lock()
            .remove_framed_area_with_start(self.bottom.into());
    }
}

fn kernel_stack_position(tid: i32) -> (usize, usize) {
    assert!(tid >= 0);
    let top = KERNEL_STACK_BASE - (tid as usize) * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}


