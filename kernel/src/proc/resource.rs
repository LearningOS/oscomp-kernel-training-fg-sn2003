use spin::lazy::Lazy;
use crate::config::*;

pub type RlimArr = [Rlimit; RLIMIT::NLIMITS as usize];

#[derive(Debug, Clone, Copy, Default)]
pub struct Rlimit {
    pub rlim_cur: usize,
    pub rlim_max: usize,
}

/* RESOURCE */

#[derive(Debug, Clone, Copy)]
pub enum RLIMIT {
    CPU     = 0,
    FSZIE   = 1,
    DATA    = 2,
    STACK   = 3,
    CORE    = 4,
    NOFILE  = 7,
    NLIMITS = 15,
}

pub static DEFAULT_RLIMIT: Lazy<[Rlimit; RLIMIT::NLIMITS as usize]> = Lazy::new(||{
    let mut rlims: [Rlimit; RLIMIT::NLIMITS as usize] = [Default::default(); RLIMIT::NLIMITS as usize];
    rlims[RLIMIT::CPU as usize]   = Rlimit{rlim_cur: 1,  rlim_max: 1};
    rlims[RLIMIT::FSZIE as usize] = Rlimit{rlim_cur: 0x10000,  rlim_max: 0x10000};
    rlims[RLIMIT::DATA as usize]  = Rlimit{rlim_cur: 0x10000,  rlim_max: 0x10000};
    rlims[RLIMIT::STACK as usize] = Rlimit{rlim_cur: USER_STACK_SIZE,  rlim_max: USER_STACK_SIZE};
    rlims[RLIMIT::CORE as usize]   = Rlimit{rlim_cur: 2,  rlim_max: 2};
    rlims[RLIMIT::NOFILE as usize] = Rlimit{rlim_cur: MAX_FD_LEN as usize, rlim_max: MAX_FD_LEN as usize};
    rlims
});

pub fn rlimit_default() -> [Rlimit; RLIMIT::NLIMITS as usize] {
    DEFAULT_RLIMIT.clone()
}
