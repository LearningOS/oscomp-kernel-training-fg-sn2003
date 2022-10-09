#![allow(unused)]
use core::arch::asm;

/* Legacy Extensions */
const SBI_SET_TIMER: usize = 0;
const SBI_CONSOLE_PUTCHAR: usize = 1;
const SBI_CONSOLE_GETCHAR: usize = 2;
const SBI_CLEAR_IPI: usize = 3;
const SBI_SEND_IPI: usize = 4;
const SBI_REMOTE_FENCE_I: usize = 5;
const SBI_REMOTE_SFENCE_VMA: usize = 6;
const SBI_REMOTE_SFENCE_VMA_ASID: usize = 7;
const SBI_SHUTDOWN: usize = 8;

const EID_HSM: usize = 0x48534D;
const HSM_HART_START: usize = 0;

#[inline(always)]
fn sbi_call(eid: usize, fid: usize, arg0: usize, arg1: usize, arg2: usize) -> usize {
    let mut ret;
    unsafe {
        asm!{
            "ecall",
            inlateout("x10") arg0 => ret,
            in("x11") arg1,
            in("x12") arg2,
            in("x16") fid,
            in("x17") eid,
        };
    }
    ret
}

pub fn set_timer(timer: usize) {
    sbi_call(SBI_SET_TIMER, 0, timer, 0, 0);
}
#[no_mangle]
pub fn sbi_putchar(c: usize) {
    sbi_call(SBI_CONSOLE_PUTCHAR, 0, c, 0, 0);
}

pub fn sbi_getchar() -> usize {
    sbi_call(SBI_CONSOLE_GETCHAR,  0, 0, 0, 0)
}

pub fn sbi_send_ipi(hart_mask: usize) -> usize {
    sbi_call(SBI_SEND_IPI, 0, &hart_mask as *const _ as usize, 0, 0)
}

pub fn sbi_hart_start(hartid: usize, start_addr: usize, a1: usize) -> usize {
    sbi_call(EID_HSM, HSM_HART_START, hartid, start_addr, a1)
}

pub fn sbi_remote_sfence_vma_all() -> usize {
    let hart_mask = 0xffff;
    sbi_call(SBI_REMOTE_SFENCE_VMA, 0, &hart_mask as *const _ as usize, 0, 0xffffffffffffffff)
}

pub fn shutdown() -> ! {
    sbi_call(SBI_SHUTDOWN, 0, 0, 0, 0);
    panic!("It should shutdown!");
}
