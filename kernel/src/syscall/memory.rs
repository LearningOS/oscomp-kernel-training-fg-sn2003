use core::panic;

use crate::proc::{
    get_current_task,
};
use crate::memory::{
        MapArea,VirtAddr,MapType,VirtPageNum,translate_byte_buffer,LazyFlag,frame_alloc,FrameTracker, PTEFlags, MapProt, MemorySet
};
use crate::sbi::{sbi_putchar, sbi_remote_sfence_vma_all};
use alloc::collections::{BTreeMap};
use crate::config::*;
use crate::fs::{SeekMode, File};
use crate::utils::{Error, error};
use alloc::sync::Arc;
use alloc::vec::Vec;
use log::*;


pub fn sys_brk(address: usize) -> Result<isize, Error> {
    trace!("sys_brk, address = {:x}", address);
    let task =  get_current_task().unwrap();
    let mut memory_set = &mut *task.get_memory();
    //println!("\ncurrent_end:0x{:x}\n",memory_set.current_end.0);
    let addr = VirtAddr::from(address);

    if address == 0 || addr == memory_set.current_end{
        return Ok(memory_set.current_end.0 as isize);
    }

    if addr < memory_set.program_end {
        return Err(Error::ENOMEM);
    }

    let ceil = addr.ceil_page_num();
    //let heap_area = memory_set.areas
    //    .iter_mut()
    //    .find(|area| VirtAddr::from(area.vpn_range.get_start())
    //         == memory_set.program_end);
             
    /* 如果没有heap_area, 就创建一个 */
    if addr > memory_set.current_end{
        let map_area = MapArea::new(
            memory_set.current_end,
            ceil.into(),
            MapType::Framed,
            MapProt::READ | MapProt::WRITE | MapProt::USER,
        );
        memory_set.push_delay(map_area);
        memory_set.current_end = ceil.into();
    }
    if addr < memory_set.current_end {
        //let mut delete = heap_area.split_right_pop(ceil, heap_area.map_prot);
        memory_set.remove_area(addr, memory_set.current_end, false);
    }
        //println!("heap_area = {:x?}, prot = {:?}", heap_area.vpn_range, heap_area.map_prot);    

    memory_set.current_end = ceil.into();


    Ok(memory_set.current_end.0 as isize)
}

pub fn sys_munmap(mut start: usize, mut len: usize) -> Result<isize, Error> {
    trace!("sys_munmap: start: {:x}, len = {:x}", start, len);

    if start % PAGE_SIZE != 0  {
        println!("start = {:x} len = {:x}",start,len);
        panic!();
        return Err(Error::EINVAL);
    }
    if len % PAGE_SIZE !=0 {
        len = (len/PAGE_SIZE + 1) * PAGE_SIZE;
    }
    let task = get_current_task().unwrap();
    let memory_set = &mut *task.get_memory();
    memory_set.remove_area(start.into(), (start + len).into(), true);

    Ok(0)
}

bitflags! {
    pub struct MmapFlag: u32 {
        const SHARED    = 0x01;
        const PRIVATE   = 0x02;
        const FIXED     = 0x10;
        const ANON      = 0x20;
    }
}



pub fn sys_mmap(
    start: usize, 
    mut len: usize, 
    prot: u32, 
    flags: u32, 
    fd: i32, 
    off: usize
) -> Result<isize, Error> {
    let flag = MmapFlag::from_bits(flags).ok_or(Error::EINVAL)?;
    let mut map_prot = MapProt::from_mmap_prot(prot);

    trace!("[sys_mmap]: start: {:x}, len: {:x}, prot:{:b}, flag: {:?}, fd: {}, off: {}" ,
        start, len, map_prot, flag, fd, off);

    if (start % PAGE_SIZE) != 0 {
        return Err(Error::EINVAL);
    }
    //mmap允许数据长度不页对齐，cc1会传入不对齐的长度
    if (len % PAGE_SIZE) != 0 {
        len = (len/PAGE_SIZE + 1) * PAGE_SIZE;
    }

    let task = get_current_task().unwrap();
    let mut memory_set = task.get_memory();
    let mut addr = VirtAddr::from(start);
    let mut share = false;
    let file;

    if flag.contains(MmapFlag::FIXED) {
        //println!("\nfixed addr:0x{:x}\n",start);
        memory_set.remove_area(start.into(), (start + len).into(), false);
    } else {
        addr = memory_set.find_free_space(len).ok_or(Error::ENOMEM)?;
    }

    if flag.contains(MmapFlag::ANON) || fd == -1 {
        file = None;
    } else {
        let _file = task.get_file(fd as _)?;
        if (!_file.readable() && map_prot.contains(MapProt::READ))
        || (!_file.writable() && map_prot.contains(MapProt::WRITE)) {
            return Err(Error::EACCES);
        }
        file = Some(_file);
    }
    if flag.contains(MmapFlag::SHARED) {
        map_prot |= MapProt::WRITE;
        share = true
    } else {
        share = false
    }
    
    let map_area = MapArea::new_pro(
        addr,
        addr + len.into(),
        MapType::Framed,
        map_prot,
        file,
        off,
        share
    );
    memory_set.push_delay(map_area);
    return Ok(addr.0 as isize);
}


bitflags! {
    pub struct ProtFlag: u32 {
        const NONE  = 0x0;
        const READ  = 0x1;
        const WRITE = 0x2;
        const EXEC  = 0x4;
    }
}

bitflags! {
    pub struct MapFlag: u32 {
        const SHARE     = 0x01;
        const PRIVATE   = 0x02;
        const FIXED     = 0x10;
        const ANON      = 0x20;
    }
}

pub fn sys_msync() -> Result<isize, Error> {
    Ok(0)
}

pub fn sys_mprotect(mut addr: usize, mut len: usize, prot: u32) -> Result<isize, Error>  {
    let start = VirtAddr::from(addr);
    let end = VirtAddr::from(addr + len);

    let prot = MapProt::from_mmap_prot(prot);

    trace!("sys_mprotect: start = {:x}, end = {:x}, prot = {:?}", start.0, end.0, prot);

    let task = get_current_task().unwrap();
    let mut memory_set = task.get_memory();
    memory_set.change_map_prot(start, end, prot)?;
    Ok(0)
}

pub fn sys_madvise(addr:usize,len: usize,advice:i32) -> Result<isize,Error>{
    Ok(0)
}


bitflags! {
    pub struct MremapFlag: u32 {
        const MAYMOVE   = 0x01;
        const FIXED     = 0x02;
        const DONTUNMAP = 0x04;
    }
}

//必须确保remap的段权限一致
pub fn sys_mremap(old_address:usize,old_size:usize,new_size:usize,flags:u32,new_address:usize) -> Result<isize,Error> {
    return Ok(-1);
}