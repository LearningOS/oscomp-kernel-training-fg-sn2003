mod convert;
mod simple_range;

pub use convert::*;
pub use simple_range::{StepByOne, VPNRange};
use crate::config::{PAGE_SIZE, PAGE_BITS_WIDTH};
use super::pagetable::*;
use core::ops::Sub;
use core::ops::Add;

pub const PPN_WIDTH :usize = 44;
pub const PA_WIDTH  :usize = 56;
//const VPN_WIDTH :usize = 27;
//const VA_WIDTH  :usize = 39;

//PhysAddr:56bits       PPN:44        Page Offset:12
//[55..0]
#[repr(C)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysAddr(pub usize);


//[55..12]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq,Debug)]
pub struct PhysPageNum(pub usize);


//VirtAddr:39bits       VPN:27        Page Offset:12
//[63:39]位与第38位必须相同，只有最高256GB和最低256G有效
#[repr(C)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtAddr(pub usize);


#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct VirtPageNum(pub usize);


impl PhysAddr {
    pub fn floor_page_num(&self) -> PhysPageNum {
        PhysPageNum(self.0 / PAGE_SIZE)
    }

    pub fn ceil_page_num(&self) -> PhysPageNum {
        PhysPageNum((self.0 + PAGE_SIZE - 1) / PAGE_SIZE)
    }

    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }

    pub fn if_aligned(&self) -> bool {
        self.page_offset() == 0
    }

    pub unsafe fn get_mut<T>(&self) -> &'static mut T {
        (self.0 as *mut T).as_mut().unwrap()
    }
    pub unsafe fn get_ref<T>(&self) -> &'static T {
        (self.0 as *const T).as_ref().unwrap()
    } 

    pub fn get_byte_mut(&self) -> &'static mut u8 {
        unsafe{
            self.get_mut()
        }
    }

    pub fn get_byte_ref(&self) -> &'static u8 {
        unsafe{
            self.get_ref()
        }
    }
}

impl Add for PhysAddr{
    type Output = PhysAddr;

    fn add(self, other: PhysAddr) -> PhysAddr{
        let result = self.0 + other.0;
        PhysAddr(result)
    }
}

impl Sub for PhysAddr{
    type Output = PhysAddr;

    fn sub(self, other: PhysAddr) -> PhysAddr{
        let result = self.0 - other.0;
        PhysAddr(result)
    }
}

impl PhysPageNum {
    //返回物理页帧的可变引用（以字节数组的形式）
    pub fn get_byte_array(&self) -> &'static mut [u8] {
        let pa: PhysAddr = (*self).into();
        unsafe {core::slice::from_raw_parts_mut(pa.0 as *mut u8, PAGE_SIZE)}
    }

    //返回物理页帧的可变引用（以PTE的形式）
    pub fn get_pte_array(&self) -> &'static mut [PageTableEntry] {
        let pa: PhysAddr = (*self).into();
        unsafe {core::slice::from_raw_parts_mut(pa.0 as *mut PageTableEntry, 512)}
    }

    //返回一个放在物理页帧开头的类型为 T 的数据的可变引用
    pub fn get_mut<T>(&self) -> &'static mut T {
        assert!(core::mem::size_of::<T>() < PAGE_SIZE);
        let pa: PhysAddr = (*self).into();
        unsafe {(pa.0 as *mut T).as_mut().unwrap()}
    }
}

impl Add for PhysPageNum {
    type Output = PhysPageNum;

    fn add(self,other:PhysPageNum) -> PhysPageNum{
        let result = self.0 + other.0;
        PhysPageNum(result)
    }
}

impl Sub for PhysPageNum {
    type Output = PhysPageNum;

    fn sub(self, other: PhysPageNum) -> PhysPageNum{
        let result = self.0 - other.0;
        PhysPageNum(result)
    }
}

impl VirtAddr {
    pub fn floor_page_num(&self) -> VirtPageNum {
        VirtPageNum(self.0 / PAGE_SIZE)
    }

    pub fn ceil_page_num(&self) -> VirtPageNum {
        VirtPageNum((self.0 - 1 + PAGE_SIZE ) / PAGE_SIZE)
    }

    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }

    pub fn if_aligned(&self) -> bool {
        self.page_offset() == 0
    }
}

impl Add for VirtAddr {
    type Output = VirtAddr;

    fn add(self, other: VirtAddr) -> VirtAddr{
        let result = self.0 + other.0;
        VirtAddr(result)
    }
}

impl Sub for VirtAddr {
    type Output = VirtAddr;

    fn sub(self, other: VirtAddr) -> VirtAddr{
        let result = self.0 - other.0;
        VirtAddr(result)
    }
}

impl VirtPageNum {
    //idx[2] = vpn[8..0], idx[1] = vpn[17..9], idx[0] = vpn[26..18] 
    pub fn indexes(&self) -> [usize; 3] {
        let mut vpn = self.0;
        let mut idx = [0usize; 3];
        for i in (0..3).rev() {
            idx[i] = vpn & 511;
            vpn >>= 9;
        }
        idx
    }
}

impl Add for VirtPageNum {
    type Output = VirtPageNum;

    fn add(self, other: VirtPageNum) -> VirtPageNum{
        let result = self.0 + other.0;
        VirtPageNum(result)
    }
}

impl Sub for VirtPageNum {
    type Output = VirtPageNum;

    fn sub(self, other: VirtPageNum) -> VirtPageNum{
        let result = self.0 - other.0;
        VirtPageNum(result)
    }
}