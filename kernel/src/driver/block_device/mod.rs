pub mod virtio_blk;
#[cfg(feature = "board_sifive")]
pub mod sifive_sdcard;
#[cfg(feature = "board_k210")]
pub mod k210_sdcard;

use spin::lazy::Lazy;
use alloc::sync::Arc;
use crate::board::BlockDeviceImpl;
use super::DevId;

pub static BLOCK_DEVICE: Lazy<Arc<BlockDeviceImpl>> 
= Lazy::new(||{Arc::new(BlockDeviceImpl::new())});
