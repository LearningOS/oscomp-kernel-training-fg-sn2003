use core::arch::asm;

use alloc::collections::{VecDeque, BTreeMap};
use alloc::sync::Arc;
use alloc::{vec::Vec};
use alloc::vec;
use alloc::string::String;
use log::*;
use super::*;
use crate::config::*;
use crate::proc::{get_current_task, exit_current, SwapList, get_tid};
use crate::utils::Error;

//[63:54]   Reserved
//[53:10]   PPN
//[9:0]     flags: 3RSW, D, A, G, U, X, W, R, V
#[derive(Copy, Clone,Debug)]
#[repr(C)]
pub struct PageTableEntry {
    pub bits: usize,
}

bitflags! {
    pub struct PTEFlags: u16 {
        const V = 1;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
        const S = 1 << 8;
        const C = 1 << 9;
    }
}

impl PTEFlags {
    pub fn from_map_prot(map_prot: MapProt) -> Self {
        /* 因为OopS的map属性定义和riscv的PTE标志一致，所以位模式不用改变 */
        assert!(map_prot.bits() >> 10 == 0);
        Self::from_bits(map_prot.bits() as u16).unwrap()
    }
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        Self {
            bits: ppn.0 << 10 | flags.bits() as usize
        }
    }   

    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }

    pub fn ppn(&self) -> PhysPageNum {
        ((self.bits >> 10) & ((1usize << PPN_WIDTH) - 1)).into()
    }

    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits((self.bits & ((1 << 10) - 1)) as u16).unwrap()
    }

    pub fn if_valid(&self) -> bool {
        self.flags().contains(PTEFlags::V)
    }

    pub fn if_swap(&self) -> bool {
        self.flags().contains(PTEFlags::S)
    }

    pub fn if_cow(&self) -> bool {
        self.flags().contains(PTEFlags::C)
    }

    pub fn readable(&self) -> bool {
        self.flags().contains(PTEFlags::R)
    }

    pub fn writable(&self) -> bool {
        self.flags().contains(PTEFlags::W)
    }

    pub fn is_accessible_by(&self, is_store: bool) -> bool {
        if !self.if_user() || !self.if_valid() {
            return false;
        }

        (self.writable() && is_store == true) ||
        (self.readable() && is_store == false)
        
    }

    pub fn executable(&self) -> bool {
        self.flags().contains(PTEFlags::V)
    }

    pub fn if_user(&self) -> bool {
        self.flags().contains(PTEFlags::U)
    }

    pub fn if_leaf(&self) -> bool {
        self.if_valid() && (self.readable() || self.writable() || self.executable())
    }

    pub fn set_cow(&mut self) {
        assert!(self.flags().contains(PTEFlags::U));
        assert!(!self.flags().contains(PTEFlags::S));

        if self.flags().contains(PTEFlags::W) {
            let flag = (self.flags() | PTEFlags::C) & !PTEFlags::W;
            self.set_flag(flag);
        }
    }

    pub fn set_swap(&mut self) {
        /* 物理页帧的引用计数必须为1 */
        let mut flag = self.flags();
        flag.remove(PTEFlags::C);
        flag.remove(PTEFlags::V);
        flag.insert(PTEFlags::S);
        self.set_flag(flag);
    }

    pub fn set_flag(&mut self, flag: PTEFlags) {
        let ppn = self.ppn();
        assert!(flag.bits() >> 10 == 0);
        self.bits = ppn.0 << 10 | flag.bits() as usize;
    }
}


pub struct PageTable {
    root_ppn: PhysPageNum,
    pub frames: Vec<FrameTracker>,
    pub data_frames: BTreeMap<VirtPageNum, FrameTracker>,
}

