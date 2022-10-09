use alloc::vec::Vec;

use super::*;
use crate::{config::*, utils::Error};

pub struct SysLog {
    buf: [u8; SYSLOG_SIZE],
}

impl File for SysLog {
    fn read(&self, len : usize) -> Result<Vec<u8>, Error> {
        todo!()
    }

    fn write(&self, data: Vec<u8>) -> Result<usize, Error> {
        todo!()
    }
}