#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![feature(drain_filter)]
#![feature(array_chunks)]
#![feature(btree_drain_filter)]
#![feature(ptr_metadata)]
#![feature(map_first_last)]
#![allow(unused)]

#[macro_use]
mod console;
mod syscall;
mod trap;
mod lang_item;
mod sbi;
mod config;
mod proc;
mod timer;
mod memory;
mod fs;
mod utils;
mod driver;
mod loader;
mod net;

#[cfg(feature = "board_k210")]
#[path = "board/k210.rs"]
mod board;

#[cfg(feature = "board_sifive")]
#[path = "board/sifive/mod.rs"]           
mod board;

#[cfg(not(any(feature = "board_k210", feature = "board_sifive")))]
#[path = "board/qemu.rs"]           
mod board;

use core::arch::{global_asm, asm};
use core::sync::atomic::{AtomicBool, Ordering};
use config::MAX_CPU_NUM;
use riscv::asm::wfi;
use riscv::register::{sstatus};
use sbi::{sbi_putchar, sbi_hart_start};
use trap::intr_off;
use utils::mem_buffer;

use crate::memory::KERNEL_SPACE;
use crate::proc::{get_hartid};
use crate::sbi::{sbi_send_ipi};
use log::*;

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate alloc;

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

static COULD_START_INIT: AtomicBool = AtomicBool::new(false);

fn start_other_harts(hartid: usize) {
    println!("Other cores start initializing");
    extern "C" {
        fn _other_core_start();
    }
    for i in 0..MAX_CPU_NUM {
        if i != hartid {
            sbi_hart_start(i, _other_core_start as usize, i);
        }
    }
}

#[no_mangle]
pub fn rust_main(hartid: usize, device_tree: usize) {
    intr_off();
    if hartid == 0 {
        trap::set_kernel_trap_entry();
        memory::clear_bss();
        memory::init_heap_allocator();
        memory::init_frame_allocator();
        utils::logger::init();        
        //driver::device_tree::init(device_tree);
        memory::kernel_space_activate();
        memory::load_dynamic_linker();
        fs::fs_init();
        proc::add_initproc();
        //start_other_harts(hartid);    /* 目前多核不完善 */
        //sbi::sbi_send_ipi(0b11);
        //COULD_START_INIT.store(true, Ordering::SeqCst);
        trap::enable_timer_interrupt();
        timer::set_next_trigger();
        proc::scheduler();
    } else {
        unsafe {
            wfi();
        }
        while !COULD_START_INIT.load(Ordering::SeqCst) {}
        other_main(hartid);
    }
}

#[no_mangle]
pub fn other_main(hartid: usize) {
    trace!("start initializing: hart_id = {}", get_hartid());
    trap::set_kernel_trap_entry();
    memory::kernel_space_activate();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    proc::scheduler();
}