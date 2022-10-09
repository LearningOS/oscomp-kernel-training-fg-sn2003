use crate::fs::{CharFile, PTS, FileOpenMode};
use core::fmt::{self, Write};
use spin::Mutex;
use spin::lazy::Lazy;
use alloc::sync::Arc;
use crate::sbi::sbi_putchar; 

pub struct Console(pub Arc<dyn CharFile>);

pub static CONSOLE: Lazy<Mutex<Console>> = Lazy::new(||{
    Mutex::new(Console(PTS::new(FileOpenMode::SYS)))
});


impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            sbi_putchar(c as usize);
        }
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    let mut console = CONSOLE.lock();
    console.write_fmt(args).unwrap();
    drop(console);
}

#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?))
    }
}

#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}