impl PageTable {
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        Self {
            root_ppn: frame.ppn,
            frames: vec![frame],
            data_frames: BTreeMap::new(),
        }
    }

    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let indexes = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result = None;
        for (i, index) in indexes.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*index];
            if i == 2 {
                result =Some(pte);
                break;
            }
            if !pte.if_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }

    pub fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let indexes = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result = None;
        for (i, index) in indexes.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*index];
            if i == 2 {
                result =Some(pte);
                break;
            }
            if !pte.if_valid() {
                result = None;
                break;
            }
            ppn = pte.ppn();
        }
        result
    }

    pub fn copy_from_existed(
        &mut self,
        existed: &mut PageTable
    ) -> Result<(), Error> {
        for (vpn, frame) in existed.data_frames.iter() {
            let pte = existed.find_pte(*vpn).unwrap();
            assert!(pte.if_valid());
            
            //trapframe
            if !pte.if_user() {
                let src = frame.ppn.get_byte_array();
                let dst = self
                    .alloc_map(*vpn, MapProt::WRITE | MapProt::READ)
                    .ok_or(Error::ENOMEM)?
                    .get_byte_array();
                dst.copy_from_slice(src);
                continue;
            }

            self.data_frames.insert(*vpn, frame.clone());
            pte.set_cow();
            let new_pte = self.find_pte_create(*vpn).ok_or(Error::ENOMEM)?;
            *new_pte = PageTableEntry::new(frame.inner.ppn, pte.flags());
        }
        return Ok(());
    }

    pub fn copy_from_existed2 (
        &mut self,
        existed: &mut PageTable
    ) -> Result<(), Error> {
        for (vpn, frame) in existed.data_frames.iter() {
            let pte = existed.find_pte(*vpn).unwrap();
            assert!(pte.if_valid());
            let src = frame.ppn.get_byte_array();
                
            let dst = self
                .alloc_map(*vpn, MapProt::from_bits(pte.flags().bits() as u64).unwrap())
                .ok_or(Error::ENOMEM)?
                .get_byte_array();
            dst.copy_from_slice(src);
        }
        return Ok(());
    }

    pub fn alloc_map(&mut self, vpn: VirtPageNum, flags: MapProt) -> Option<PhysPageNum> {
        if let Some(frame) = frame_safe_alloc() {
            let pte = self.find_pte_create(vpn).unwrap();
            *pte = PageTableEntry::new(frame.ppn, PTEFlags::from_map_prot(flags | MapProt::VALID));
            let ppn = frame.ppn;
            self.data_frames.insert(vpn, frame);
            return Some(ppn);
        } 
        else if let Some(swap_vpn) = self.pick_page_to_swap() {
            log::set_max_level(log::LevelFilter::Trace);
            let task = get_current_task().unwrap();
            let swap = &mut *task.swap.lock();
            let frame = self.data_frames.get(&swap_vpn).unwrap().clone();
            let ppn = frame.ppn;

            assert!(Arc::strong_count(&frame) == 2);
            self.swap_out(swap_vpn, swap).unwrap();
            assert!(Arc::strong_count(&frame) == 1);
            
            let pte = self.find_pte_create(vpn).unwrap();
            *pte = PageTableEntry::new(ppn, PTEFlags::from_map_prot(flags | MapProt::VALID));
            self.data_frames.insert(vpn, frame);
            return Some(ppn);
        } 
        loop {
            swap_out_task();
            if let Some(frame) = frame_safe_alloc() {
                let pte = self.find_pte_create(vpn).unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::from_map_prot(flags | MapProt::VALID));
                let ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
                return Some(ppn);
            }
        }
        panic!();
    }

    pub fn swap_out(
        &mut self, 
        vpn: VirtPageNum,
        swap: &mut SwapList,
    ) -> Result<(), Error> {
        let frame = self.data_frames.get(&vpn).unwrap();
        
        let pte = self.find_pte(vpn).unwrap();
        if !pte.if_user() {
            return Err(Error::EINVAL);
        }
        pte.set_swap();
        error!("alloc_map: swap_out: tid = {}, vpn = {:x}, ppn = {:x}", get_tid(), vpn.0, pte.ppn().0);
        let swap_frame = alloc_swap_frame().unwrap();
        swap_out_page(pte.ppn(), &swap_frame);
        trace!("after swap_out_page");
        swap.list.insert(vpn, swap_frame);
        self.data_frames.remove(&vpn);
        Ok(())
    }

    fn pick_page_to_swap(&mut self) -> Option<VirtPageNum> {
        let mut p1 = None;      //优先级1: PTE_A为0
        let mut p2 = None;      //优先级2: PTE_G为0
        let mut p3 = None;      //优先级3: 引用计数为1
        for (vpn, frame) in self.data_frames.iter() {
            let pte = self.find_pte(*vpn).unwrap();
            let mut flag = pte.flags();
            assert!(flag.contains(PTEFlags::V));
            if !flag.contains(PTEFlags::U) {
                continue;
            }
            if Arc::strong_count(frame) == 1 {
                p3 = Some(*vpn);
            } else {
                continue;
            }

            if !flag.contains(PTEFlags::D) {
                p2 = Some(*vpn);
            } else {
                flag.remove(PTEFlags::D);
                pte.set_flag(flag);
                continue;
            }
            
            if !flag.contains(PTEFlags::A) {
                p1 = Some(*vpn);
            } else {
                flag.remove(PTEFlags::A);
                pte.set_flag(flag);
                break;
            }
        }

        p1.or(p2).or(p3)
    }

    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: MapProt) {
        let pte = self.find_pte_create(vpn).unwrap();
        *pte = PageTableEntry::new(ppn, 
            PTEFlags::from_map_prot(flags | MapProt::VALID));
    }

    pub fn unmap(&mut self, vpn: VirtPageNum) {
        match self.find_pte(vpn) {
            None => return,
            Some(pte) => {
                *pte = PageTableEntry::empty();
            }
        }
    }

    pub fn change_prot(&mut self, vpn: VirtPageNum, mut prot: MapProt) {
        /* 需要特别注意SWAP位和COW位 */
        if let Some(pte) = self.find_pte(vpn) {
            let flag = pte.flags();
            let mut new_flag = PTEFlags::from_map_prot(prot);
            
            if flag.contains(PTEFlags::C) {
                if new_flag.contains(PTEFlags::W) {
                    new_flag.remove(PTEFlags::W);
                    new_flag.insert(PTEFlags::C);
                } else {
                    new_flag.remove(PTEFlags::C);
                }
            }
            if flag.contains(PTEFlags::S) {
                new_flag.insert(PTEFlags::S);
            }
            new_flag.insert(PTEFlags::V);
            assert!(!(new_flag.contains(PTEFlags::C) && !new_flag.contains(PTEFlags::W)));
            pte.set_flag(new_flag);
        }
    }

    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << PPN_WIDTH) - 1)),
            frames: Vec::new(),
            data_frames: BTreeMap::new(),
        }
    }
    
    //不要用这个方法来判断虚拟页对应的物理内存是否已经分配,因为某些段在创建的时候可能也把页表填上了
    pub fn translate_vpn_to_pte(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }

    pub unsafe fn translate_va_to_pa(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.floor_page_num()).map(|pte| {
            let pa: PhysAddr = pte.ppn().into();
            let pa: usize = pa.into();
            let offset = va.page_offset();
            (pa + offset).into()
        })
    }

    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}

