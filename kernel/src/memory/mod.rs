mod heap_allocator;
mod address;
mod pagetable;
mod frame_allocator;
mod memory_set;
pub mod swap;

use log::*;
use address::*;
pub use frame_allocator::*;
pub use address::*;
pub use pagetable::*;
pub use memory_set::*;
pub use swap::*;

pub fn kernel_space_activate() {
    KERNEL_SPACE.lock().activate();         //开启SV39分页机制
}

pub fn init_heap_allocator() {
    heap_allocator::init_heap_allocator();
}

pub fn init_frame_allocator() {
    println!("[kernel] init FRAME ALLOCATOR, frame num = 0x{:x}", get_available_frame_num());
}


pub fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

pub fn get_kernel_space_satp() -> usize {
    KERNEL_SPACE.lock().token() | (1 << PPN_WIDTH)
}