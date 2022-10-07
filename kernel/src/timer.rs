use riscv::register::time;
use crate::sbi::set_timer;
use crate::board::CLOCK_FREQ;
use crate::config::TICKS_PER_SEC;

const MSEC_PER_SEC: usize = 1000;

//读取CPU复位以来的时间，驱动time计数器是已知的固定频率的时钟
pub fn get_time() -> usize {
    time::read()
}

pub fn get_time_ms() -> usize {
    get_time() / (CLOCK_FREQ / MSEC_PER_SEC)
}

pub fn set_next_trigger() {
    set_timer(get_time() + CLOCK_FREQ / TICKS_PER_SEC);
}