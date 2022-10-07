use core::arch::asm;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::lazy::Lazy;
use super::*;
use crate::config::MAX_CPU_NUM;
use crate::sbi::sbi_putchar;
use crate::timer::get_time;
use crate::trap::TrapContext;
use crate::utils::UPSafeCell;

pub struct Processor {
    current: Option<Arc<TaskControlBlock>>, 
    idle_task_cx: TaskContext,  
}

impl Processor {
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    pub fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        (&mut self.idle_task_cx ) as *mut TaskContext
    }

    pub fn take_current_task(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    pub fn get_current_task(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }

    pub fn get_current_user_token(&self) -> usize {
        self.get_current_task()
        .unwrap()
        .get_user_token()
    }

    pub fn get_current_trap_context(&self) -> &'static mut TrapContext {
        self.get_current_task()
        .unwrap()
        .get_task_info()
        .get_trap_cx()
    }
}


pub static PROCESSOR_LIST: Lazy<Vec<UPSafeCell<Processor>>> = Lazy::new(||{
    let mut list = Vec::new();
    for _ in 0..MAX_CPU_NUM {
        list.push(UPSafeCell::new(Processor::new()));
    }
    list
});

#[no_mangle]
#[inline(always)]
pub fn get_hartid() -> usize {
    let mut hartid: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) hartid);
    }
    hartid
}

pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    let id = get_hartid();
    PROCESSOR_LIST[id].lock().take_current_task()
}

pub fn get_current_task() -> Option<Arc<TaskControlBlock>> {
    let id = get_hartid();
    PROCESSOR_LIST[id].lock().get_current_task()
}

pub fn get_idle_task_cx_ptr() -> *mut TaskContext {
    let id = get_hartid();
    PROCESSOR_LIST[id].lock().get_idle_task_cx_ptr()
}

pub fn get_current_trap_context() -> &'static mut TrapContext {
    let id = get_hartid();
    PROCESSOR_LIST[id].lock().get_current_trap_context()
}

pub fn get_current_user_token() -> usize {
    let id = get_hartid();
    PROCESSOR_LIST[id].lock().get_current_user_token()
}

pub fn get_current_user_satp() -> usize {
    get_current_user_token()
}

/* 切换任务上下文 */
pub fn sched(switched_task_cx_ptr: *mut TaskContext) {
    unsafe {
        __switch(
            switched_task_cx_ptr,
            get_idle_task_cx_ptr()
        )
    }
}

pub fn scheduler() {
    loop {
        let id = get_hartid();
        let mut processor = PROCESSOR_LIST[id].lock();

        if let Some(task) = processor.current.take() {
            add_task(task);
        }

        if let Some(task) = fetch_task() {
            // if task.is_zombie() {
            //     unimplemented!();
            //     free_current_in_running_list();
            //     drop(task);
            //     continue;
            // }
            let switch_task_cx_ptr = processor.get_idle_task_cx_ptr();
            let task_info = task.get_task_info();
            let next_task_cx_ptr = &task_info.context as *const TaskContext;
            task.time_info.lock().timestamp_enter_smode = get_time() as u64;
            task.set_status(TaskStatus::RUNNING);

            drop(task_info);
            processor.current = Some(task);
            drop(processor);
            unsafe{
                __switch(switch_task_cx_ptr, next_task_cx_ptr);
            }
        }
    }
}
