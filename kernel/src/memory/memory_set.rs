use alloc::{
    collections::BTreeMap,
    vec::Vec,
    sync::Arc,
    string::String,
};

use riscv::addr::page;
use spin::Mutex;
use spin::lazy::Lazy;
use riscv::register::satp;
use xmas_elf::program::ProgramHeader;
use core::arch::asm;
use core::ptr::from_raw_parts;
use super::*;
use super::swap::SwapFrame;
use crate::config::*;
use crate::board::*;
use crate::fs::{File, SeekMode};
use crate::memory::pagetable::PTEFlags;
use crate::proc::get_tid;
use crate::proc::{get_current_trap_context, get_current_task, trapframe_bottom, Aux};
use crate::sbi::{sbi_putchar, sbi_remote_sfence_vma_all};
use crate::utils::mem_buffer::MemBuffer;
use crate::utils::{Path, Error, mem_buffer};
use crate::fs::{open, FileOpenMode};
use log::*;


extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss_with_stack();
    fn ebss();
    fn ekernel();
    fn strampoline();
    fn signal_default_handlers();
}

static mut LINKER_BASE_VA   :usize = 0;
static mut LINKER_ENTRY_VA  :usize = 0;
static mut KERNEL_TOKEN     :usize = 0;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MapType {
    Identical,       
    Framed,            
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum LazyFlag {
    MMAP,         //map to a file
    NORMAL,         
}

bitflags! {
    pub struct MapProt: u64 {
        const VALID     = 1 << 0;
        const READ      = 1 << 1;
        const WRITE     = 1 << 2;
        const EXEC      = 1 << 3;
        const USER      = 1 << 4;
        const GLOBAL    = 1 << 5;
        const ACCESS    = 1 << 6;
        const DIRTY     = 1 << 7;
        const SWAP      = 1 << 8;
    }
}

impl MapProt {
    pub fn from_mmap_prot(prot: u32) -> Self {
        const PROT_READ   :u32 =  0x1;      /* Page can be read.  */
        const PROT_WRITE  :u32 =  0x2;      /* Page can be written.  */
        const PROT_EXEC   :u32 =  0x4;      /* Page can be executed.  */
        let mut map_area_prot = MapProt::USER;
        if (prot & PROT_EXEC) != 0 {
            map_area_prot = map_area_prot | MapProt::EXEC;
        }
        if (prot & PROT_READ) != 0 {
            map_area_prot = map_area_prot | MapProt::READ;
        }
        if (prot & PROT_WRITE) != 0 {
            map_area_prot = map_area_prot | MapProt::WRITE;
        }
        map_area_prot
    }
}

pub struct MapArea {
    pub vpn_range: VPNRange,
    pub map_type: MapType,
    pub map_prot: MapProt,
    pub file: Option<Arc<dyn File>>,
    pub offset: usize,
    pub share: bool,
}

bitflags! {
    pub struct MapFlag: u32 {
        const SHARED    = 0x1;
        const PRIVATE   = 0x2;
        const FIXED     = 0x10;
        const ANON      = 0x20;
    }
}

impl MapArea {
    pub fn new(
        start: VirtAddr,
        end: VirtAddr,
        map_type: MapType,
        map_prot: MapProt,    
    ) -> Self {
        Self {
            vpn_range: VPNRange::new(start.floor_page_num(), end.ceil_page_num()),
            map_type,
            map_prot,
            file: None,
            offset: 0,
            share: false,
        }
    }

    pub fn new_pro(
        start: VirtAddr,
        end: VirtAddr,
        map_type: MapType,
        map_prot: MapProt,    
        file: Option<Arc<dyn File>>,
        offset: usize,
        share: bool
    ) -> Self {
        Self {
            vpn_range: VPNRange::new(start.floor_page_num(), end.ceil_page_num()),
            map_prot,
            map_type,
            file:file,
            offset:offset,
            share: share,
        }
    }

    pub fn get_start(&self) -> VirtPageNum {
        self.vpn_range.get_start()
    }

    pub fn get_end(&self) -> VirtPageNum {
        self.vpn_range.get_end()
    }

    pub fn map(&mut self, pagetable: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(vpn, pagetable);           
        }
    }

    pub fn unmap(&mut self, pagetable: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(vpn, pagetable);
        }
    }

    pub fn write_back(&self, pagetable: &mut PageTable) -> Result<(), Error>{
        if self.file.is_none() {
            return Ok(());
        }
        let file = self.file.as_ref().unwrap();
        if !file.writable() {
            return Ok(());
        }
        for vpn in self.vpn_range {
            match pagetable.translate_vpn_to_pte(vpn) {
                None => {},
                Some(pte) => {
                    if !pte.if_valid() {
                        continue;
                    }
                    let data = pte.ppn().get_byte_array();
                    let offset = (vpn - self.vpn_range.get_start()).0
                        * PAGE_SIZE + self.offset;
                    file.seek(offset, SeekMode::SET)?;
                    let mem_buffer = MemBuffer::from_kernel_space(data.as_mut_ptr(), data.len());
                    file.write_from_buffer(mem_buffer)?;
                }
            }
        }
        Ok(())
    }

    pub fn change_prot(
        &mut self, 
        pagetable: &mut PageTable,
        prot: MapProt
    ) -> Result<(), Error> {
        assert!(prot.contains(MapProt::USER));
        assert!(!prot.contains(MapProt::VALID));
        
        self.map_prot = prot;
        for vpn in self.vpn_range {
            pagetable.change_prot(vpn, prot);
        }
        Ok(())
    }

    pub fn map_one(&mut self, vpn: VirtPageNum, pagetable: &mut PageTable) -> Result<PhysPageNum, Error> {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Framed => {
                if let Some(ret) = pagetable.alloc_map(vpn, self.map_prot) {
                    ppn = ret;
                } else {
                    return Err(Error::ENOMEM);
                }
            }
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
                pagetable.map(vpn, ppn, self.map_prot);
            }
        }
        Ok(ppn)
    }

    pub fn unmap_one(&mut self, vpn: VirtPageNum, pagetable: &mut PageTable) {
        if self.map_type == MapType::Framed{
            pagetable.data_frames.remove(&vpn);
        }
        pagetable.unmap(vpn);
    }

    pub fn copy_data (
        &mut self, 
        pagetable: &mut PageTable, 
        data: &[u8], 
        offset: usize,         //将数据复制到起始偏移地址为page_offset处
    ) {
        let mut start: usize = 0;
        let page_offset = offset / PAGE_SIZE;  //页间偏移
        let mut offset = offset % PAGE_SIZE;   //页内偏移
        let mut current_vpn = self.vpn_range.get_start() + VirtPageNum(page_offset);
        let len = data.len();
        loop {
            assert!(offset < PAGE_SIZE);
            let end = len.min(start + PAGE_SIZE - offset);
            let src = &data[start..end];
            let dst = &mut pagetable
                .translate_vpn_to_pte(current_vpn)
                .unwrap()
                .ppn()
                .get_byte_array()[offset..offset + src.len()];
            dst.copy_from_slice(src);

            start += PAGE_SIZE - offset;
            offset = 0;

            current_vpn.step();
            if start >= len {
                break;
            }
        }
    }

    pub fn change_maparea_range(&mut self, start: VirtAddr, end: VirtAddr){
        self.vpn_range = VPNRange::new(start.floor_page_num(),end.ceil_page_num());
    }
    pub fn is_writable(&self) -> bool {
        self.map_prot.contains(MapProt::WRITE)
    }

    pub fn is_readable(&self) -> bool {
        self.map_prot.contains(MapProt::READ)
    }

    pub fn is_user(&self) -> bool {
        self.map_prot.contains(MapProt::USER)
    }

    pub fn is_trapframe(&self) -> bool {
        !self.map_prot.contains(MapProt::USER)
    }

    pub fn is_accessible(&self, is_store: bool) -> bool {
        if is_store == true && !self.is_writable() {
            warn!("Try to write an unwriteable page");
            return false;
        }
        if is_store == false && !self.is_readable() {
            warn!("Try to read an unreadable page");
            return false;
        }
        true
    }

    /* 将map_area分裂，并返回高地址部分 */
    pub fn split_right_pop(&mut self, split_vpn: VirtPageNum, new_prot: MapProt) -> MapArea {
        let new_area = MapArea {
            vpn_range: VPNRange::new(split_vpn,self.vpn_range.get_end()),
            map_type: self.map_type,
            map_prot: new_prot,
            file: self.file.clone(),
            offset: self.offset + PAGE_SIZE* (split_vpn.0 - self.vpn_range.get_start().0),
            share: self.share,
        };
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(),split_vpn);
        new_area
    }

    /* 将map_area分裂，并返回低地址部分 */
    pub fn split_left_pop(&mut self, split_vpn: VirtPageNum, new_prot: MapProt) -> MapArea {
        let new_area = MapArea {
            vpn_range: VPNRange::new(self.vpn_range.get_start(),split_vpn),
            map_type:self.map_type,
            map_prot:new_prot,
            file:self.file.clone(),
            offset:self.offset,
            share: self.share,
        };
        self.offset = self.offset + PAGE_SIZE*(split_vpn.0 - self.vpn_range.get_start().0);
        self.vpn_range = VPNRange::new(split_vpn,self.vpn_range.get_end());
        new_area
    }
}