pub fn translate_byte_buffer(
    token: usize,
    ptr: *const u8,
    len: usize,
    is_to_write: bool       //是否会修改返回的byte_buffer
) -> VecDeque<&'static mut [u8]> {
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = VecDeque::new();
    let mut is_successful = true;

    let task = get_current_task().unwrap();
    let mut memory_set = task.get_memory();

    while start < end {
        let start_va = VirtAddr(start);
        let mut vpn = start_va.floor_page_num();
        
        let ppn: PhysPageNum;
        match memory_set.page_check(vpn.into(), is_to_write) {
            Err(string) => {
                memory_set.trace_areas();
                warn!("[kernel] translate_byte_buffer error: {}", string);
                warn!("    start_va:{:x}", start_va.0);
                is_successful = false;
                break;
            },
            Ok(result_ppn) => ppn = result_ppn
        }

        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
         
        if end_va.page_offset() == 0 {
           v.push_back(&mut ppn.get_byte_array()[start_va.page_offset()..]);
        } else {
           v.push_back(&mut ppn.get_byte_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();   
    }

    if !is_successful {
        drop(memory_set);
        drop(task);
        exit_current(-2);
    }
    v
}



// 获取用户空间的字符串，以'\0'标识字符串的结尾
pub fn translate_str(token: usize, ptr: *const u8) -> Result<String, Error> {
    let mut string = Vec::new();

    // let mut buffer = translate_byte_buffer(token, ptr, MAX_STR_LEN, false);
    #[allow(unused_assignments)]
    let mut is_find = false;
    let mut sum_len = 0;

    'a:loop {
        let len = (MAX_STR_LEN - sum_len).min(PAGE_SIZE - ptr as usize % PAGE_SIZE);
        let ptr = unsafe{ptr.add(sum_len)};
        sum_len += len;
        let mut buffer = translate_byte_buffer(token, ptr, len, false);
        
        let slice = buffer.pop_back().unwrap();
        for ch in slice {
            if *ch == 0 {
                is_find = true;
                break 'a;
            } else {
                //string.push(*ch as char);
                string.push(*ch);
            }
        }
    }

    if !is_find {
        Err(Error::ENAMETOOLONG)
    } else {
        let string = String::from_utf8(string).unwrap();    //tofix
        Ok(string)
    }
}

