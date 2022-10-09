use virtio_drivers::{VirtIOBlk, VirtIOHeader};
use spin::Mutex;
use spin::lazy::Lazy;
use alloc::vec::Vec;
use crate::memory::{PhysAddr, PhysPageNum, frame_alloc, frame_dealloc, 
    FrameTracker, VirtAddr, PageTable, kernel_token};
use super::DevId;

pub struct VirtIOBlock {
    pub id: DevId,
    inner: Mutex<VirtIOBlk<'static>>
}

const VIRT_VIRTIO0: usize = 0x10001000;

static QUEUE_FRAMES: Lazy<Mutex<Vec<FrameTracker>>> = Lazy::new(||{Mutex::new(Vec::new())});

impl VirtIOBlock {
    pub fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        self.inner
        .lock()
        .read_block(block_id, buf)
        .expect("Error when reading VirtIOBlk");
    }

    pub fn write_block(&self, block_id: usize, buf: &[u8]) {
        self.inner
        .lock()
        .write_block(block_id, buf)
        .expect("Error when reading VirtIOBlk");
    }
}

impl VirtIOBlock {
    pub fn new() -> Self {
        Self{
            id: DevId::new(),
            inner: unsafe {
                Mutex::new(
                    VirtIOBlk::new(&mut *(VIRT_VIRTIO0 as *mut VirtIOHeader)).unwrap())
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn virtio_dma_alloc(pages: usize) -> PhysAddr {
    let mut ppn_base = PhysPageNum(0);
    for i in 0..pages {
        let frame = frame_alloc().unwrap();
        if i == 0 {
            ppn_base = frame.ppn;
        }
        assert_eq!(frame.ppn.0, ppn_base.0 + i);
        QUEUE_FRAMES.lock().push(frame);
    }
    ppn_base.into()
}

#[no_mangle]
pub extern "C" fn virtio_dma_dealloc(pa: PhysAddr, pages: usize) -> i32 {
    let mut ppn_base: PhysPageNum = pa.into();
    for _ in 0..pages {
        frame_dealloc(ppn_base);
        ppn_base.0 += 1;
    }
    0
}

#[no_mangle]
pub extern "C" fn virtio_phys_to_virt(paddr: PhysAddr) -> VirtAddr {
    VirtAddr(paddr.0)
}

#[no_mangle]
pub extern "C" fn virtio_virt_to_phys(vaddr: VirtAddr) -> PhysAddr {
    unsafe{
        PageTable::from_token(kernel_token())
        .translate_va_to_pa(vaddr)
        .unwrap()
    }
}