pub struct MemorySet {
    pub pagetable: PageTable,
    pub areas: Vec<MapArea>,
    pub program_end: VirtAddr,  
    pub current_end: VirtAddr, //当前进程的数据段结束地址
    pub free_memory_bottom: VirtAddr,
}

impl MemorySet {
    fn new() -> Self {
        Self {
            pagetable: PageTable::new(),
            areas: Vec::new(),
            program_end: VirtAddr(0),
            current_end: VirtAddr(0),
            free_memory_bottom: VirtAddr(FREE_MEMORY_TOP),
        }
    }

    pub fn push(&mut self, offset: usize, mut map_area: MapArea, data: Option<&[u8]>) { 
        map_area.map(&mut self.pagetable); 
        if let Some(data) = data {
            map_area.copy_data(&mut self.pagetable, data, offset);
        }  
        self.areas.push(map_area);
    }

    pub fn push_delay(&mut self, map_area: MapArea) {
        self.areas.push(map_area);
    }

    pub fn remove_framed_area_with_start(&mut self, start_va: VirtAddr) {
        if let Some((idx, area)) = self.areas
            .iter_mut()
            .enumerate()
            .find(|(_, area)| 
                area.vpn_range.get_start() == VirtPageNum::from(start_va))
        {
            area.unmap(&mut self.pagetable);
            self.areas.remove(idx);
        }
    }

