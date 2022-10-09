use crate::trap::trap_return;

//#[derive(Copy,Clone)]
#[repr(C)]
#[derive(Copy, Clone)]
pub struct TaskContext {
    ra: usize,
    sp: usize, 
    //callee-saved
    s: [usize; 12],
}

impl TaskContext {
    pub fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    pub fn goto_trap_return(&mut self, kstack: usize) {
        self.ra = trap_return as usize;
        self.sp = kstack;
    }
}