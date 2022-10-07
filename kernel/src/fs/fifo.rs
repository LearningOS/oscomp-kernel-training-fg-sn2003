// from rCore_tutorial v3   fixme: license


use super::File;
use super::file::PollType;
use spin::Mutex;
use alloc::vec::Vec;
use alloc::sync::{Arc,Weak};
use crate::utils::Error;
use crate::proc::{suspend_current, get_current_task};
use crate::config::PIPE_BUFFER_SIZE;
use crate::utils::mem_buffer::MemBuffer;
use log::*;

#[derive(Copy, Clone, PartialEq)]
enum PipeBufferStatus {
    FULL,
    EMPTY,
    NORMAL
}

pub struct RingBuffer<T> {
    buffer: [u8; PIPE_BUFFER_SIZE],
    head: usize,
    tail: usize,
    status: PipeBufferStatus,
    write_end: Option<Weak<T>>,
}

impl<T> RingBuffer<T> {
    pub fn new() -> Self {
        Self {
            buffer: [0; PIPE_BUFFER_SIZE],
            head: 0,
            tail: 0,
            status: PipeBufferStatus::EMPTY,
            write_end: None,
        }
    }

    pub fn all_wirte_ends_closed(&self) -> bool {
        self.write_end.as_ref().unwrap().upgrade().is_none()
    }

    pub fn record_write_end(&mut self, write_pipe: &Arc<T>) {
        self.write_end = Some(Arc::downgrade(write_pipe));
    }

    pub fn available_read_bytes(&self) -> usize {
        if self.status == PipeBufferStatus::EMPTY {
            0
        } else if self.tail > self.head {
            self.tail - self.head
        } else {
            self.tail + PIPE_BUFFER_SIZE - self.head
        }
    }

    pub fn available_write_bytes(&self) -> usize {
        if self.status == PipeBufferStatus::FULL {
            0
        } else {
            PIPE_BUFFER_SIZE - self.available_read_bytes()
        }
    }

    pub fn read_one_byte(&mut self) -> u8 {
        self.status = PipeBufferStatus::NORMAL;
        let byte = self.buffer[self.head];
        self.head = (self.head + 1) % PIPE_BUFFER_SIZE;
        if self.head == self.tail {
            self.status = PipeBufferStatus::EMPTY;
        }
        byte
    }

    pub fn write_one_byte(&mut self, byte: u8) {
        self.status = PipeBufferStatus::NORMAL;
        self.buffer[self.tail] = byte;
        self.tail = (self.tail + 1) % PIPE_BUFFER_SIZE;
        if self.tail == self.head {
            self.status = PipeBufferStatus::FULL;
        }
    }
}

type PipeRingBuffer = RingBuffer<Pipe>;

pub struct Pipe {
    readable: bool,
    writable: bool,
    buffer: Arc<Mutex<PipeRingBuffer>>,
}

impl Pipe{
    pub fn set_wirte(buffer: Arc<Mutex<PipeRingBuffer>>) -> Self{
        Self{
            readable:false,
            writable:true,
            buffer:buffer.clone(),
        }
    }
    pub fn set_read(buffer: Arc<Mutex<PipeRingBuffer>>) -> Self{
        Self{
            readable:true,
            writable:false,
            buffer:buffer.clone(),
        }
    }
}

pub fn create_pipe() -> (Arc<Pipe>, Arc<Pipe>){
    let buffer: Arc<Mutex<PipeRingBuffer>> = Arc::new(Mutex::new(PipeRingBuffer::new()));
    let write_pipe:Arc<Pipe> = Arc::new(Pipe::set_wirte(buffer.clone()));
    let read_pipe :Arc<Pipe> = Arc::new(Pipe::set_read(buffer.clone()));
    buffer.lock().record_write_end(&write_pipe);
    (read_pipe,write_pipe)
}

impl File for Pipe {
    fn close(&self) -> Result<(),Error>{
        drop(self);
        Ok(())
    }

    fn read(&self, len: usize) -> Result<Vec<u8>, Error> {
        if !self.readable {
            warn!("pipe_read: This pipe is not readable for current process!");
            return Err(Error::EPERM);
        }

        let mut read_buf: Vec<u8> = Vec::new();
        let mut has_read: usize = 0;
        let mut loop_read ;
        /* 没有数据则会阻塞 */
        loop {
            let task = get_current_task().unwrap();

            let buffer = self.buffer.lock();
            loop_read = buffer.available_read_bytes();
            if loop_read != 0 {
                break;
            } else if buffer.all_wirte_ends_closed() {
                return Ok(read_buf)
            } else if task.has_signal() {
                //task.set_interrupted();
                return Err(Error::EINTR);
            } else {
                drop(buffer);
                drop(task);
                suspend_current();
            }
        }
        let mut buffer = self.buffer.lock();
        for _ in 0..loop_read {
            if has_read == len {
                break;
            }
            read_buf.push(buffer.read_one_byte());
            has_read += 1;
        }
        return Ok(read_buf);
    }

    fn write(&self, data: Vec<u8>) -> Result<usize, Error> {
        if !self.writable {
            warn!("pipe_write: This pipe is not writable for current process!");
            return Err(Error::EPERM);
        }
        let len:usize = data.len();
        let mut has_write:usize = 0;

        while  has_write < len {
            let mut buffer = self.buffer.lock();
            let loop_write = buffer.available_write_bytes();
            trace!("pipe_write: loop_write = {}", loop_write);
            let task = get_current_task().unwrap();
            if task.has_signal() {
                //task.set_interrupted();
                return Err(Error::EINTR);
            } 
            if loop_write == 0{
                drop(task);
                drop(buffer);
                suspend_current();
                continue;
            }
            for _ in 0..loop_write{
                if has_write == len {
                    return Ok(len);
                }
                buffer.write_one_byte(data[has_write]);
                has_write += 1;
            }
        }
        return Ok(len);
    }

    fn readable(&self) -> bool {
        self.readable
    }

    fn writable(&self) -> bool {
        self.writable
    }

    fn poll(&self, ptype: PollType) -> Result<bool, Error> {
        let buffer = self.buffer.lock();
        let ret = match ptype {
            PollType::READ => buffer.available_read_bytes() != 0,
            PollType::WRITE => buffer.available_write_bytes() != 0,
            PollType::ERR => false
        };
        Ok(ret)
    }
}