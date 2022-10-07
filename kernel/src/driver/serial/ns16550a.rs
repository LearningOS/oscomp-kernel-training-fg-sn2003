/// for qemu

use core::convert::Infallible;
use core::ptr::{read_volatile, write_volatile};
use embedded_hal::serial::{Read, Write};
use crate::driver::DevId;

use super::LegacyStdio;
use nb::block;

pub struct Ns16550a {
    id: DevId,
    base: usize,
    shift: usize,
}

#[allow(unused)]
const ADDR_QEMU_UART     : usize = 0x10000000;

impl Ns16550a {
    //RustSbi-qemu has initialized the Ns16550a
    pub fn new() -> Self {
        Self {
            id: DevId::new(),
            base: ADDR_QEMU_UART,
            shift: 0
        }
    }
}

impl Read<u8> for Ns16550a {
    type Error = Infallible;

    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        let pending =
            unsafe { read_volatile((self.base + (offsets::LSR << self.shift)) as *const u8) }
                & masks::DR;
        if pending != 0 {
            let word =
                unsafe { read_volatile((self.base + (offsets::RBR << self.shift)) as *const u8) };
            Ok(word)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

impl Write<u8> for Ns16550a {
    type Error = Infallible;

    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        // 写，但是不刷新
        unsafe { write_volatile((self.base + (offsets::THR << self.shift)) as *mut u8, word) };
        Ok(())
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        let pending =
            unsafe { read_volatile((self.base + (offsets::LSR << self.shift)) as *const u8) }
                & masks::THRE;
        if pending != 0 {
            // 发送已经结束了
            Ok(())
        } else {
            // 发送还没有结束，继续等
            Err(nb::Error::WouldBlock)
        }
    }
}

impl LegacyStdio for Ns16550a {
    #[inline]
    fn getchar(&mut self) -> u8 {
        block!(self.read()).ok().unwrap()
    }

    #[inline]
    fn putchar(&mut self, ch: u8) {
        block!(self.write(ch)).ok();
        block!(self.flush()).ok();
    }

    fn get_id(&self) -> usize {
        self.id.0
    }
}

#[allow(unused)]
mod offsets {
    pub const RBR: usize = 0x0;
    pub const THR: usize = 0x0;

    pub const IER: usize = 0x1;
    pub const FCR: usize = 0x2;
    pub const LCR: usize = 0x3;
    pub const MCR: usize = 0x4;
    pub const LSR: usize = 0x5;

    pub const DLL: usize = 0x0;
    pub const DLH: usize = 0x1;
}

mod masks {
    pub const THRE: u8 = 1 << 5;
    pub const DR: u8 = 1;
}
