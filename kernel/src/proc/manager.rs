
use core::sync::atomic::Ordering;

use alloc::collections::{VecDeque};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::lazy::Lazy;
use spin::{Mutex, MutexGuard};
use crate::config::*;
use crate::sbi::sbi_putchar;
use crate::syscall::time::Timespec;
use crate::timer::get_time;
use super::task::CHAN_ALLOCATOR;
use super::{TaskControlBlock, get_hartid};
use log::*;

#[derive(Debug, Clone, Copy)]
pub struct Channel(usize);

impl Channel {
    pub fn new() -> Self {
        Self(CHAN_ALLOCATOR.alloc_id().unwrap())
    }

    pub fn eq(chan1: &Channel, chan2: &Channel) -> bool {
        chan1.0 == chan2.0
    }
}

pub static CLOCK_WAKER: Lazy<Mutex<ClockWaker>> = Lazy::new(||{
    Mutex::new(ClockWaker::new())
});

pub struct ClockWaker {
    pub list: Vec<(Timespec, Weak<TaskControlBlock>)>
}

impl ClockWaker {
    pub fn new() -> Self {
        Self { list: Vec::new() }
    }

    pub fn wait(&mut self, task: Weak<TaskControlBlock>, time: Timespec) {
        trace!("clockwaker: add a task, time = {:?}", Timespec::now());
        self.list.push((time, task));
    }

    pub fn wake(&mut self) {
        //trace!("clock: try wake task, now = {:?}", Timespec::now());
        self.list
        .drain_filter(|(time, _)| time.pass())
        .collect::<Vec<_>>()
        .iter()
        .for_each(|(_, task)| {
            if let Some(task) = task.upgrade() {
                task.interrupted.store(true, Ordering::Release);
                wake_task(*task.get_channel()); 
            } 
        });
    }

    fn debug_print(&self) {
        //trace!("clockwaker: sleeping task num: {}, not: {:?}", self.list.len(), Timespec::now());
        for (time, task) in self.list.iter() {
            trace!(" time: {:?}, id = {}", time, task.upgrade().unwrap().tid);
        }
    }
}

pub fn wake_clock_futex_task() {
    CLOCK_WAKER.lock().wake();
}

pub fn add_clock_futex_task(task: Weak<TaskControlBlock>, time: Timespec) {
    CLOCK_WAKER.lock().wait(task, time);
}

pub struct TaskManager {
    pub ready_tasks: ReadyList,
    pub stopped_tasks: StoppedList,
    pub running_tasks: [Option<Arc<TaskControlBlock>>; MAX_CPU_NUM]
}

pub struct ReadyList {
    pub list: VecDeque<Arc<TaskControlBlock>>,
}

impl ReadyList {
    pub fn new() -> Self {
        Self {
            list: VecDeque::new()
        }
    }

    pub fn push(&mut self, task: Arc<TaskControlBlock>) {
        self.list.push_back(task);
    }

    pub fn pop(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.list.pop_front()
    } 
}

pub struct StoppedList {
    list: Vec<(Channel, Arc<TaskControlBlock>)>,
}

impl StoppedList {
    pub fn new() -> Self {
        Self{
            list: Vec::new()
        }
    }

    pub fn push(&mut self, chan: Channel, task: Arc<TaskControlBlock>) {
        self.list.push((chan, task));
        drop(chan);
    }

    pub fn pop(&mut self, chan: Channel) -> Option<Arc<TaskControlBlock>> {
        let find_pair = self.list.iter().enumerate()
        .find(|(_, (chan2, _))| {
            Channel::eq(&chan, chan2)
        });
        if let Some((i, _)) = find_pair {
            let (_, task) = self.list.remove(i);
            trace!("stopped_list: wake a task, tid = {}", task.tid);
            Some(task)
        } else {
            None
        }
    }
}


impl TaskManager {
    pub fn new() -> Self {
        Self {
            ready_tasks: ReadyList::new(),
            stopped_tasks: StoppedList::new(),
            running_tasks: Default::default()
        }
    }

     
    pub fn add_task(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_tasks.push(task);
    }
    
    pub fn fetch_task(&mut self) -> Option<Arc<TaskControlBlock>> {
        if let Some(task) = self.ready_tasks.pop() {
            Some(task)
        } else {
            None
        }
    }
    
    pub fn pop_task(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.ready_tasks.pop()
    }

    pub fn sleep_task(&mut self, task: Arc<TaskControlBlock>, chan: Channel) {
        self.stopped_tasks.push(chan, task);
    }
    
    pub fn wake_task(&mut self, chan: Channel) {
        self.stopped_tasks
        .pop(chan)
        .map(|task| self.ready_tasks.push(task));
    }

    pub fn get_task_by_tid(&self, tid: i32) -> Option<Arc<TaskControlBlock>> {
        for task in self.ready_tasks.list.iter() {
            if task.tid == tid {
                return Some(task.clone())
            }
        }
    
        for task in self.running_tasks.iter() {
            if let Some(task1) = task {
                if task1.tid == tid {
                    return Some(task1.clone())
                }
            }
        }
    
        for (_, task) in self.stopped_tasks.list.iter() {
            if task.tid == tid {
                return Some(task.clone());
            }
        }

        None
    }

    pub fn get_task_by_pid(&self, pid: i32) -> Option<Arc<TaskControlBlock>> {
        for task in self.ready_tasks.list.iter() {
            if task.pid == pid {
                return Some(task.clone())
            }
        }
    
        for task in self.running_tasks.iter() {
            if let Some(task1) = task {
                if task1.pid == pid {
                    return Some(task1.clone())
                }
            }
        }
    
        for (_, task) in self.stopped_tasks.list.iter() {
            if task.pid == pid {
                return Some(task.clone());
            }
        }
        None
    }

    pub fn debug_print(&self) {
        for task in self.ready_tasks.list.iter() {
            println!("task: {}   --Ready", task.tid);
        }
        for task in self.running_tasks.iter() {
            if task.is_some() {
                println!("task: {}   --Runing", task.as_ref().unwrap().tid);
            }
        }
        for (_, task) in self.stopped_tasks.list.iter() {
            println!("task: {}   --Sleeping", task.tid);
        }
    }
}

// 将running_lists中的当前CPU的任务清除
// 只会被exit_current调用
pub fn free_current_in_running_list() {
    TASK_MANAGER.lock()
    .running_tasks[get_hartid()]
    .take()
    .unwrap();
}

/* 将task加入到就绪队列 */
pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.lock().add_task(task);
}

// 只会被scheduler调用，所以需要在running_list上标记上task
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.lock().fetch_task()
}

/* 将task阻塞 */
pub fn sleep_task(task: Arc<TaskControlBlock>, chan: Channel) {
    TASK_MANAGER.lock().sleep_task(task, chan)
}

/* 将task唤醒，加入到就绪队列中 */
pub fn wake_task(chan: Channel) {
    TASK_MANAGER.lock().wake_task(chan)
}


pub fn get_task_by_pid(pid: i32) -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.lock().get_task_by_pid(pid)
}

pub fn get_task_by_tid(tid: i32) -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.lock().get_task_by_tid(tid)
}

pub static TASK_MANAGER: Lazy<Mutex<TaskManager>> = Lazy::new(||{
    Mutex::new(TaskManager::new())
});