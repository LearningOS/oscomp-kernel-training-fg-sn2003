#![allow(unused)]
use spin::lazy::Lazy;
use alloc::{vec::Vec};
use crate::{proc::{
    sig_handlers::*
}, trap::trap_return};
use alloc::{
    collections::{BTreeMap}, 
};
use crate::config::*;
use super::{
    get_current_trap_context, get_current_task
};

//不可靠信号(最多只有一个排队)
pub const SIGHUP    : usize =  1; 
pub const SIGINT    : usize =  2; 
pub const SIGQUIT   : usize =  3; 
pub const SIGILL    : usize =  4; 
pub const SIGTRAP   : usize =  5; 
pub const SIGABRT   : usize =  6; 
pub const SIGBUS    : usize =  7; 
pub const SIGFPE    : usize =  8; 
pub const SIGKILL   : usize =  9; 
pub const SIGUSR1   : usize = 10; 
pub const SIGSEGV   : usize = 11; 
pub const SIGUSR2   : usize = 12; 
pub const SIGPIPE   : usize = 13; 
pub const SIGALRM   : usize = 14; 
pub const SIGTERM   : usize = 15; 
pub const SIGSTKFLT : usize = 16; 
pub const SIGCHLD   : usize = 17; 
pub const SIGCONT   : usize = 18; 
pub const SIGSTOP   : usize = 19; 
pub const SIGTSTP   : usize = 20; 
pub const SIGTTIN   : usize = 21; 
pub const SIGTTOU   : usize = 22; 
pub const SIGURG    : usize = 23; 
pub const SIGXCPU   : usize = 24; 
pub const SIGXFSZ   : usize = 25; 
pub const SIGVTALRM : usize = 26; 
pub const SIGPROF   : usize = 27; 
pub const SIGWINCH  : usize = 28; 
pub const SIGIO     : usize = 29; 
pub const SIGPWR    : usize = 30; 
pub const SIGSYS    : usize = 31; 

//保留信号量，提供给用户程序自行设置
pub const SIG_RESERVE_0    : usize = 32; 
pub const SIG_RESERVE_1    : usize = 33; 

//可靠信号(随便排队)
// pub const SIGRTMIN	: usize = 34;
// SIGRT_N = SIGRTMAX-B / SIGREMIN+A
// pub const SIGRTMAX	: usize = 64;

//当sig_handler不为以下值的时候,对sa_mask和sa_flag进行反应
pub const SIG_DFL: usize = 0;
pub const SIG_IGN: usize = 1;

//sa_flag
pub const NOCLDSTOP  :usize = 0x00000001;
pub const NOCLDWAIT  :usize = 0x00000002;
pub const SIGINFO    :u32   = 0x00000004;//目前只用到这个
pub const ONSTACK    :usize = 0x08000000;
pub const RESTART    :usize = 0x10000000;
pub const NODEFER    :usize = 0x40000000;
pub const UNSUPPORTED:usize = 0x00000400;
pub const RESETHAND  :usize = 0x80000000;

pub fn hander_signal() {
    let task = get_current_task().unwrap();
    task.handle_signal();
}

pub static SIGNAL_HANDLERS: Lazy<Vec<usize>> = Lazy::new(||{
    extern "C"{
        fn signal_default_handlers();
    }
    let ignore_va = def_ignore as usize - signal_default_handlers as usize +  SIG_DEFAULT_HANDLER;
    let core_va = def_terminate_self_with_core_dump as usize - signal_default_handlers as usize +  SIG_DEFAULT_HANDLER;
    let term_va = def_terminate_self as usize - signal_default_handlers as usize +  SIG_DEFAULT_HANDLER;
    let stop_va = def_stop as usize - signal_default_handlers as usize +  SIG_DEFAULT_HANDLER;
    let cont_va = def_continue as usize - signal_default_handlers as usize +  SIG_DEFAULT_HANDLER;
    let mut handlers = Vec::new();
    handlers.insert(0,0);
    handlers.insert(SIGHUP, term_va);
    handlers.insert(SIGINT, term_va);
    handlers.insert(SIGQUIT,term_va);
    handlers.insert(SIGILL, term_va);
    handlers.insert(SIGTRAP,ignore_va);
    handlers.insert(SIGABRT,core_va);
    handlers.insert(SIGBUS, core_va);
    handlers.insert(SIGFPE, core_va);
    handlers.insert(SIGKILL,term_va);
    handlers.insert(SIGUSR1,ignore_va);
    handlers.insert(SIGSEGV,core_va);
    handlers.insert(SIGUSR2,ignore_va);
    handlers.insert(SIGPIPE,term_va);
    handlers.insert(SIGALRM,term_va);
    handlers.insert(SIGTERM,term_va);
    handlers.insert(SIGSTKFLT,term_va);
    handlers.insert(SIGCHLD,ignore_va);
    handlers.insert(SIGCONT,cont_va);
    handlers.insert(SIGSTOP,stop_va);
    handlers.insert(SIGTSTP,stop_va);
    handlers.insert(SIGTTIN,stop_va);
    handlers.insert(SIGTTOU,stop_va);
    handlers.insert(SIGURG, ignore_va);
    handlers.insert(SIGXCPU,term_va);
    handlers.insert(SIGXFSZ,term_va);
    handlers.insert(SIGVTALRM,ignore_va);
    handlers.insert(SIGPROF,term_va);
    handlers.insert(SIGWINCH,ignore_va);
    handlers.insert(SIGIO,  ignore_va);
    handlers.insert(SIGPWR, ignore_va);
    handlers.insert(SIGSYS, term_va);
    handlers.insert(SIG_RESERVE_0, ignore_va);
    handlers.insert(SIG_RESERVE_1, ignore_va);
    handlers
});

