use core::sync::atomic::{AtomicI32, AtomicU8, AtomicU64, Ordering};
use _core::{sync::atomic::{AtomicBool, AtomicUsize}, panic};
use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
    string::String,
    collections::{BTreeMap, VecDeque},
};
use riscv::register::sstatus::{Sstatus, self};
use spin::{Mutex, MutexGuard};
use log::*;
use bitflags::*;
use spin::lazy::Lazy;
use crate::{memory::{MemorySet, PhysPageNum, VirtAddr, copyout, get_kernel_space_satp, VirtPageNum, swap::SwapFrame}, timer::get_time, proc::manager::wake_task};
use crate::trap::{TrapContext, trap_handler};
use crate::config::*;
use crate::fs::{File, PTS, FileOpenMode};
use crate::utils::{Path, Error, allocator::IdAllocator};
use super::*;

pub static TID_ALLOCATOR: Lazy<IdAllocator> = Lazy::new(||{
    IdAllocator::new(0, u32::MAX as usize)});

pub static CHAN_ALLOCATOR: Lazy<IdAllocator> = Lazy::new(||{
    IdAllocator::new(0, usize::MAX)});

pub static TASK_NUM: AtomicUsize = AtomicUsize::new(0);

fn alloc_tid() -> i32 {
    TID_ALLOCATOR.alloc_id().unwrap() as i32    
}

pub struct Aux {
    pub aux_type: usize,
    pub value: usize,
}

bitflags! {
    pub struct CloneFlags: u32 {
         /* set if VM shared between processes */
         const VM = 0x0000100;  
         /* set if fs info shared between processes */
         const FS = 0x0000200;  
         /* set if open files shared between processes */
         const FILES = 0x0000400; 
         /* set if signal handlers and blocked signals shared */
         const SIGHAND = 0x00000800; 
         /* set if we want to have the same parent as the cloner */
         const PARENT = 0x00008000; 
         /* Same thread group? */
         const THREAD = 0x00010000; 
         /* share system V SEM_UNDO semantics */
         const SYSVSEM = 0x00040000;
         /* create a new TLS for the child */
         const SETTLS = 0x00080000;
         /* set the TID in the parent */
         const PARENT_SETTID = 0x00100000;
         /* clear the TID in the child */
         const CHILD_CLEARTID = 0x00200000;
         /* Unused, ignored */
         const CLONE_DETACHED = 0x00400000;
        /* set the TID in the child */
         const SETTID = 0x01000000;
         const CLONE_VFORK	 = 0x00004000;
        }
}

impl CloneFlags {
    pub fn valid(&self) -> Result<(), Error> {
        if self.contains(CloneFlags::VM) && !self.contains(CloneFlags::SIGHAND) {
            return Err(Error::EINVAL);
        }

        if (self.contains(CloneFlags::SETTID) || self.contains(CloneFlags::PARENT_SETTID)
        || self.contains(CloneFlags::CHILD_CLEARTID) || self.contains(CloneFlags::CLONE_DETACHED)
        || self.contains(CloneFlags::VM)) && !self.contains(CloneFlags::THREAD) {
            return Err(Error::EINVAL)
        }
        Ok(())
    }
}

/* file description table */
pub struct FdTable {
    pub table: BTreeMap<u32, Arc<dyn File>>,
}

impl FdTable {
    pub fn alloc_fd(&mut self, limit: usize) -> Option<u32> {
        let num = self.table.len();
        if num + 1 > limit {
            return None;
        }
        (0..MAX_FD_LEN).find(|fd| !self.table.contains_key(fd))
    }

    pub fn get_file(&self, fd: u32) -> Result<Arc<dyn File>, Error> {
        let file = self.table.get(&fd);
        match file {
            None => Err(Error::EBADF),
            Some(file) => Ok(file.clone())
        }
    }

    pub fn delete_file(&mut self, fd: u32) -> Result<u32, Error> {
        match self.table.remove(&fd) {
            Some(_) => Ok(fd),
            None => Err(Error::EBADF)
        }
    }

