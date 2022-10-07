use alloc::{sync::{Arc, Weak}, vec::Vec};
use spin::{Mutex, MutexGuard};

use super::{TaskControlBlock, Channel};

pub struct ThreadGroup {
    pub inner: Mutex<ThreadGroupInner>,
    chan: Arc<Mutex<Channel>>,
}

pub struct ThreadGroupInner {
    pub list: Vec<Arc<TaskControlBlock>>,
    pub leader: Option<Weak<TaskControlBlock>>,
    tgid: Option<i32>,
    num: usize,
}

impl ThreadGroupInner {
    pub fn add_leader(&mut self, leader: &Arc<TaskControlBlock>) {
        assert!(self.num == 0);
        self.leader = Some(Arc::downgrade(leader));
        self.tgid = Some(leader.tid);
        self.list.push(leader.clone());
        self.num = 1;
    }

    pub fn is_leader(&self, task: &Arc<TaskControlBlock>) -> bool {
        self.tgid.unwrap() == task.tid
    }

    pub fn add_task(&mut self, task: Arc<TaskControlBlock>) {
        assert!(self.leader.is_some());
        self.list.push(task);
        self.num += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.num == 0
    }

    pub fn get_tgid(&self) -> i32 {
        self.tgid.unwrap()
    }

    pub fn thread_exit(&mut self) {
        self.num -= 1;
    }
}

impl ThreadGroup {
    pub fn new() -> Self {
        Self { 
            inner: Mutex::new(ThreadGroupInner {
                list: Vec::new(),
                leader: None,
                num: 0,
                tgid: None
            }),
            chan: Arc::new(Mutex::new(Channel::new()))
        }
    }

    pub fn get_channel(&self) -> MutexGuard<Channel> {
        self.chan.lock()
    }
}