use core::mem::{zeroed};
use core::ops::Add;
use crate::timer::{get_time, get_time_ms};
use crate::proc::{get_current_task, get_current_user_token, suspend_current};
use crate::memory::{copyout,copyin};
use crate::board::CLOCK_FREQ;
use crate::utils::Error;
use log::*;

const MSEC_PER_SEC: usize = 1_000;
const USEC_PER_SEC: usize = 1_000_000;
const NSEC_PER_SEC: usize = 1_000_000_000;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Timespec {
    pub tv_sec: isize,
    pub tv_nsec: isize,
}

impl Add for Timespec {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Timespec {
            tv_sec: (self.tv_nsec + rhs.tv_nsec) / 1_000_000_000 + self.tv_sec + rhs.tv_sec,
            tv_nsec: (self.tv_nsec + rhs.tv_nsec) % 1_000_000_000,
        }
    }
}

impl Timespec {
    pub const ZERO: Self = Self {
        tv_sec: 0, tv_nsec: 0
    };

    pub fn now() -> Self {
        let time = get_time();
        let sec = time / CLOCK_FREQ;
        let nsec = (time % CLOCK_FREQ) * (NSEC_PER_SEC / CLOCK_FREQ);
        Self {
            tv_sec: sec as isize,
            tv_nsec: nsec as isize,
        }
    }

    pub fn from_tick(tick: usize) -> Self {
        let s = tick / CLOCK_FREQ;
        let ns =  (tick % CLOCK_FREQ) * NSEC_PER_SEC / CLOCK_FREQ;
        Self { tv_sec: s as isize, tv_nsec: ns as isize }
    }

    pub fn to_tick(&self) -> usize {
        self.tv_sec as usize * CLOCK_FREQ 
            + self.tv_nsec as usize * CLOCK_FREQ / NSEC_PER_SEC
    }
 
    pub fn pass(&self) -> bool {
        let now = Self::now();
        if self.tv_sec < now.tv_sec {
            return true;
        } else if self.tv_sec == now.tv_sec && self.tv_nsec < now.tv_nsec {
            return true;
        } else {
            return false;
        }
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct Timeval {
    pub tv_sec: isize,
    pub tv_usec: isize,
}

impl Timeval {
    pub const ZERO: Self = Self {
        tv_sec: 0,
        tv_usec: 0
    };

    pub fn now() -> Self {
        Self::from_tick(get_time())
    }

    pub fn from_tick(tick: usize) -> Self {
        let s = tick / CLOCK_FREQ;
        let us = (tick % CLOCK_FREQ) * USEC_PER_SEC / CLOCK_FREQ;
        Self { tv_sec: s as isize, tv_usec: us as isize}
    }

    pub fn to_tick(&self) -> usize {
        self.tv_sec as usize * CLOCK_FREQ 
            + self.tv_usec as usize * CLOCK_FREQ / USEC_PER_SEC
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Tms {
    pub tms_utime: isize,   //进程在U态消耗的CPU时间
    pub tms_stime: isize,   //进程在S态消耗的CPU时间
    pub tms_cutime: isize,  //所有子进程的utime之和
    pub tms_cstime: isize,  //所有子进程的stime之和
}



pub fn sys_nanosleep(req_ptr: *const Timespec, _rem_ptr: *mut Timespec) -> Result<isize, Error> {
    let mut req: Timespec = unsafe {zeroed()};
    copyin(get_current_user_token(), &mut req, req_ptr)?;

    if req.tv_sec < 0 || req.tv_nsec < 0 || req.tv_nsec > (NSEC_PER_SEC as isize) {
        return Err(Error::EINVAL);
    }
    
    let start = get_time_ms() as isize;
    let end = start + req.tv_sec * 1000 + req.tv_nsec / (USEC_PER_SEC as isize);
    trace!("sys_nanosleep: sleep: start = {}, end = {}", start, end);
    while get_time_ms() < end as usize {
        suspend_current();
    } 

    Ok(0)
}

pub fn sys_setitimer() -> Result<isize, Error> {
    trace!("sys_setitimer, unimplemented, return Ok(0)");
    Ok(0)
}

pub fn sys_gettimeofday(timeval_ptr: *mut Timeval) -> Result<isize, Error> {
    let timeval = Timeval::now();
    //trace!("timeval = {:?}", timeval);
    copyout(get_current_user_token(), timeval_ptr, &timeval)?;
    Ok(0)
}

pub fn sys_times(tms_ptr: *mut Tms) -> Result<isize, Error> {
    let task = get_current_task().unwrap();
    let time_info = task.time_info.lock();
    let tms = Tms {
        tms_utime: time_info.utime as isize,
        tms_stime: (time_info.stime + (get_time() as u64 - time_info.timestamp_enter_smode)) as isize,
        tms_cutime: time_info.sum_of_dead_descenants_utime as isize,
        tms_cstime: time_info.sum_of_dead_descenants_stime as isize,
    };

    copyout(get_current_user_token(), tms_ptr, &tms)?;
    Ok(crate::timer::get_time_ms() as isize)
}

pub fn sys_clock_gettime(clk_id: i32, tp: *mut Timespec) -> Result<isize, Error> {
    //trace!("sys_clock_gettime: clk_id = {}", clk_id);
    let token = get_current_user_token();
    copyout(token, tp, &Timespec::now())?;
    Ok(0)
}