/* from FLT OS */
/* todo: add license */

pub mod clock;
pub mod spi;

pub const CLOCK_FREQ    : usize = 26_000_000;

//MMIO
pub const MMIO: &[(usize, usize)] = &[
    (0x10040000, 0x1000),
    (0x10041000, 0x1000),
    (0x10050000, 0x1000),
];

pub type BlockDeviceImpl = crate::driver::block_device::sifive_sdcard::SDCardWrapper;
pub type StdioImpl = crate::driver::serial::sbi_stdio::SBIStdio;