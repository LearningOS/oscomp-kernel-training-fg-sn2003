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
    let addr = VirtAddr::from(address);

    if address == 0 {
        return Ok(memory_set.current_end.0 as isize);
    }

    if addr < memory_set.program_end {
        return Err(Error::ENOMEM);
    }

    let ceil = addr.ceil_page_num();
    let heap_area = memory_set.areas
        .iter_mut()
        .find(|area| VirtAddr::from(area.vpn_range.get_start())
             == memory_set.program_end);
             
    /* 如果没有heap_area, 就创建一个 */
    if heap_area.is_none() {
        let map_area = MapArea::new(
            memory_set.program_end,
            ceil.into(),
            MapType::Framed,
            MapProt::READ | MapProt::WRITE | MapProt::USER,
        );
        memory_set.push_delay(map_area);
        memory_set.current_end = ceil.into();
    } else {
        let heap_area = heap_area.unwrap();
        if addr < memory_set.current_end {
            let mut delete = heap_area.split_right_pop(ceil, heap_area.map_prot);
            delete.unmap(&mut memory_set.pagetable);
        }
        heap_area.vpn_range.set_end(ceil);
        memory_set.current_end = ceil.into();
    }

    Ok(memory_set.current_end.0 as isize)
}

pub fn sys_munmap(mut start: usize, mut len: usize) -> Result<isize, Error> {
    trace!("sys_munmap: start: {:x}, len = {:x}", start, len);

    if start % PAGE_SIZE != 0 || len % PAGE_SIZE != 0 {
        return Err(Error::EINVAL);
    }
    let task = get_current_task().unwrap();
    let memory_set = &mut *task.get_memory();
    memory_set.remove_area(start.into(), (start + len).into(), true);

    Ok(0)
}


pub fn sys_mmap(
    start: usize, 
    len: usize, 
    prot: u32, 
    flags: u32, 
    fd: i32, 
    off: usize
) -> Result<isize, Error> {
    let flag = MmapFlag::from_bits(flags).ok_or(Error::EINVAL)?;
    let mut map_prot = MapProt::from_mmap_prot(prot);

    trace!("[sys_mmap]: start: {:x}, len: {:x}, prot:{:b}, flag: {:?}, fd: {}, off: {}" ,
        start, len, map_prot, flag, fd, off);

    if (start % PAGE_SIZE) != 0 || (len % PAGE_SIZE) != 0 {
        return Err(Error::EINVAL);
    }

    let task = get_current_task().unwrap();
    let mut memory_set = task.get_memory();
    let mut addr = VirtAddr::from(start);
    let mut share = false;
    let file;

    if flag.contains(MmapFlag::FIXED) {
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
    pub struct MmapFlag: u32 {
        const SHARED    = 0x01;
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