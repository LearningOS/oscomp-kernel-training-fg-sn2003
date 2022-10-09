use buddy_system_allocator::LockedHeap;
use crate::config::KERNEL_HEAP_SIZE;

#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap::<64> = LockedHeap::<64>::empty();

#[alloc_error_handler]
pub fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("Kernel Heap allocation error, layout = {:?}", layout);
}

static mut KERNEL_HEAP_SPACE:[u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

pub fn init_heap_allocator() {
    unsafe {
        let mut lock = HEAP_ALLOCATOR.lock();
        lock.init(
            KERNEL_HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE
        );
    }
}