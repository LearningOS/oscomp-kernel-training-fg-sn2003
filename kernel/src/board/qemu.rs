#[allow(unused)]
pub const ADDR_BLK      : usize = 0x10001000;
#[allow(unused)]
pub const ADDR_UART     : usize = 0x10000000;

pub const CLOCK_FREQ    : usize = 12_500_000;

//MMIO
pub const MMIO: &[(usize, usize)] = &[
    (0x10001000, 0x1000),   //virtio-blk
    (0x10000000, 0x1000),   //uart
];

pub type BlockDeviceImpl    = crate::driver::block_device::virtio_blk::VirtIOBlock;
pub type StdioImpl          = crate::driver::serial::sbi_stdio::SBIStdio;