#[repr(C)]
#[derive(Copy, Clone, PartialEq,Debug)]
pub struct stack_t{
    ss_sp:usize,
    ss_flags:i32,
    ss_size:usize,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq,Debug)]
pub struct sigset_t{
    pub __val:[usize;17],
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq,Debug)]
pub struct Sigaction {
    pub sa_handler: usize,      //默认信号处理函数
    pub sa_flags: u32,        //标志位
    pub sa_mask: usize,         //信号处理函数执行期间的mask码
    //pub sa_restorer: usize,     //也许用不上,保存用户进程trampoline地址?
    //pub sig_info_ptr: usize,
}
impl Sigaction {
    pub fn new(
        sa_handler: usize, 
        sa_mask: usize,
        sa_flags: u32,
    )->Sigaction{
        Sigaction{
            sa_handler,
            sa_mask,
            sa_flags,
        }
    }
}


#[allow(unused)]
#[repr(C)]
#[derive(Copy, Clone, PartialEq,Debug)]
pub struct SigInfo {
    si_signo     :i32,	/* Signal number */
    si_errno     :i32,	/* An errno value */
    si_code      :i32,	/* Signal code */
    si_trapno    :i32,	/* Trap number that caused hardware-generated signal (unused on most architectures) */
    si_pid       :u32,	/* Sending process ID */
    si_uid       :u32,	/* Real user ID of sending process */
    si_status    :i32,	/* Exit value or signal */
    si_utime     :i32,	/* User time consumed */
    si_stime     :i32,	/* System time consumed */
}
impl SigInfo{
    pub fn new() -> SigInfo{
        SigInfo{
            si_signo     :0,	
            si_errno     :0,	
            si_code      :0,	
            si_trapno    :0,	
            si_pid       :0,	
            si_uid       :0,	
            si_status    :0,	
            si_utime     :0,	
            si_stime     :0,	
        }
    }
}


#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MContext {
    pub greps: [usize; 32],
    reserve: [u8; 528]
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct UContext {
    pub uc_flags: usize,
    pub uc_link: usize, //指向上一个ucontext
    pub uc_stack: stack_t,
    pub uc_sigmask: sigset_t,
    pub uc_mcontext: MContext,
}
impl UContext {
    pub fn new() -> Self {
        let mut mcontext = MContext {
            greps: [0; 32],
            reserve: [0; 528],
        };
        mcontext.greps = get_current_trap_context().x;
        mcontext.greps[0] = get_current_trap_context().sepc;
        Self {
            uc_flags: 0,
            uc_link: 0,
            uc_stack: stack_t{ss_sp:0,ss_flags:0,ss_size:0} ,
            uc_mcontext: mcontext,
            uc_sigmask: sigset_t{__val:[0 as usize;17]},
        }
    }
}

//作为信号处理上下文压入栈中
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SignalContext{
    pub ucontext: UContext,
    pub info: SigInfo,
    pub signum: usize,
}

impl SignalContext {
    pub fn new(signum: usize) -> Self {
        Self { 
            ucontext: UContext::new(), 
            info: SigInfo::new(),
            signum 
        }
    }

    pub fn set_mask(&mut self, mask: usize) {
        self.ucontext.uc_sigmask.__val[0] = mask;
    }

    pub fn pc(&self) -> usize {
        self.ucontext.uc_mcontext.greps[0]
    }
}



pub fn signal_sigaction_init() -> BTreeMap<usize,Sigaction>{
    let mut new_handle = BTreeMap::new();

    new_handle.insert(SIGHUP,           Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGINT,           Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGQUIT,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGILL,           Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGTRAP,          Sigaction::new(SIG_IGN, !0, 0));
    new_handle.insert(SIGABRT,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGBUS,           Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGFPE,           Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGKILL,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGUSR1,          Sigaction::new(SIG_IGN, !0, 0));
    new_handle.insert(SIGSEGV,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGUSR2,          Sigaction::new(SIG_IGN, !0, 0));
    new_handle.insert(SIGPIPE,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGALRM,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGTERM,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGSTKFLT,        Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGCHLD,          Sigaction::new(SIG_IGN, !0, 0));
    new_handle.insert(SIGCONT,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGSTOP,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGTSTP,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGTTIN,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGTTOU,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGURG,           Sigaction::new(SIG_IGN, !0, 0));
    new_handle.insert(SIGXCPU,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGXFSZ,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGVTALRM,        Sigaction::new(SIG_IGN, !0, 0));
    new_handle.insert(SIGPROF,          Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIGWINCH,         Sigaction::new(SIG_IGN, !0, 0));
    new_handle.insert(SIGIO,            Sigaction::new(SIG_IGN, !0, 0));
    new_handle.insert(SIGPWR,           Sigaction::new(SIG_IGN, !0, 0));
    new_handle.insert(SIGSYS,           Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIG_RESERVE_0,    Sigaction::new(SIG_DFL, !0, 0));
    new_handle.insert(SIG_RESERVE_1,    Sigaction::new(SIG_DFL, !0, 0));

    new_handle
}