    pub fn add_file(&mut self, file: Arc<dyn File>, limit: usize) -> Result<u32, Error> {
        let fd = self.alloc_fd(limit);
        if fd.is_none() {
            /* limit on the number of open file
              descriptors has been reached */
            return Err(Error::EMFILE);
        }
        let fd = fd.unwrap();

        self.table.insert(fd, file);
        trace!("add_file: add a file to fd_table({})", fd);
        Ok(fd)
    }
}

/* file system information struct */
pub struct FsStruct {
    pub cwd: Path
}

/* 包含了线程间不会共享的一些数据 */
pub struct TaskInfo {
    pub context: TaskContext,                   
    pub trap_cx_ppn: PhysPageNum,       /* trapframe所在物理页页号 */

    pub set_child_tid   :usize,    /* CLONE_CHILD_SETTID */
    pub clear_child_tid :usize,    /* CLONE_CHILD_CLEARTID */
}

impl TaskInfo {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
}

pub struct TimeStruct {
    pub utime: u64,               /*进程在U态消耗的CPU时间*/
    pub stime: u64,               /*进程在S态消耗的CPU时间*/
    pub sum_of_dead_descenants_utime: u64,   
    pub sum_of_dead_descenants_stime: u64,   
    pub timestamp_enter_smode :u64, /*进程这次进入S态的时间戳*/
    pub timestamp_exit_smode  :u64, /*进程上次退出S态的时间戳*/
}

impl TimeStruct {
    pub fn new() -> Self {
        Self {
            utime: 0,
            stime: 0,
            sum_of_dead_descenants_stime: 0,
            sum_of_dead_descenants_utime: 0,
            timestamp_enter_smode: 0,
            timestamp_exit_smode : 0,
        }
    }

    pub fn update_time_scheduled(&mut self) {
        self.timestamp_exit_smode = get_time() as u64;
        self.stime += self.timestamp_exit_smode - self.timestamp_enter_smode;
    } 

    pub fn update_time_enter_kernel(&mut self) {
        self.timestamp_enter_smode = get_time() as u64;
        self.utime += self.timestamp_enter_smode - self.timestamp_exit_smode;
    }

    pub fn update_time_exit_kernel(&mut self) {
        self.timestamp_exit_smode = get_time() as u64;
        self.stime += self.timestamp_exit_smode - self.timestamp_enter_smode;
    }

    pub fn update_time_child_exit(&mut self, child_time_info: &TimeStruct) {
        self.sum_of_dead_descenants_stime 
            += child_time_info.sum_of_dead_descenants_stime + child_time_info.stime;
            self.sum_of_dead_descenants_utime
            += child_time_info.sum_of_dead_descenants_utime + child_time_info.utime;
    }
}

pub struct SigHandlers {
    pub table: BTreeMap<usize, Sigaction>,
}

impl SigHandlers {
    pub fn new() -> Self {
        Self { table: signal_sigaction_init() }
    }
}

pub struct SigPending {
    pub pendings: Vec<usize>,
}

impl SigPending {
    pub fn new() -> Self {
        Self { pendings: Vec::new() }
    }

    fn block(signo: usize, mask: usize) -> bool {
        ((1 << (signo - 1)) & mask) != 0
    }

    pub fn get_signal(&mut self, mask: usize) -> Option<usize> {
        let pendings = &mut self.pendings;
        pendings
        .iter()
        .position(|signo| !Self::block(*signo, mask))
        .map(|pos| 
            pendings.remove(pos))
    }

    pub fn has_signal(&self, mask: usize) -> bool {
        let pendings = &self.pendings;
        pendings
        .iter()
        .any(|signo| !Self::block(*signo, mask))
    }

    pub fn pending_signal(&mut self, signum: usize) {
        let pending = &mut self.pendings;
        match signum {
            1..=33 => {
                if !pending.contains(&signum) {
                    pending.push(signum);
                }
            }
            34..=64 => {
                pending.push(signum);
            }
            _ => {
                warn!("pending_signal: {} is unknown", signum);
            }
        }
    }
}


pub struct RobustList {
    pub head: usize,
    pub len: usize,
}

impl RobustList {
    pub fn _new(tid: i32) -> Self {
        Self { head: 0, len: 0 }
    }
}

