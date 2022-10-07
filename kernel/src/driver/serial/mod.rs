pub mod sbi_stdio;
pub mod ns16550a;
use core::any::Any;
use spin::Mutex;
use spin::lazy::Lazy;
use alloc::sync::Arc;
use crate::board::{StdioImpl};


pub trait LegacyStdio: Send + Sync + Any {
    fn getchar(&mut self) -> u8;
    fn putchar(&mut self, ch: u8);
    fn get_id(&self) -> usize;
}

pub static STDIO: Lazy<Arc<Mutex<dyn LegacyStdio>>> = Lazy::new(||{
    Arc::new(Mutex::new(StdioImpl::new()))});

