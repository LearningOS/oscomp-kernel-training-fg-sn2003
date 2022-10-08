pub const TICKS_PER_SEC: usize = 10;

/* MEMORY */
pub const USER_STACK_SIZE: usize = 4096 * 256;
pub const KERNEL_STACK_SIZE: usize = 4096 * 32;
pub const KERNEL_HEAP_SIZE: usize = 0x160_000;
pub const MEMORY_END: usize = 0x90_000_000;  
pub const PAGE_SIZE: usize = 4096;
pub const PAGE_BITS_WIDTH: usize = 12;
pub const TRAMPOLINE: usize = (usize::MAX) - PAGE_SIZE + 1;     
pub const TRAPFRAME: usize = TRAMPOLINE - PAGE_SIZE;
pub const SIG_DEFAULT_HANDLER: usize = TRAPFRAME - 0x1000 * PAGE_SIZE;
pub const DYNAMIC_LINKER:usize = SIG_DEFAULT_HANDLER - 0x10000 * PAGE_SIZE;
pub const FREE_MEMORY_TOP: usize = DYNAMIC_LINKER - PAGE_SIZE; 
pub const KERNEL_STACK_BASE: usize = 0xf0_000_000;


/* PROCESS */
pub const MAX_FD_LEN: u32 = 1024;
pub const MAX_CPU_NUM: usize = 16;


/* FILE SYSTEM */
pub const BLOCK_SIZE: usize = 512; 
pub const BLOCK_CACHE_SIZE: usize = 128;
pub const CLUSTER_CACHE_SIZE: usize = 4096;
pub const MAX_LINK_RECURSE: usize = 32;
pub const MAX_FILE_SIZE: usize = 3*1024*1024*1024;
pub const PIPE_BUFFER_SIZE: usize = 512;
pub const SYSLOG_SIZE: usize = 0x1;

//temporary
pub const MAX_STR_LEN: usize = 512;

//utsname
pub const UTSNAME_LEN       : usize = 65;
pub const UTS_SYSNAME       : &'static str = "OopS\0";
pub const UTS_NODENAME      : &'static str = "Oops\0";
pub const UTS_RELEASE       : &'static str = "4.17.0-1-riscv64\0";
pub const UTS_VERSION       : &'static str = "4.1\0";
pub const UTS_MACHINE       : &'static str = "RISC-V64\0";
pub const UTS_DOMAINNAME    : &'static str = "Oops\0";

// Execution of programs
pub const  AT_NULL      : usize = 0 ;    /* end of vector */
//pub const  AT_IGNORE    : usize = 1 ;    /* entry should be ignored */
//pub const  AT_EXECFD    : usize = 2 ;    /* file descriptor of program */
pub const  AT_PHDR      : usize = 3 ;    /* program headers for program */
pub const  AT_PHENT     : usize = 4 ;    /* size of program header entry */
pub const  AT_PHNUM     : usize = 5 ;    /* number of program headers */
pub const  AT_PAGESZ    : usize = 6 ;    /* system page size */
pub const  AT_BASE      : usize = 7 ;    /* base address of interpreter */
pub const  AT_FLAGS     : usize = 8 ;    /* flags */
pub const  AT_ENTRY     : usize = 9 ;    /* entry point of program */
//pub const  AT_NOTELF    : usize = 10;    /* program is not ELF */
pub const  AT_UID       : usize = 11;    /* real uid */
pub const  AT_EUID      : usize = 12;    /* effective uid */
pub const  AT_GID       : usize = 13;    /* real gid */
pub const  AT_EGID      : usize = 14;    /* effective gid */
//pub const  AT_PLATFORM  : usize = 15;    /* string identifying CPU for optimizations */
pub const  AT_HWCAP     : usize = 16;    /* arch dependent hints at CPU capabilities */
pub const  AT_CLKTCK    : usize = 17;    /* frequency at which times() increments */
/* AT_* values 18 through 22 are reserved */
pub const AT_SECURE     : usize = 23;    /* secure mode boolean */
//pub const AT_BASE_PLATFORM : usize = 24;     /* string identifying real platform, may
//                                 * differ from AT_PLATFORM. */
pub const AT_RANDOM     : usize = 25;    /* address of 16 random bytes */
//pub const AT_HWCAP2     : usize = 26;    /* extension of AT_HWCAP */

pub const AT_EXECFN     : usize = 31;   /* filename of program */
/* Pointer to the global system page used for system calls and other
   nice things.  */
//pub const AT_SYSINFO	: usize = 32;
//pub const AT_SYSINFO_EHDR: usize = 	33;

