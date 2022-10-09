mod context;
mod switch;
mod kernel_stack;
mod manager;
mod processor;
mod resource;
mod task;
mod sig_handlers;
mod signal;
mod futex;
mod thread;

use core::sync::atomic::Ordering;
use spin::{lazy::Lazy, MutexGuard};
use log::*;
use alloc::{sync::Arc, boxed::Box};
use context::TaskContext;
use switch::__switch;
use processor::sched;
pub use resource::*;
pub use task::*;
pub use signal::*;
pub use manager::*;
pub use processor::*;
pub use futex::*;
pub use thread::*;
pub use kernel_stack::KernelStack;
use crate::{loader::get_app_data_by_name, memory::copyout};

use self::manager::{sleep_task};

/* 让出当前task的CPU */
pub fn suspend_current() {
    let current_task = get_current_task().unwrap();
    let mut task_info = current_task.get_task_info();
    let task_context_ptr = &mut task_info.context as *mut TaskContext;

    current_task.set_status(TaskStatus::READY);
    drop(task_info);
    drop(current_task);
    sched(task_context_ptr);
}


pub fn sleep_current<T>(chan: Channel, lock: MutexGuard<T>) {
    let current_task = take_current_task().unwrap();
    let mut task_info = current_task.get_task_info();
    let task_context_ptr = &mut task_info.context as *mut TaskContext;
    current_task.time_info.lock().update_time_scheduled();
    current_task.set_status(TaskStatus::STOPPED);
    drop(task_info);
    sleep_task(current_task, chan);
    drop(lock);
    sched(task_context_ptr);
}

pub fn exit_current(exit_code: i32) {
    let current_task = get_current_task().unwrap();
    current_task.exit_code.store(exit_code, core::sync::atomic::Ordering::SeqCst);
    current_task.time_info.lock().update_time_scheduled();
    current_task.clear_child_tid();

    let mut thread_group = current_task.get_thread_group();
    /* 在线程组中删除该线程 */
    thread_group.thread_exit();
    if !thread_group.is_empty() && thread_group.is_leader(&current_task) {
        error!("exit_current: leader exit, tid = {}", current_task.tid);
        todo!();
    }
    if thread_group.is_empty() {
        trace!("exit_current: last thread exit");
        current_task.free_resource();
        thread_group.list.clear();
        INITPROC.adopt_orphan(&current_task);
    }
    drop(thread_group);
    
    take_current_task().unwrap();
    current_task.set_status(TaskStatus::ZOMBIE);
    drop(current_task);

    let mut unuse = TaskContext::zero_init();
    sched(&mut unuse as *mut TaskContext);
}


pub fn exit_current_group(exit_code: i32) {
    let current_task = get_current_task().unwrap();
    current_task.exit_code.store(exit_code, Ordering::SeqCst);
    current_task.time_info.lock().update_time_scheduled();
    INITPROC.adopt_orphan(&current_task);
    let mut thread_group = current_task.get_thread_group();
    current_task.clear_child_tid();
    /* 清空线程组的所有线程 */
    thread_group.thread_exit();
    if !thread_group.is_empty() && thread_group.is_leader(&current_task) {
        error!("exit_current_group: leader exit, tid = {}", current_task.tid);
        todo!();
    }
    for thread in thread_group.list.iter() {
        //thread.set_status(TaskStatus::ZOMBIE);
    }
    if thread_group.is_empty() {
        current_task.free_resource();
    }
    thread_group.list.clear();
    let mut fdtable = current_task.get_fd_table();
    fdtable.table.clear();
    drop(fdtable);
    drop(thread_group);

    take_current_task().unwrap();
    
    let parent = current_task.parent.lock().as_ref().unwrap().upgrade().unwrap();
    let chan = parent.get_channel();
    wake_task(*chan);
    drop(chan);

    current_task.set_status(TaskStatus::ZOMBIE);
    drop(current_task);
    
    let mut unuse = TaskContext::zero_init();
    sched(&mut unuse as *mut TaskContext);
}

pub fn add_initproc() {
    println!("[kernerl] add initproc");
    add_task(INITPROC.clone());
}

pub static INITPROC: Lazy<Arc<TaskControlBlock>> = Lazy::new(||{
    TaskControlBlock::new(
        get_app_data_by_name("initproc").unwrap()
    )
});

pub fn get_tid() -> i32 {
    get_current_task().unwrap().tid
}