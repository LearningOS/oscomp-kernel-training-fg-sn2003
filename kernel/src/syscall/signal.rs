use crate::memory::{copyout, copyin};
use crate::proc::{get_current_task, get_current_user_token, get_task_by_pid,get_task_by_tid,get_current_trap_context};
pub use crate::proc::{Sigaction,UContext,SignalContext,SigInfo,SiganlStack_info};
use crate::proc::{SIGKILL,SIGSTOP};
use crate::utils::Error;
use core::ptr;
use log::*;


pub fn sys_sigaction(
    signum: usize, 
    act: *mut Sigaction, 
    old_act: *mut Sigaction
) -> Result<isize, Error> {
    if signum == SIGKILL || signum == SIGSTOP {
        return Err(Error::EINVAL);
    }

    if ptr::null() != act {
        let user_token = get_current_user_token();
        let task = get_current_task().unwrap();
        let mut new_sigaction = Sigaction::new(0,0,0);
        let handlers = &mut task.get_handlers().table;
        if ptr::null()!= old_act {
            let old_sigaction = handlers.get(&signum).ok_or(Error::EINVAL)?;
            copyout(user_token,old_act,old_sigaction)?;
        }
        copyin(user_token,&mut new_sigaction,act)?;
        new_sigaction.sa_mask = new_sigaction.sa_mask & !(1 << SIGKILL) & !(1 << SIGSTOP); //kill和stop信号不能被屏蔽
        let sigaction = handlers.get_mut(&signum).ok_or(Error::EINVAL)?;
        *sigaction = new_sigaction;
    }
    Ok(0)
}


const SIG_BLOCK     : usize = 0;
const SIG_UNBLOCK   : usize = 1;
const SIG_SETMASK   : usize = 2;
pub fn sys_sigprocmask(how: usize, set: *mut usize, oldset: *mut usize) -> Result<isize, Error> {
    trace!("sys_sigprocmask: how = {} ,set = {:x?}, oldset = {:x?}", how , set, oldset);

    let task = get_current_task().unwrap();
    let token = task.get_user_token();
    let sig_mask = &mut *task.sig_mask.lock();

    let mut user_set: usize = 0;

    if ptr::null() != oldset {
        copyout(token,oldset,&sig_mask)?;
    }

    if set == ptr::null_mut() {
        return Ok(0);
    }

    copyin(token,&mut user_set,set)?;

    user_set = user_set & !(1 << SIGKILL) & !(1 << SIGSTOP); //kill和stop信号不能被屏蔽
    let ret = match how {
        SIG_BLOCK =>{
            *sig_mask = *sig_mask | user_set;
            Ok(0)
        }
        SIG_UNBLOCK =>{
            *sig_mask = *sig_mask & !user_set;
            Ok(0)
        }
        SIG_SETMASK =>{
            *sig_mask = user_set;
            Ok(0)
        }
        _=>{
            Err(Error::EINVAL)
        }
    };
    ret
}

pub fn sys_sigtimedwait() -> Result<isize, Error> {
    Ok(0)
}

//向pid指定的进程发送信号
pub fn sys_kill(pid: i32, signum: usize) -> Result<isize, Error> {
    println!("sys_kill: pid = {}, signum = {}", pid, signum);
    let task = match pid {
        0 => get_current_task().unwrap(),
        _ => get_task_by_pid(pid).ok_or(Error::ESRCH)?,
    };
    task.p_pending.lock().pending_signal(signum);
    Ok(0)
}

//向tid指定的进程发送信号
pub fn sys_tkill(tid: i32, signum: usize) -> Result<isize, Error>{

    let task = get_task_by_tid(tid).ok_or(Error::ESRCH)?;
    task.t_pending.lock().pending_signal(signum);
    Ok(0)
}

pub fn sys_tgkill(tgid: i32, tid: i32, signum: usize) -> Result<isize, Error> {
    println!("sys_tgkill: tgid = {}, tid = {}, signum = {}", tgid, tid, signum);
    /* 线程组id = 组长tid */
    let task = get_task_by_tid(tgid).ok_or(Error::ESRCH)?;
    if tgid != tid {
        todo!("send signum to certain group member");
    }
    task.p_pending.lock().pending_signal(signum);
    Ok(0)
}

pub fn sys_sigreturn() -> Result<isize, Error> {
    //panic!();
    //还原被保存的signal_context
    let task =  get_current_task().unwrap();
    let user_token = task.get_user_token();
    let mut signal_context = SignalContext::new(0);
    let mut signal_context_ptr = task.signal_context_ptr.lock();
    copyin(user_token,&mut signal_context,*signal_context_ptr as *mut SignalContext)?;//将signal_context数据取出

    
    let trap_cx = task.get_trap_cx();
    //恢复上下文

    trap_cx.x = signal_context.ucontext.uc_mcontext.greps;
    trap_cx.x[0] = 0;
    trap_cx.sepc = signal_context.pc();

    *task.sig_mask.lock() = signal_context.ucontext.uc_sigmask.__val[0];
    *signal_context_ptr = signal_context.ucontext.uc_link;
    Ok(0)
}



//替换信号处理时的上下文
pub fn sys_sigaltstack(signal_stack:usize,old_signal_stack:usize) -> Result<isize,Error> {

    let task =  get_current_task().unwrap();
    let user_token = task.get_user_token();
    let mut signal_context_ptr = task.signal_context_ptr.lock();
    let mut signal_context = SignalContext::new(0);
    Ok(0)
}
