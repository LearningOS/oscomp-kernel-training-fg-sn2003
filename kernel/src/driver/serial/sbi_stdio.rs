/* k210使用rust-sbi的sbi call来实现一个伪驱动 */
use super::LegacyStdio;
use crate::{sbi::{sbi_getchar, sbi_putchar}};
use crate::driver::DevId;
pub struct SBIStdio {
    pub id: DevId,
}

impl SBIStdio {
    #[allow(unused)]
    pub fn new() -> Self {
        Self {
            id: DevId::new()
        }
    }
}

impl LegacyStdio for SBIStdio {
    fn getchar(&mut self) -> u8 {
        sbi_getchar() as u8
    }

    fn putchar(&mut self, ch: u8) {
        sbi_putchar(ch as usize);
    }

    fn get_id(&self) -> usize {
        self.id.0
    }
}