pub struct SwapList {
    pub list: BTreeMap<VirtPageNum, SwapFrame>,
}

impl SwapList {
    pub fn new() -> Self {
        Self { list: BTreeMap::new() }
    }
}

impl Clone for SwapList {
    fn clone(&self) -> Self {
        let mut new = Self::new();
        if self.list.len() == 0 {
            return new;
        }
        
        for (vpn, swap_frame) in self.list.iter() {
            new.list.insert(*vpn, swap_frame.clone());
        } 
        new
    }
}

pub struct TaskControlBlock {
    pub status: AtomicU8,      /* 0: Ready; 1: Running; 2: Stopped; 3: Zombie */
    pub kernel_stack: KernelStack,  
    pub chan: Arc<Mutex<Channel>>,
    /* id */
    pub pid : i32,
    pub tid : i32,          /* global tid */
    pub private_tid: i32,   /* private_tid */

    /* exit */
    pub exit_code   :AtomicI32,
    pub exit_signal :AtomicU64, 

    pub exit        :AtomicBool,
    pub fp_modifyed :AtomicBool,              /* 浮点寄存器是否被修改 */
    pub interrupted :AtomicBool,              /* 是否在睡眠的时候被中断 */
    pub signal_context_ptr    :Mutex< usize>, /* 线程处理信号函数时的上下文指针 */
    pub task_info   :Mutex< TaskInfo>,
    pub parent      :Mutex< Option<Weak<TaskControlBlock>>>,
    pub time_info   :Mutex< TimeStruct>,
    pub sig_mask    :Mutex< usize>,
    pub t_pending   :Mutex< SigPending>,  
    pub robust_list :Mutex< Option<RobustList>>,
    
    /* 可以共享的成员 */
    pub groups      :Arc<ThreadGroup>,
    pub memory      :Arc<Mutex< MemorySet>>,
    pub swap        :Arc<Mutex< SwapList>>,
    pub child       :Arc<Mutex< Vec<Arc<TaskControlBlock>>>>,
    pub rlim        :Arc<Mutex< RlimArr>>,
    pub fs_info     :Arc<Mutex< FsStruct>>,
    pub fd_table    :Arc<Mutex< FdTable>>,
    pub handlers    :Arc<Mutex< SigHandlers>>,
    pub p_pending   :Arc<Mutex< SigPending>>,
    pub futex_list  :Arc<Mutex< FutexList>>,
}

#[allow(non_snake_case)]
pub mod TaskStatus {
    pub const READY  :u8 = 0;
    pub const RUNNING:u8 = 1;
    pub const STOPPED:u8 = 2;
    pub const ZOMBIE :u8 = 3;
}


impl TaskControlBlock {
    pub fn get_memory(&self) -> MutexGuard<MemorySet> {
        self.memory.lock()
    }

    pub fn get_fd_table(&self) -> MutexGuard<FdTable> {
        self.fd_table.lock()
    }

    pub fn get_fs_info(&self) -> MutexGuard<FsStruct> {
        self.fs_info.lock()
    }

    pub fn get_user_token(&self) -> usize {
        self.get_memory().token()
    }

    pub fn get_pid(&self) -> i32 {
        self.pid
    }

    pub fn get_handlers(&self) -> MutexGuard<SigHandlers> {
        self.handlers.lock()
    }

    pub fn get_task_info(&self) -> MutexGuard<TaskInfo> {
        self.task_info.lock()
    }