#[allow(unused)]
pub fn copyin<T>(token: usize, dst: &mut T, src: * const T) -> Result<(), Error>{
    let mut src_buffer = translate_byte_buffer(
        token, 
        src as *const u8, 
        core::mem::size_of::<T>(), 
        false);

    let dst_slice = unsafe {
        core::slice::from_raw_parts_mut(
        dst as *mut T as *mut u8,
        core::mem::size_of::<T>())
    };
    
    let mut start_byte = 0;
    loop {
        let src_slice = src_buffer.pop_front().unwrap();  //pop_front
        let src_slice_len = src_slice.len();
        dst_slice[start_byte..start_byte + src_slice_len].copy_from_slice(src_slice);
        start_byte += src_slice_len;
        if src_buffer.len() == 0 {
            break;
        }
    }
    Ok(())
}

pub fn copyout<T>(token: usize, dst: *mut T, src: &T) -> Result<(), Error>{
    //println!("[copyout_size] {}",core::mem::size_of::<T>());
    let mut dst_buffer = translate_byte_buffer(
        token, 
        dst as *const u8, 
        core::mem::size_of::<T>(), 
        true);
    
    let src_slice = unsafe {
        core::slice::from_raw_parts(
            src as *const T as *const u8, 
            core::mem::size_of::<T>())
    };
    
    let mut start_byte = 0;
    loop {
        let dst_slice = dst_buffer.pop_front().unwrap();  //pop_front
        let dst_slice_len = dst_slice.len();
        dst_slice.copy_from_slice(&src_slice[start_byte..start_byte + dst_slice_len]);
        start_byte += dst_slice_len;
        if dst_buffer.len() == 0 {
            break;
        }
    }
    Ok(())
}

pub fn copyin_vec(token: usize, src: *const u8, len: usize) -> Result<Vec<u8>, Error> {
    let mut buffer = Vec::new();
    let slice_vec = translate_byte_buffer(token, src, len, false);
    for slice in slice_vec {
        buffer.extend_from_slice(slice);
    }
    Ok(buffer)
}

pub fn copyout_vec(token: usize, dst: *mut u8, mut data: Vec<u8>) -> Result<(), Error>{    
    let mut slice_vec = translate_byte_buffer(token, dst, data.len(), true);
    for slice in slice_vec.iter_mut().rev() {
        (slice).copy_from_slice(data.split_off(data.len() - slice.len()).as_slice())
    }
    Ok(())
}