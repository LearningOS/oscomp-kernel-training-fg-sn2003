mod trap_context;
use core::arch::{global_asm, asm};
use alloc::sync::Arc;
use riscv::{register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap, Scause},
    stval,
    stvec,
    sstatus,
    sie,
    sepc,
}, addr::page};
use crate::{syscall::{syscall, time::Timespec}, proc::{trapframe_bottom, wake_clock_futex_task},proc::UContext, sbi::{sbi_putchar, sbi_remote_sfence_vma_all}};
use crate::config::*;
use crate::timer::{set_next_trigger};
use crate::proc::{
    get_current_trap_context,
    get_current_user_satp,
    exit_current,
    suspend_current,
    get_current_task,
    get_current_user_token,
};
use crate::memory::VirtAddr;
use log::*;
use crate::memory::{copyout};
use crate::proc::*;

pub use trap_context::TrapContext;

global_asm!(include_str!("trap.S"));

#[allow(unused)]
pub fn intr_on() {
    unsafe {sstatus::set_sie()}
}

pub fn intr_off() {
    unsafe {sstatus::clear_sie()}
}

pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

pub fn set_kernel_trap_entry() {
    unsafe {
        stvec::write(kernel_trap_handler as usize, TrapMode::Direct);
    }
}

pub fn set_user_trap_entry() {
    unsafe {
        stvec::write(TRAMPOLINE, TrapMode::Direct);
    }
}

pub fn flush_tlb() {
    unsafe{
        asm!(
            "csrw sstatus, {}",
            "sfence.vma zero, zero",
            in(reg) 0x8000000000006000 as usize,
        );
    }
}

#[no_mangle]
pub fn kernel_trap_handler() -> ! {
    println!("stval = {:#x}, sepc = {:#x}", stval::read(), sepc::read());
    panic!("a trap {:?} from kernel!", scause::read().cause());
}

pub fn pagefault_handler(scause: Scause, vaddr: VirtAddr) {
    let task =  get_current_task().unwrap();
    let mut memory_set =  task.get_memory();
    let tid = task.tid;
    let is_store = match scause.cause() {
        Trap::Exception(Exception::StorePageFault) => true,
        Trap::Exception(Exception::StoreFault) => true,
        Trap::Exception(Exception::LoadPageFault) => false,
        Trap::Exception(Exception::LoadFault) => false,
        Trap::Exception(Exception::InstructionPageFault) => false,
        Trap::Exception(Exception::InstructionFault) => false,
        _ => panic!()
    };
    let result = memory_set.page_check(vaddr, is_store); 
    drop(memory_set);
    drop(task);
    
    match result {
        Ok(_) => {
            flush_tlb();
            trace!("[kernel] handle a pagefault, vaddr = {:x} tid = {} is_store={:?}", vaddr.0, tid, is_store);
        }
        Err(string) => {
            let task = get_current_task().unwrap();
            task.get_memory().trace_areas();
            let trap_cx = task.get_trap_cx();
            println!("trap_cx = {:x?}", trap_cx);
            println!(
                "[kernel] {:?} in application, bad addr = {:#x}, bad instruction = {:#x}, {}, tid = {}.",
                scause.cause(),
                vaddr.0,
                task.get_trap_cx().sepc,
                string,
                get_tid()
            );
            panic!();
            drop(task);
            exit_current(-2);
        }
    };
}

#[no_mangle]
pub fn trap_handler(){
    set_kernel_trap_entry();

    let scause = scause::read();
    let stval = stval::read();
    //task.time_info.lock().update_time_enter_kernel();
    //wake_clock_futex_task();
    unsafe{
        asm!(
            "csrw sstatus, {}",
            in(reg) 0x8000000000006000 as usize,
        );
    }
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {
            let mut cx = get_current_trap_context();
            cx.sepc += 4;
            let result = syscall(cx.x[17], 
                [cx.x[10], cx.x[11], cx.x[12], cx.x[13], cx.x[14], cx.x[15]]) as usize;
            cx = get_current_trap_context();
            cx.x[10] = result as usize;
        }
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::LoadPageFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::InstructionFault) => {
            pagefault_handler(scause, stval.into());
        }

        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            suspend_current();
        }

        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x},sepc={:#x}!",
                scause.cause(),
                stval,
                get_current_trap_context().sepc,
            );
        }
    }
    trap_return();
}

pub fn trap_return() {
    extern "C" {
        fn uservec();
        fn userret();
    }

    let task = get_current_task().unwrap();
    task.handle_signal();

    let userret_va = userret as usize - uservec as usize + TRAMPOLINE;
    let trap_cx_ptr = trapframe_bottom(task.private_tid).0;
    let user_satp = task.get_user_token();
    set_user_trap_entry();
    drop(task);


    unsafe{
        asm!(
            "csrw sstatus, {}",
            in(reg) 0x8000000000006000 as usize,
        );
    }

    //error!("return, sstatus = {:x?}", sstatus::read());
    unsafe {
        asm!(
            "fence.i",
            "jr {userret_va}",
            userret_va = in(reg) userret_va,
            in("a0") trap_cx_ptr,
            in("a1") user_satp,
            options(noreturn)   
        );
    }
}