    pub fn get_rlim(&self) -> MutexGuard<RlimArr> {
        self.rlim.lock()
    }

    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.task_info.lock().get_trap_cx()
    }

    pub fn get_exit_code(&self) -> i32 {
        self.exit_code.load(Ordering::Relaxed)
    }
    
    pub fn get_max_fd(&self) -> usize {
        let rlim = self.rlim.lock();
        rlim[RLIMIT::NOFILE as usize].rlim_cur
    }

    pub fn get_file(&self, fd: u32) -> Result<Arc<dyn File>, Error> {
        self.fd_table.lock().get_file(fd)
    }

    pub fn get_futex_list(&self) -> MutexGuard<FutexList> {
        self.futex_list.lock()
    }

    pub fn get_thread_group(&self) -> MutexGuard<ThreadGroupInner> {
        self.groups.inner.lock()
    }

    pub fn set_status(&self, status: u8) {
        self.status.store(status, Ordering::Release);
    }

    pub fn set_interrupted(&self) {
        self.interrupted.store(true, Ordering::SeqCst);
    }

    pub fn is_interrupted(&self) -> bool {
        self.interrupted.fetch_and(false, Ordering::SeqCst)
    }

    pub fn get_status(&self) -> u8 {
        self.status.load(Ordering::Relaxed)
    }
    
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::ZOMBIE
    }

    /* 必须要获取thread_group的锁 */
    pub fn clear_child_tid(&self) {
        let task_info = self.task_info.lock();
        if task_info.clear_child_tid != 0 {
            let mut futex_list = self.get_futex_list();
            copyout(self.get_user_token(), task_info.clear_child_tid as *mut i32, &0).unwrap();
            futex_list.wake(task_info.clear_child_tid, 1);
        }
    }

    pub fn adopt_orphan(self: &Arc<TaskControlBlock>, task: &Arc<TaskControlBlock>) {
        assert!(self.tid == 0);
        trace!("Iniproc adopt all child of task: {:?}", task.tid);
        let mut initproc_child = self.child.lock();
        let mut child = task.child.lock();

        for task in child.iter() {
            *task.parent.lock() = Some(Arc::downgrade(self));
            initproc_child.push(task.clone());
        }
        child.clear();
    }

    pub fn get_channel(&self) -> MutexGuard<Channel> {
        self.chan.lock()
    }

    pub fn get_thread_channel(&self) -> MutexGuard<Channel> {
        self.groups.get_channel()
    }

    pub fn get_signal(&self) -> Option<usize> {
        let mask = *self.sig_mask.lock();
        self
            .t_pending
            .lock()
            .get_signal(mask)
            .or(self.p_pending.lock().get_signal(mask))
    }

    pub fn has_signal(&self) -> bool {
        let mask = *self.sig_mask.lock();
        self.t_pending.lock().has_signal(mask)
        || self.p_pending.lock().has_signal(mask)
    }

    pub fn is_leader(&self) -> bool {
        self.get_thread_group().get_tgid() == self.tid
    }

    pub fn free_resource(&self) {
        let mut memory = self.get_memory();
        memory.clear();
        let mut handler = self.get_handlers();
        handler.table.clear();
        let mut fd_table = self.get_fd_table();
        fd_table.table.clear();
        
    }

    pub fn handle_signal(&self) {
        extern "C"{
            fn __user_call_sigreturn();
            fn signal_default_handlers();
        }

        if self.t_pending.lock().pendings.len() == 0 
         && self.p_pending.lock().pendings.len() == 0 
        {
            return;
        }
        
        let signo = match self.get_signal() {
            Some(signo) => {
                signo
            }
            None => return
        };

        let mask = *self.sig_mask.lock();
        let mut handlers = self.get_handlers();

        let sigaction = match handlers.table.get_mut(&signo) {
            Some(sigaction) => sigaction,
            None => {
                warn!("handle signal: sigaction is null");
                return;
            }
        };

        /* 在修改上下文前保存上下文 */
        let trap_context = self.get_trap_cx();

        //创建信号处理上下文
        let mut signal_context = SignalContext::new(signo);
        signal_context.set_mask(mask);
        /* 将SignalContext数据放入栈中 */
        trap_context.x[2] = trap_context.x[2] - core::mem::size_of::<SignalContext>();//栈空出SignalContext的位置
        let signal_context_addr = trap_context.x[2];  //获取SignalContext的用户地址
        let ucontext_addr = signal_context_addr ;   //获取ucontext的用户地址
        let siginfo_addr  = signal_context_addr + core::mem::size_of::<UContext>();//获取siginfo的用户地址
        let user_token = self.get_user_token();
        copyout(user_token,signal_context_addr as *mut SignalContext,&mut signal_context).unwrap();//将SignalContext数据放入栈中

        /* 更新当前的signal_context指针 */
        let mut signal_context_ptr = self.signal_context_ptr.lock();
        signal_context.ucontext.uc_link = *signal_context_ptr;
        *signal_context_ptr = signal_context_addr;
        drop(signal_context_ptr);

        let mut task_sig_mask = self.sig_mask.lock();
        *task_sig_mask = *task_sig_mask | (sigaction.sa_mask | (1 << signo - 1));   //信号函数执行时掩码生效,自身信号也被屏蔽
        match sigaction.sa_handler {
            SIG_DFL => trap_context.sepc = SIGNAL_HANDLERS[signo],
            SIG_IGN => trap_context.sepc = SIGNAL_HANDLERS[signo],
            _=>{
                trap_context.x[10] = signo;                 //信号序号作为处理函数的唯一输入参数

                if sigaction.sa_flags & SIGINFO != 0 {
                    trap_context.x[11] = siginfo_addr;      //a1寄存器设置为指向siginfo的指针
                    trap_context.x[12] = ucontext_addr;     //a2寄存器设置为指向ucontext的指针
                }
                trap_context.sepc = sigaction.sa_handler;   //设置函数入口地址
            }
        }

        trap_context.x[1] = __user_call_sigreturn as usize - signal_default_handlers as usize + SIG_DEFAULT_HANDLER; //计算__user_call_sigreturn虚拟地址
        info!("ready to handle signal");
    }

    /* 通过elf文件创建一个而进程, 只有在创建init进程的时候才会用到该方法 */
    pub fn new(elf_data: &[u8]) -> Arc<Self> {
        /* 构造memory_set */
        let (mut memory_set, user_sp, entry_point) = MemorySet::from_elf_data(elf_data);
        memory_set.push_trapframe(0);

        /* 获得Trapframe所在物理页页号 */
        let trap_cx_ppn = memory_set
            .translate_vpn_to_pte(VirtAddr::from(TRAPFRAME).into()).unwrap().ppn();
        
        /* 分配tid */
        let tid = alloc_tid();
        
        /* 分配内核栈 */
        let kernel_stack = KernelStack::new(tid as i32);

        /* 构造task_context */
        //1， sp <- kstack_top
        //2,  ra <- trap_return。从__switch返回后进入trap_return
        let mut task_context = TaskContext::zero_init();
        task_context.goto_trap_return(kernel_stack.top);
        
        /* 构造trap_context */
        //spec <- entry_point。 设置用户栈
        let trap_context: &mut TrapContext = trap_cx_ppn.get_mut();
        *trap_context = TrapContext::init(
            entry_point,
            user_sp,
            get_kernel_space_satp(),
            kernel_stack.top,
            trap_handler as usize
        );

        /* 构造文件描述符表 */
        let mut fd_table: BTreeMap<u32, Arc<dyn File>> = BTreeMap::new();
        fd_table.insert(0, PTS::new(FileOpenMode::READ).as_file());
        fd_table.insert(1, PTS::new(FileOpenMode::WRITE).as_file());
        fd_table.insert(2, PTS::new(FileOpenMode::WRITE).as_file());

        
        let new_task = Arc::new(TaskControlBlock {
            status: AtomicU8::new(TaskStatus::READY),
            kernel_stack,
            chan: Arc::new(Mutex::new(Channel::new())),

            task_info: Mutex::new(TaskInfo{
                context: task_context,
                trap_cx_ppn,
                set_child_tid   :0,
                clear_child_tid :0,
            }),

            exit       : AtomicBool::new(false),
            fp_modifyed: AtomicBool::new(false),
            interrupted: AtomicBool::new(false),    /* futex是否被中断 */
            signal_context_ptr: Mutex::new(0),
            parent: Mutex::new(None),
            time_info: Mutex::new(TimeStruct::new()),
            sig_mask:  Mutex::new(0),
            t_pending: Mutex::new(SigPending::new()),
            robust_list: Mutex::new(None),

            pid :tid as i32,
            tid :tid as i32,
            private_tid: 0, 

            exit_code   :AtomicI32::new(0),
            exit_signal :AtomicU64::new(0),

            groups  :Arc::new(ThreadGroup::new()),
            memory  :Arc::new(Mutex::new(memory_set)),
            swap    :Arc::new(Mutex::new(SwapList::new())),
            child   :Arc::new(Mutex::new(Vec::new())),
            rlim    :Arc::new(Mutex::new(Default::default())),
            fs_info :Arc::new(Mutex::new(FsStruct { cwd: "/".into() })),
            fd_table:Arc::new(Mutex::new(FdTable { table: fd_table })),
            handlers:Arc::new(Mutex::new(SigHandlers::new())),
            p_pending: Arc::new(Mutex::new(SigPending::new())),
            futex_list: Arc::new(Mutex::new(FutexList::new())),
        });
        new_task.get_thread_group().add_leader(&new_task);
        TASK_NUM.fetch_add(1, Ordering::Relaxed);
        new_task
    }
    
    pub fn init_user_stack(
        &self, 
        sp: usize,
        arg_strings: Vec<String>, 
        env_strings: Vec<String>,
        mut auxv: Vec<Aux>,
     ) -> Result<(usize, usize, usize, usize), Error> {
        trace!("init_user_stack: sp = {:x},
            arg_strings = {:?},
            env_strings = {:?}", sp, arg_strings, env_strings);
        let token = self.get_user_token();
        let mut envp: Vec<usize> = Vec::new();
        let mut arvp: Vec<usize> = Vec::new();
        let mut current_sp = sp;
        //env_string
        for (_, arg) in env_strings.iter().enumerate() {
            current_sp -= 1;
            copyout(token, current_sp as *mut u8, &0)?;
            for byte in arg.as_bytes().iter().rev() {
                current_sp -= 1;
                copyout(token, current_sp as *mut u8, &byte)?;
            }
            envp.push(current_sp);
        }
        current_sp -= current_sp % 8;
    
        //argv_string
        for (_, arg) in arg_strings.iter().enumerate() {
            current_sp -= 1;
            copyout(token, current_sp as *mut u8, &0)?;
            for byte in arg.as_bytes().iter().rev() {
                current_sp -= 1;
                copyout(token, current_sp as *mut u8, &byte)?;
            }
            arvp.push(current_sp);
        }
        current_sp -= current_sp % 16;
    
        
        //auxv
        auxv.push(Aux{aux_type: AT_RANDOM, value: current_sp as usize});
        auxv.push(Aux{aux_type: AT_EXECFN, value: current_sp as usize}); // file name
        auxv.push(Aux{aux_type: AT_NULL, value:0 as usize});    // end
        current_sp = current_sp - (auxv.len())*core::mem::size_of::<usize>()*2;
        let auxv_ptr = current_sp as *mut usize;
        for(idx, _) in auxv.iter().enumerate(){
            unsafe{
                copyout(token, auxv_ptr.add(idx*2),&auxv[idx].aux_type)?;
                copyout(token, auxv_ptr.add(idx*2+1),&auxv[idx].value)?;
            }
        }
        current_sp -= current_sp % 8;
    
        //env
        current_sp = current_sp - (env_strings.len() + 1) * core::mem::size_of::<usize>();
        let envp_ptr = current_sp as *mut usize;                //envp指针, 指向envp[0]
        for (idx, _) in envp.iter().enumerate(){
            unsafe{
                copyout(token, envp_ptr.add(idx),&envp[idx])?;
            }
        }
        unsafe {copyout(token, envp_ptr.add(env_strings.len()), &0)?;} //填充0进行分隔
    
        //argv
        current_sp = current_sp - (arg_strings.len()+1) * core::mem::size_of::<usize>();
        let arvp_ptr = current_sp as *mut usize;                //arvp指针, 指向arvp[0]
        
        for (idx, _) in arvp.iter().enumerate(){
            unsafe{
                copyout(token, arvp_ptr.add(idx),&arvp[idx])?;
            }
        }
        unsafe {copyout(token, arvp_ptr.add(arg_strings.len()), &0)?;}
    
        //argc
        let argc = arg_strings.len();
        current_sp = current_sp - core::mem::size_of::<usize>();
        copyout(token, current_sp as *mut usize, &argc)?;

        Ok((current_sp, arvp_ptr as usize, envp_ptr as usize, auxv_ptr as usize))
     }
    /* 负责sys_exec的大部分工作 */
   pub fn exec(
        &self, 
        elf_file: Arc<dyn File>, 
        arg_strings: Vec<String>, 
        env_strings: Vec<String>
    ) -> Result<isize, Error> {
     /* 根据elf_data构造新的memory_set */
    let (mut memory_set, user_sp, entry_point,mut auxv) = MemorySet::from_elf_file(elf_file);
    memory_set.push_trapframe(self.private_tid);

    let trap_cx_ppn = memory_set
        .translate_vpn_to_pte(VirtAddr::from(TRAPFRAME).into())
        .unwrap()
        .ppn();

    *self.get_memory() = memory_set;
    /* 初始化用户栈 */
    let argc = arg_strings.len();
    let (current_sp, arvp_ptr, envp_ptr, auxv_ptr) = 
        self.init_user_stack(user_sp, arg_strings, env_strings, auxv)?;

    /* execve需要清空sig_handlers */
    *self.handlers.lock() = SigHandlers::new();;

    /* 重新设置trapframe内容 */
    let mut info = self.task_info.lock();
    info.trap_cx_ppn = trap_cx_ppn;
    let trap_cx = info.get_trap_cx();
    *trap_cx = TrapContext::init(
        entry_point,
        current_sp,
        get_kernel_space_satp(),
        self.kernel_stack.top,
        trap_handler as usize
    ); 
    
    trap_cx.x[11] = arvp_ptr;      //trap_cx.a1 = argv
    trap_cx.x[12] = envp_ptr;      //trap_cx.a2 = envp
    trap_cx.x[13] = auxv_ptr;      //trap_cx.a2 = envp
    //Ok(argc as isize)              //trap_cx.a0 = argc
    Ok(0)
    }

    fn copy_memory(&self, clone_flags: CloneFlags) -> Arc<Mutex<MemorySet>> {
        if clone_flags.contains(CloneFlags::VM) {
            self.memory.clone()
        } else {
            Arc::new(Mutex::new(
                MemorySet::from_existed_memory_set(&mut self.get_memory())
            ))
        }
    }

    fn copy_fd_table(&self, clone_flags: CloneFlags) -> Arc<Mutex<FdTable>> {
        if clone_flags.contains(CloneFlags::FILES) {
            self.fd_table.clone()
        } else {
            Arc::new(Mutex::new(
                FdTable { table: self.fd_table.lock().table.clone()}    
            ))
        }
    }

    fn copy_fs(&self, clone_flags: CloneFlags) -> Arc<Mutex<FsStruct>> {
        if clone_flags.contains(CloneFlags::FS) {
            self.fs_info.clone()
        } else {
            Arc::new(Mutex::new(
                FsStruct { cwd: self.fs_info.lock().cwd.clone() }
            ))
        }
    }

    fn copy_sighander(&self, clone_flags: CloneFlags) -> Arc<Mutex<SigHandlers>> {
        if clone_flags.contains(CloneFlags::SIGHAND) || clone_flags.contains(CloneFlags::THREAD) {
            self.handlers.clone()
        } else {
            Arc::new(Mutex::new(
                SigHandlers { table: self.handlers.lock().table.clone() }
            ))
        }
    }

    fn copy_signal(&self, clone_flags: CloneFlags) -> Arc<Mutex< SigPending>> {
        if clone_flags.contains(CloneFlags::THREAD) {
            self.p_pending.clone()
        } else {
            Arc::new(Mutex::new(
                SigPending { pendings: self.p_pending.lock().pendings.clone() }
            ))
        }
    }

    /* tid, pid, tgid */
    fn copy_get_id(&self, clone_flags: CloneFlags) -> (i32, i32) {
        let tid = alloc_tid();
        
        let pid = if clone_flags.contains(CloneFlags::THREAD) {
            self.pid
        } else {
            tid
        };

        (tid, pid)
    }

    /* 负责sys_clone的大部分工作 */
    pub fn copy_process(
        self: &Arc<TaskControlBlock>, 
        clone_flags: CloneFlags,
    ) -> Result<Arc<TaskControlBlock>, Error> {
        /* 分配id */
        let (tid, pid) = self.copy_get_id(clone_flags);
        let private_tid = tid - pid;
        /* 分配内核栈 */
        let kernel_stack = KernelStack::new(tid as i32);
        /* 获得memory和trap_cx_ppn */
        let memory = self.copy_memory(clone_flags);
        let mut memory_set = memory.lock();
        if clone_flags.contains(CloneFlags::THREAD) {
            memory_set.push_trapframe(private_tid);
        }
        let trap_cx_ppn = memory_set
            .translate_vpn_to_pte(VirtAddr::from(trapframe_bottom(private_tid)).into())
            .unwrap()
            .ppn();
        if clone_flags.contains(CloneFlags::THREAD) {
            // let trap_cx = trap_cx_ppn.get_mut();
            // *trap_cx = *self.get_trap_cx();
        }
        let trap_cx = trap_cx_ppn.get_mut();
        *trap_cx = *self.get_trap_cx();

        let child_token = memory_set.token();
        drop(memory_set);

        /* 构造context */
        let mut context = TaskContext::zero_init();
        context.goto_trap_return(kernel_stack.top);
                
        /* 构造TaskInfo */
        let task_info = TaskInfo {
            trap_cx_ppn,
            context,
            clear_child_tid: 0,
            set_child_tid: 0   
        };
        let parent;
        let child;
        let groups;
        let rlim;
        let futex_list;
        let swap;
        if clone_flags.contains(CloneFlags::THREAD) {
            parent  = self.parent.lock().clone();
            child   = self.child.clone();
            groups  = self.groups.clone();
            rlim    = self.rlim.clone();
            futex_list   = self.futex_list.clone();
            swap = self.swap.clone();
        } else {
            parent  = Some(Arc::downgrade(self));
            child   = Arc::new(Mutex::new(Vec::new()));
            groups  = Arc::new(ThreadGroup::new());
            rlim    = Arc::new(Mutex::new(rlimit_default()));
            futex_list   = Arc::new(Mutex::new(FutexList::new()));
            swap = Arc::new(Mutex::new(self.swap.lock().clone()));
        }

        /* 构造TCB */
        let task_control_block = Arc::new(TaskControlBlock{
            status: AtomicU8::new(TaskStatus::READY),
            kernel_stack,
            chan: Arc::new(Mutex::new(Channel::new())),

            pid: pid,
            tid: tid,
            private_tid: tid - pid, 

            exit       : AtomicBool::new(false),
            fp_modifyed: AtomicBool::new(self.fp_modifyed.load(Ordering::Relaxed)),
            interrupted: AtomicBool::new(false),
            signal_context_ptr: Mutex::new(0),
            task_info: Mutex::new(task_info),
            parent: Mutex::new(parent),
            time_info: Mutex::new( TimeStruct::new()),
            sig_mask : Mutex::new(*self.sig_mask.lock()),
            t_pending: Mutex::new(SigPending::new()),
            robust_list: Mutex::new(None),

            exit_code: AtomicI32::new(0),
            exit_signal: AtomicU64::new(0),

            memory,
            swap,
            child,
            groups,
            rlim, 
            fs_info: self.copy_fs(clone_flags),
            fd_table: self.copy_fd_table(clone_flags),
            handlers: self.copy_sighander(clone_flags),
            p_pending: self.copy_signal(clone_flags),
            futex_list,
        });
        Ok(task_control_block)
    }
}

impl Drop for TaskControlBlock {
    fn drop(&mut self) {
        let num = TASK_NUM.fetch_sub(1, Ordering::Relaxed);
        info!("TaskControlBlock: {} is dropped, {} tasks are alive", self.tid, num - 1);
    }
}

pub fn trapframe_bottom(tid: i32) -> VirtAddr {
    VirtAddr(TRAMPOLINE - (tid as usize + 1) * PAGE_SIZE)
}
