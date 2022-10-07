use alloc::collections::{BTreeMap, VecDeque};
use super::*;

pub struct FutexList {
    pub list: BTreeMap<usize, VecDeque<Arc<TaskControlBlock>>>,
}

impl FutexList {
    pub fn new() -> Self {
        Self {list: BTreeMap::new()}
    }

    pub fn add_task(&mut self, task: Arc<TaskControlBlock>, key: usize) {
        trace!("futexlist.add_task : task.tid = {} waiting on {:x}", task.tid, key);
        if self.list.contains_key(&key) {
            let queue = self.list.get_mut(&key).unwrap();
            queue.push_back(task);
        } else {
            let mut queue = VecDeque::new();
            queue.push_back(task);
            self.list.insert(key, queue);
        }
    }

    pub fn wake(&mut self, key: usize, num: usize) -> usize {
        trace!("futexlist.wake: waking key = {:x}", key);
        if self.list.contains_key(&key) {
            let queue = self.list.get_mut(&key).unwrap();
            for i in 0..num {
                
                if let Some(task) = queue.pop_front() {
                    trace!("futexlist:wake a task, tid = {}", task.tid);
                    wake_task(*task.get_channel());
                } else {
                    return i;
                }
            }
            return num;
        } else {
            return 0;
        }
    }

    pub fn requeue(&mut self, key1: usize, key2: usize, num1: usize, num2: usize) -> usize {
        let mut wake_num = 0;
        let queue1;
        if self.list.contains_key(&key1) {
            queue1 = self.list.get_mut(&key1).unwrap();
            for i in 0..num1 {
                if let Some(task) = queue1.pop_front() {
                    wake_task(*task.get_channel());
                } else {
                    wake_num = i;
                    break;
                }
            }
        } else {
            return 0;
        }
        drop(queue1);
        
        let mut remain = self.list.remove(&key1).unwrap();
        warn!("requeue: wake_num = {}, remain = {}", wake_num, remain.len());
        if remain.len() > num2 as usize {
            remain.pop_back();
        }


        if self.list.contains_key(&key2) {
            let queue = self.list.get_mut(&key2).unwrap();
            queue.append(&mut remain);
        } else {
            self.list.insert(key2, remain);
        }

        return wake_num as usize;
    }
}