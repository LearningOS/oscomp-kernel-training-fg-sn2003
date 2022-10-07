use core::sync::atomic::{AtomicUsize, Ordering};
use alloc::vec::Vec;

pub struct IdAllocator {
    current: AtomicUsize,
    end: usize,
}

impl IdAllocator {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            current: AtomicUsize::new(start),
            end: end,
        }
    }

    pub fn alloc_id(&self) -> Option<usize> {
        let current = self.current.fetch_add(1, Ordering::Release);
        if current >= self.end {
            return None;
        }
        return Some(current);
    }
 }

 
pub trait RecyledAllocator {
    fn new(start: usize, end: usize) -> Self;
    fn alloc(&mut self) -> Option<usize>;
    fn dealloc(&mut self, a: usize);
    fn available_num(&self) -> usize; 
}

 pub struct StackAllocator {
    current: usize,
    end: usize,
    recycled: Vec<usize>,
 }

 impl RecyledAllocator for StackAllocator {
    fn new(start: usize, end: usize) -> Self {
        StackAllocator { 
            current: start, 
            end: end, 
            recycled: Vec::new() }
    }

    fn alloc(&mut self) -> Option<usize> {
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn)
        } else if self.current == self.end {
            None
        } else {
            self.current += 1;
            Some(self.current - 1)
        }
    }

    fn dealloc(&mut self, ppn: usize) {
        if ppn >= self.current || self.recycled.iter().any(|&v| v == ppn) {
            panic!("Frame ppn={:#x} has not been allocated!", ppn);
        }
        self.recycled.push(ppn);
    }

    fn available_num(&self) -> usize {
        self.end - self.current + self.recycled.len()
    }
 }