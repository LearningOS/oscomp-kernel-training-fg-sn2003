use alloc::{collections::VecDeque, vec::Vec};
use core::slice::from_raw_parts_mut;
use crate::{memory::{VirtAddr, translate_byte_buffer}, fs::{mknod, FilePerm, FileOpenMode, open}};
use log::*;

pub struct MemBuffer {
    pub buffer: VecDeque<&'static mut [u8]>
}

impl MemBuffer {
    pub fn new(buffer: VecDeque<&'static mut [u8]>) -> Self {
        Self { buffer }
    }

    pub fn from_user_space(
        vaddr: *mut u8, 
        len: usize, 
        token: usize,
        is_to_write: bool
    ) -> Self {
        trace!("Membuffer.from_user_space: vaddr = {:x}. len = {:x}", vaddr as usize, len);
        Self {buffer: translate_byte_buffer(token, vaddr, len, is_to_write)}
    }

    pub fn from_kernel_space(addr: *mut u8, len: usize) -> Self {
        let mut buffer = VecDeque::new();
        unsafe {
            buffer.push_front(from_raw_parts_mut(addr, len));
        }
        Self { buffer }
    }

    pub fn len(&self) -> usize {
        let mut total: usize = 0;
        for b in self.buffer.iter() {
            total += b.len();
        }
        total
    }

    pub fn read_data_to_buffer(&mut self, data: &[u8]) {
        assert!(data.len() <= self.len());
        let mut total = 0;
        let len = data.len();
        loop {
            let buffer = self.buffer.pop_front().unwrap();
            let remain = len - total;
            let b_len = buffer.len();

            if b_len < remain {
                buffer.copy_from_slice(&data[total..total+b_len]);
                total += b_len;
            } else {
                buffer[..remain].copy_from_slice(&data[total..]); 
                let (_, left) = buffer.split_at_mut(remain);
                self.buffer.push_front(left);
                break;
            }    
        }
    }

    pub fn write_data_from_buffer(&mut self, data: &mut [u8]) {
        assert!(data.len() <= self.len());
        let mut total = 0;
        let len = data.len();
        loop {
            let buffer = self.buffer.pop_front().unwrap();  
            let remain = len - total;
            let b_len = buffer.len();

            if b_len < remain {
                data[total..total+b_len].copy_from_slice(buffer);
                total += b_len;
            } else {
                data[total..].copy_from_slice(&buffer[..remain]);
                let (_, left) = buffer.split_at_mut(remain);
                self.buffer.push_front(left);
                break;
            }
        }
    }

    pub fn to_vec(self) -> Vec<u8> {
        let mut vec = Vec::new();
        for buf in self.buffer.iter() {
            vec.append(&mut buf.to_vec());
        }
        vec
    }

    pub fn add_slice(&mut self, addr: *mut u8, len: usize) {
        unsafe {
            self.buffer.push_back(core::slice::from_raw_parts_mut(addr, len));
        }
    }

}

#[allow(unused)]
pub fn test() {


}