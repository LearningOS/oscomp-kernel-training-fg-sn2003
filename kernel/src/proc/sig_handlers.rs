use core::arch::asm;
use crate::syscall::{SYSCALL_EXIT_GROUP};

#[no_mangle]
#[link_section = ".text.signal"]
pub fn def_ignore(){
    //println!("sig receive success!");
}

#[no_mangle]
#[link_section = ".text.signal"]
pub fn def_terminate_self(){
    unsafe {
        asm!(
            "ecall",
            in("a7") SYSCALL_EXIT_GROUP
        )
    }
}

#[no_mangle]
#[link_section = ".text.signal"]
//结束进程并打印内存快照
pub fn def_terminate_self_with_core_dump(){
    unsafe {
        asm!(
            "ecall",
            in("a7") SYSCALL_EXIT_GROUP
        )
    }
}

#[no_mangle]
#[link_section = ".text.signal"]
pub fn def_stop(){
    
}

#[no_mangle]
#[link_section = ".text.signal"]
pub fn def_continue(){

}