    pub fn insert_framed_area(
        &mut self, start_va: VirtAddr, end_va: VirtAddr, prot: MapProt
    ){
        self.push(
            0,
            MapArea::new(
            start_va,
            end_va,
            MapType::Framed,
            prot,
        ), None);
    }


    fn map_trampoline(&mut self) {
        self.pagetable.map(
            VirtAddr::from(TRAMPOLINE).into(), 
            PhysAddr::from(strampoline as usize).into(), 
            MapProt::EXEC | MapProt::READ);
    }

    fn map_signal_handler(&mut self) {
        self.pagetable.map(
            VirtAddr::from(SIG_DEFAULT_HANDLER).into(), 
            PhysAddr::from(signal_default_handlers as usize).into(), 
        MapProt::EXEC | MapProt::READ | MapProt::USER);
    }

    pub fn push_trapframe(&mut self, tid: i32) {
        let base = trapframe_bottom(tid);
        self.push(
            0,
            MapArea::new(
                base,
                base + PAGE_SIZE.into(),
                MapType::Framed,
                MapProt::READ | MapProt::WRITE,
            ),
            None,
        );
    }

    pub fn remove_area(
        &mut self, 
        start: VirtAddr, 
        end: VirtAddr,
        write_back: bool
    ) -> Result<(), &'static str> {
        let mut len = self.areas.len();
        let mut i = 0;
        let start = start.floor_page_num();
        let end = end.ceil_page_num();   //不包含end
        let mut finish = false;
        while i < len {
            let area = &mut self.areas[i];
            let area_start = area.get_start();
            let area_end = area.get_end();

            let mut to_remove;
            if area_start < start && area_end > end {
                let mut front = area.split_left_pop(start, area.map_prot);
                let mut end = area.split_right_pop(end, area.map_prot);
                to_remove = self.areas.remove(i);
                self.areas.push(front);
                self.areas.push(end);
                finish = true;
            } else if area_start >= start && area_start < end && area_end >= end {
                to_remove = area.split_left_pop(end, area.map_prot);
                i += 1;
            } else if area_start <= start && area_end > start && area_end <= end {
                to_remove = area.split_right_pop(start, area.map_prot);
                i += 1;
            } else if area_start >= start && area_end <= end {
                to_remove = self.areas.remove(i);
                len -= 1;
            } else {
                i += 1;
                continue;
            }
            
            if write_back && to_remove.is_writable() {
                to_remove.write_back(&mut self.pagetable);
            }
            to_remove.unmap(&mut self.pagetable);
            if finish {
                break;
            }
        }
        Ok(())
    }

    pub fn change_map_prot(
        &mut self,
        start: VirtAddr,
        end: VirtAddr,
        prot: MapProt,
    ) -> Result<(), Error> {
        let mut len = self.areas.len();
        let mut i = 0;
        let start = start.floor_page_num();
        let end = end.ceil_page_num();
        let mut finish = false;

        while i < len {
            let area = &mut self.areas[i];
            let area_start = area.get_start();
            let area_end = area.get_end();
            let mut to_change;

            if area_start < start && area_end > end {
                let mut front = area.split_left_pop(start, area.map_prot);
                let mut end = area.split_right_pop(end, area.map_prot);
                to_change = self.areas.remove(i);
                self.areas.push(front);
                self.areas.push(end);
                finish = true;
            } else if area_start >= start && area_start < end && area_end >= end {
                to_change = area.split_left_pop(end, area.map_prot);
                i += 1;
            } else if area_start <= start && area_end > start && area_end <= end {
                to_change = area.split_right_pop(start, area.map_prot);
                i += 1;
            } else if area_start >= start && area_end <= end {
                to_change = self.areas.remove(i);
                len -= 1;
            } else {
                i += 1;
                continue;
            }

            if !to_change.map_prot.contains(MapProt::USER) {
                return Err(Error::EINVAL);
            }

            to_change.change_prot(&mut self.pagetable, prot)?;
            self.areas.push(to_change);
            if finish {
                break;
            }
        }
        Ok(())
    }


    pub fn push_user_stack(&mut self, user_stack_bottom: VirtAddr) -> VirtAddr {
        let mut user_stack_bottom:usize = user_stack_bottom.into();
        /* guard_page */ 
        user_stack_bottom += PAGE_SIZE;
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;

        let stack_area = MapArea::new(
            user_stack_bottom.into(), 
            user_stack_top.into(),
            MapType::Framed,
            MapProt::READ | MapProt::WRITE | MapProt::USER
        );
        //self.push(0, stack_area, None);
        self.push_delay(stack_area);

        user_stack_top.into()
    }

    pub fn new_kernel() -> Self {
        info!("kernel_space: initing");
        let mut memory_set = MemorySet::new();
        memory_set.map_trampoline();

        info!("kernel_space: stext = {:x}, etext = {:x}", stext as usize, etext as usize);
        memory_set.push(
            0,
            MapArea::new(
                (stext as usize).into(),
                (etext as usize).into(),
                MapType::Identical,
                MapProt::READ | MapProt::EXEC,
            ),
            None
        );

        info!("kernel_space: srodata = {:x}, erodata = {:x}", srodata as usize, erodata as usize);
        memory_set.push(
            0,
            MapArea::new(
                (srodata as usize).into(),
                (erodata as usize).into(),
                MapType::Identical,
                MapProt::READ,
            ),
            None
        );

        info!("kernel_space: sdata = {:x}, edata = {:x}", sdata as usize, edata as usize);
        memory_set.push(
            0,
            MapArea::new(
                (sdata as usize).into(),
                (edata as usize).into(),
                MapType::Identical,
                MapProt::READ | MapProt::WRITE,
            ),
            None
        );

        info!("kernel_space: sbss = {:x}, ebss = {:x}", sbss_with_stack as usize, ebss as usize);
        memory_set.push(
            0,
            MapArea::new(
                (sbss_with_stack as usize).into(),
                (ebss as usize).into(),
                MapType::Identical,
                MapProt::READ | MapProt::WRITE,
            ),
            None
        );
        
        info!("kernel_space: ekernel = {:x}", ekernel as usize);
        memory_set.push(
            0,
            MapArea::new(
                (ekernel as usize).into(),
                (MEMORY_END as usize).into(),
                MapType::Identical,
                MapProt::READ | MapProt::WRITE,
            ),
            None
        );

        info!("mapping MMIO in kernel space");
        for pair in MMIO {
            memory_set.push(
                0,
                MapArea::new(
                    (*pair).0.into(),
                    ((*pair).0 + (*pair).1).into(),
                    MapType::Identical,
                    MapProt::READ | MapProt::WRITE,
                ),
                None
            )
        }

        memory_set
    }

    pub fn from_elf_file(elf_file: Arc<dyn File>) -> (Self, usize, usize, Vec<Aux>) {

        let mut auxv: Vec<Aux>=Vec::new();
        let mut memory_set = MemorySet::new();
        
        memory_set.map_trampoline();
        memory_set.map_signal_handler();

        /* 获取ELF头和ELF段头表 */
        const LEN: usize = 1024;
        let elf_data = elf_file.read(LEN).unwrap();
        let elf = xmas_elf::ElfFile::new(&elf_data.as_slice()).unwrap();
        
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        let ph_count = elf_header.pt2.ph_count();
        
        let mut max_end_vpn = VirtPageNum(0);
        let mut head_va = None; // top va of ELF which points to ELF header
        let mut is_dynamic = false;

        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Dynamic {
                is_dynamic = true;
                //unimplemented!();
            } else if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start: VirtAddr = (ph.virtual_addr() as usize).into();
                let end: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                let offset = start.0 - start.floor_page_num().0 * PAGE_SIZE;
                if head_va.is_none() {
                    head_va = Some(start.0);
                }
                let mut map_prot = MapProt::USER;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {map_prot |= MapProt::READ;}
                if ph_flags.is_write() {map_prot |= MapProt::WRITE;}
                if ph_flags.is_execute() {map_prot |= MapProt::EXEC;}
                let lazy = ph.file_size() == ph.mem_size();
                if lazy {
                    let map_area = MapArea::new_pro(
                        start, end, MapType::Framed, map_prot, Some(elf_file.clone()), ph.offset() as usize, false);
                    max_end_vpn = map_area.vpn_range.get_end();
                    memory_set.push_delay(map_area); 
                } else {
                    let map_area = MapArea::new(start, end, MapType::Framed, map_prot);
                    max_end_vpn = map_area.vpn_range.get_end();
                    elf_file.seek(ph.offset() as usize, SeekMode::SET).unwrap();
                    let data = elf_file.read(ph.file_size() as usize).unwrap();
                    
                    memory_set.push(    
                        offset,
                        map_area,
                        Some(data.as_slice()),
                    );
                }
            }
        }
        // Get ph_head addr for auxv
        let ph_head_addr = head_va.unwrap() + elf.header.pt2.ph_offset() as usize;

        /* get auxv vector */
        auxv.push(Aux{aux_type: 0x21, value: 0 as usize});          //no vdso
        auxv.push(Aux{aux_type: 0x28, value: 0 as usize});          //AT_L1I_CACHESIZE:     0
        auxv.push(Aux{aux_type: AT_PHDR, value: (ph_head_addr as usize)});
        auxv.push(Aux{aux_type: 0x29, value: 0 as usize});          //AT_L1I_CACHEGEOMETRY: 0x0
        auxv.push(Aux{aux_type: 0x2a, value: 0 as usize});          //AT_L1D_CACHESIZE:     0
        auxv.push(Aux{aux_type: 0x2b, value: 0 as usize});          //AT_L1D_CACHEGEOMETRY: 0x0
        auxv.push(Aux{aux_type: 0x2c, value: 0 as usize});          //AT_L2_CACHESIZE:      0
        auxv.push(Aux{aux_type: 0x2d, value: 0 as usize});          //AT_L2_CACHEGEOMETRY:  0x0
        auxv.push(Aux{aux_type: AT_HWCAP, value: 0 as usize});
        auxv.push(Aux{aux_type: AT_PAGESZ, value: PAGE_SIZE as usize});
        auxv.push(Aux{aux_type: AT_CLKTCK, value: CLOCK_FREQ as usize});
        auxv.push(Aux{aux_type: AT_PHENT, value: elf.header.pt2.ph_entry_size() as usize});// ELF64 header 64bytes
        auxv.push(Aux{aux_type: AT_PHNUM, value: ph_count as usize});
        auxv.push(Aux{aux_type: AT_FLAGS, value: 0 as usize});
        auxv.push(Aux{aux_type: AT_ENTRY, value: elf.header.pt2.entry_point() as usize});
        auxv.push(Aux{aux_type: AT_UID, value: 0 as usize});
        auxv.push(Aux{aux_type: AT_EUID, value: 0 as usize});
        auxv.push(Aux{aux_type: AT_GID, value: 0 as usize});
        auxv.push(Aux{aux_type: AT_EGID, value: 0 as usize});
        auxv.push(Aux{aux_type: AT_SECURE, value: 0 as usize});

        /* map user stack */
        let max_end_va: VirtAddr = max_end_vpn.into();
        let user_stack_top = memory_set.push_user_stack(max_end_va);

        //the end of the program
        memory_set.program_end = user_stack_top.into();
        memory_set.current_end = memory_set.program_end;
        
        if is_dynamic {
            memory_set.map_linker();
            unsafe {
                auxv.push(Aux{aux_type: AT_BASE, value: LINKER_BASE_VA});    //动态连接器段的偏移
                return (
                    memory_set,
                    user_stack_top.0,
                    LINKER_ENTRY_VA + LINKER_BASE_VA,
                    auxv,
                )
            }
        } else {
            auxv.push(Aux{aux_type: AT_BASE, value: 0 as usize});
            return (
                memory_set,
                user_stack_top.0,
                elf.header.pt2.entry_point() as usize,
                auxv,
            );
        }
    }

    pub fn from_elf_data(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = MemorySet::new();
        
        memory_set.map_trampoline();
        memory_set.map_signal_handler();

        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        let ph_count = elf_header.pt2.ph_count();
        
        let mut max_end_vpn = VirtPageNum(0);

        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();

            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start: VirtAddr = (ph.virtual_addr() as usize).into();
                let end: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                let offset = start.0 - start.floor_page_num().0 * PAGE_SIZE;

                let mut map_prot = MapProt::USER;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {map_prot |= MapProt::READ;}
                if ph_flags.is_write() {map_prot |= MapProt::WRITE;}
                if ph_flags.is_execute() {map_prot |= MapProt::EXEC;}

                let map_area = MapArea::new(start, end, MapType::Framed, map_prot);
                max_end_vpn = map_area.vpn_range.get_end();
                
                memory_set.push(
                    offset,
                    map_area,
                    Some(&elf.input[ph.offset() as usize ..(ph.offset() + ph.file_size()) as usize]),
                )
            }
        }

        /* map user stack */
        let max_end_va: VirtAddr = max_end_vpn.into();
        let user_stack_top = memory_set.push_user_stack(max_end_va);

        //the end of the program
        memory_set.program_end = user_stack_top.into();
        memory_set.current_end = memory_set.program_end;

        return (
            memory_set,
            user_stack_top.0,
            elf.header.pt2.entry_point() as usize,
        );
    }

    pub fn from_existed_memory_set(existed: &mut MemorySet) -> MemorySet {
       let mut memory_set = MemorySet::new();
       memory_set.program_end = existed.program_end;
       memory_set.current_end = existed.current_end;
       memory_set.free_memory_bottom = existed.free_memory_bottom;

       memory_set.map_trampoline();
       memory_set.map_signal_handler();

       for area in existed.areas.iter() {
            let new_file = match &area.file {
                None => None,
                Some(file) => Some(file.copy())
            };
            let mut new_area = MapArea::new_pro(
                VirtAddr::from(area.vpn_range.get_start()),
                VirtAddr::from(area.vpn_range.get_end()),
                area.map_type,
                area.map_prot,
                new_file,
                area.offset,
                false,
            );

            memory_set.areas.push(new_area);
        }

        memory_set.pagetable.copy_from_existed(&mut existed.pagetable);
        let task = get_current_task().unwrap();
        let mut swap = task.swap.lock();
        for (vpn, _) in swap.list.iter() {
            let pte1 = existed.pagetable.find_pte(*vpn).unwrap();
            let pte = memory_set.pagetable.find_pte(*vpn).unwrap();
            pte.set_flag(pte1.flags());
        }


        memory_set
    }

    pub fn activate(&self) {
        let satp = self.pagetable.token();  
        unsafe {
            satp::write(satp);
            asm!("sfence.vma")
        }
    }

    pub fn token(&self) -> usize{
        self.pagetable.token()
    }

    pub fn translate_vpn_to_pte(&self, vpn: VirtPageNum) -> Option<PageTableEntry>{
        self.pagetable.translate_vpn_to_pte(vpn)
    }

    pub fn clear(&mut self) {
        self.areas.clear();
        self.pagetable.frames.clear();
    }

    pub fn find_free_space(&mut self, len: usize) -> Option<VirtAddr> {
        assert!(len % PAGE_SIZE == 0);

        //println!("self.free_memory_bottom = {:x}, len = {:x}", self.free_memory_bottom.0, len);
        if self.free_memory_bottom.0 - len > self.current_end.0 {
            self.free_memory_bottom = VirtAddr(self.free_memory_bottom.0 - len);
            return Some(self.free_memory_bottom);
        }
        let mut stop_vpn: VirtPageNum = VirtPageNum::from(VirtAddr(FREE_MEMORY_TOP).floor_page_num());
        let mut start_vpn: VirtPageNum = stop_vpn;
        let page_len = VirtPageNum(len / PAGE_SIZE + 1);

        'Loop:loop{
            for area in self.areas.iter() {
                if area.vpn_range.get_start() <= start_vpn && start_vpn < area.vpn_range.get_end(){
                    start_vpn = area.vpn_range.get_start() - VirtPageNum(1);
                    stop_vpn = start_vpn;
                    continue 'Loop;
                }
            }
            if start_vpn <= VirtPageNum::from(self.current_end) {
                break;
            }
            if (stop_vpn - start_vpn).0 + 1 == page_len.0 {
                return Some(start_vpn.into());
            }
            
            start_vpn = start_vpn - VirtPageNum(1);
        }
        None
    }

    pub fn load_linker(&mut self) -> Result<(VirtAddr,VirtAddr), Error> {
        let path = Path::from_string(String::from("./libc.so"))?;
        //let path = Path::from_string(String::from("./lib/ld-linux-riscv64-lp64d.so.1"))?;
        let file = open(path,FileOpenMode::SYS)?;
        let size = file.get_size()?;
        let data = file.read(size)?;
        let elf = xmas_elf::ElfFile::new(data.as_slice()).unwrap();
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        let ph_count = elf_header.pt2.ph_count();
        let mut head_va = 0; // top va of ELF which points to ELF header

        let base_vpn = Option::from(VirtAddr::from(DYNAMIC_LINKER).floor_page_num());
        match base_vpn{
            Some(base_vpn) => {
                let base_va:VirtAddr = VirtAddr::from((VirtAddr::from(base_vpn).0));
                for i in 0..ph_count {
                    let ph = elf.program_header(i).unwrap();
        
                    if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                        let start: VirtAddr = (ph.virtual_addr() as usize).into();
                        let end: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                        let offset = start.0 - start.floor_page_num().0 * PAGE_SIZE;
                        if head_va == 0 {
                            head_va = start.0;
                        }
                        let mut map_prot = MapProt::USER;
                        let ph_flags = ph.flags();
                        if ph_flags.is_read() {map_prot |= MapProt::READ;}
                        if ph_flags.is_write() {map_prot |= MapProt::WRITE;}
                        if ph_flags.is_execute() {map_prot |= MapProt::EXEC;}
                        
                        //
                        let mut map_area = MapArea::new(base_va + start,base_va + end, MapType::Framed, map_prot);
                        //map_area.file = Some(file.clone());
                        self.push(
                            offset,
                            map_area,
                            Some(&elf.input[ph.offset() as usize ..(ph.offset() + ph.file_size()) as usize]),
                        )
                    }
                }
                println!("base_va:{:x},entry_va:{:x}",base_va.0 as usize,elf.header.pt2.entry_point() as usize);
                return Ok((base_va, VirtAddr::from(elf.header.pt2.entry_point() as usize)));
            }
            None =>{
                println!("Not enough space for linker! exit!");
                return Err(Error::EFAULT);
            }
        }
    }

    pub fn map_linker(&mut self) {
        let dynamic_linker_vpn_start = VirtAddr::from(DYNAMIC_LINKER).floor_page_num();
        let dynamic_linker_vpn_end = VirtAddr::from(SIG_DEFAULT_HANDLER).floor_page_num();
        let mut kernel_space = &mut *KERNEL_SPACE.lock();
        for area in kernel_space.areas.iter() {
            //寻找动态连接器映射的段进行共享
            if area.vpn_range.get_start() >=  dynamic_linker_vpn_start && area.vpn_range.get_start() < dynamic_linker_vpn_end{
                let mut new_area = MapArea::new_pro(
                    VirtAddr::from(area.vpn_range.get_start()),
                    VirtAddr::from(area.vpn_range.get_end()),
                    area.map_type,
                    area.map_prot,
                    area.file.clone(),
                    area.offset,
                    false,
                );
                for vpn in area.vpn_range {
                    if let Some(frame) = kernel_space.pagetable.data_frames.get_mut(&vpn){
                        self.pagetable.data_frames.insert(vpn, frame.clone());
                        self.pagetable.map(vpn,frame.inner.ppn,new_area.map_prot);

                        let pte = self.pagetable.find_pte(vpn).unwrap();
                        pte.set_cow();
                    }
                }
                self.areas.push(new_area);
            }
        }
    }

    fn find_area(areas: &mut Vec<MapArea>, vpn: VirtPageNum) -> Option<&mut MapArea> {
        areas
        .iter_mut()
        .find(|area| {
            vpn >= area.vpn_range.get_start()
            && vpn < area.vpn_range.get_end()
            && area.is_user()})
    }

    pub fn page_check(&mut self, virtaddr: VirtAddr, is_store: bool) -> Result<PhysPageNum, &'static str>{
        let vpn = virtaddr.floor_page_num();
        let pagetable = &mut self.pagetable;

        let area = Self::find_area(
                &mut self.areas, vpn).ok_or("the virtaddr is out of range")?;

        /* 如果操作与段权限不相符，则错误 */
        if !area.is_accessible(is_store) {
            return Err("Viraddr can't be accessed");
        }

        if let Some(pte) = pagetable.translate_vpn_to_pte(vpn) {
            if pte.is_accessible_by(is_store) {
                return Ok(pte.ppn());

            } else if pte.if_swap() {
                return self.swap_in(vpn);

            } else if !pte.if_valid() {   
                return self.lazy_alloc(vpn, is_store);
                
            } else if pte.if_cow() {
                return Self::copy_on_write(pagetable, vpn);  
            } 

            panic!("should not reach here, pte = {:x?}, {}", pte, is_store);
        } else {
            return self.lazy_alloc(vpn, is_store);
        }    
    }


    fn copy_on_write(
        pagetable: &mut PageTable,
        vpn: VirtPageNum,
    ) -> Result<PhysPageNum, &'static str> {
        let frame = pagetable.data_frames.get(&vpn).unwrap().clone();
        let pte = pagetable.find_pte(vpn).unwrap();
        assert!(!pte.writable());

        let mut flag = pte.flags();
        flag.remove(PTEFlags::C);
        flag.insert(PTEFlags::W);

        if Arc::strong_count(&frame) == 2 {
            assert!(pte.ppn() == frame.ppn);
            pte.set_flag(flag);
            return Ok(frame.ppn);
        }

        let ppn = pagetable
            .alloc_map(vpn, MapProt::from_bits(flag.bits() as u64).unwrap())
            .ok_or("frame is used up")?;
        let dst = ppn.get_byte_array();
        let src = frame.ppn.get_byte_array();
        dst.copy_from_slice(src);

        return Ok(ppn);
    }

    fn lazy_alloc(
        &mut self,
        vpn: VirtPageNum,
        is_store: bool,
    ) -> Result<PhysPageNum, &'static str> {
        let pagetable = &mut self.pagetable;
        let area = Self::find_area(&mut self.areas, vpn)
            .ok_or("Virtaddr is out of range")?;
        
        if !area.is_accessible(is_store) {
            return Err("area is not accessible");
        }
        match &area.file {
            None => {
                let ppn = area.map_one(vpn, pagetable)
                    .or_else(|_| Err("run out of memory"))?;
                return Ok(ppn);
            }
            Some(file) => {
                let mut offset = area.offset + 
                    (vpn.0 - area.get_start().0) * PAGE_SIZE;
                file.seek(offset, SeekMode::SET).unwrap();
                if let Ok(data) = file.read(PAGE_SIZE) {
                    let ppn = area.map_one(vpn, pagetable)
                        .or_else(|_| Err("run out of memory"))?;
                    //trace!("enter herere");
                    ppn.get_byte_array()[0..data.len()].copy_from_slice(data.as_slice());
                    return Ok(ppn);
                } else {
                    return Err("lazy_alloc: fail to read file");
                }
            }
        }
    }

    fn swap_in(&mut self, vpn: VirtPageNum) -> Result<PhysPageNum, &'static str> {
        let pte = self.pagetable.find_pte(vpn).unwrap();
        assert!(!pte.if_valid());
        assert!(!pte.if_cow());
        let area = Self::find_area(&mut self.areas, vpn)
        .ok_or("Virtaddr is out of range")?;
        
        let task = get_current_task().unwrap();
        let mut swap = task.swap.lock();
        let swap_frame = swap.list.remove(&vpn).unwrap();
        drop(swap);

        let ppn = area.map_one(vpn, &mut self.pagetable).unwrap();
        error!("swap_in: tid = {}, vpn = {:x}, ppn = {:x}", get_tid(), vpn.0, ppn.0);
        swap_in_page(ppn, swap_frame);
        Ok(ppn)
    }

    pub fn trace_areas(&self) {
        info!("_____areas_______");
        for (i, area) in self.areas.iter().enumerate() {
            trace!("area{}: start = {:x?}, end = {:x?}, perm = {:b}", i, 
                area.vpn_range.get_start(), area.vpn_range.get_end(), area.map_prot)
        }
        trace!("");
    }

}


pub static KERNEL_SPACE: Lazy<Arc<Mutex<MemorySet>>> = Lazy::new(||{
    let kernel_space = Arc::new(Mutex::new(MemorySet::new_kernel()));
    unsafe {
        KERNEL_TOKEN = kernel_space.lock().token();
    }
    kernel_space
});

pub fn kernel_token() -> usize {
    unsafe {
        KERNEL_TOKEN
    }
}

//将动态链接器加载到内核页表中
pub fn load_dynamic_linker() {
    println!("[kernel] load dynamic linker to memory");
    let (base_va,entry_va):(VirtAddr,VirtAddr) = KERNEL_SPACE.lock().load_linker().unwrap();
    unsafe {
        LINKER_BASE_VA  =  base_va.0;
        LINKER_ENTRY_VA = entry_va.0;
    }
}
