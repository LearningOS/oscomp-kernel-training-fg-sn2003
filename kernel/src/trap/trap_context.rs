use riscv::register::sstatus::{self, Sstatus, SPP};
use crate::proc::get_hartid;

/*
    register abi_name   description                         saver
    x0          zero    hard-wired zero                     
    x1          ra      return address                      caller
    x2          sp      stack pointer                       callee
    x3          gp      global pointer
    x4          tp      thread pointer
    x5-7        t0-2    temporariers                        caller
    x8          s0/fp   saved register/frame pointer        callee
    x9          s1      saved register                      callee
    x10-11      a0-1    function arguments/return values    caller
    x12-17      a2-7    function arguments                  caller
    x18-27      s2-11   saved registers                     callee
    x28-31      t3-6    temporaries                         caller
*/



#[repr(C)]
#[derive(Copy, Clone,Debug)]
pub struct TrapContext{
    pub x: [usize; 32],
    pub f: [usize; 32],
    pub sstatus: Sstatus,
    pub sepc: usize,
    pub kernel_satp: usize,
    pub kernel_sp: usize,
    pub trap_handler: usize,
    pub hart_id: usize,
}

impl TrapContext {
    pub fn init(
        entry: usize, 
        sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            f: [0; 32],
            sstatus,
            sepc: entry,
            kernel_satp,
            kernel_sp,
            trap_handler,
            hart_id: get_hartid()
        };
        cx.x[2] = sp;
        cx
    }
}