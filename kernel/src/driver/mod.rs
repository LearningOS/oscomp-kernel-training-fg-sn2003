pub mod block_device;
pub mod serial;
//pub mod device_tree;

use core::sync::atomic::{AtomicUsize, Ordering};
pub use block_device::BLOCK_DEVICE;

pub static CURRENT_DEV_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DevId(pub usize);

impl DevId {
    pub fn new() -> Self {
        let id = CURRENT_DEV_ID.fetch_add(1, Ordering::Relaxed);
        //trace!("alloc dev_id = {}", id);
        Self(id)
    }
}