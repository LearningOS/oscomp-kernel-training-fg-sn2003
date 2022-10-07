use super::*;

impl From<usize> for PhysAddr {
    fn from(s: usize) -> Self{
        Self(s & ((1 << PA_WIDTH) -1))
    }
}

impl From<usize> for PhysPageNum{
    fn from(s: usize) -> Self{
        Self(s & ((1 << PPN_WIDTH) -1))
    }
}

impl From<usize> for VirtAddr{
    fn from(s: usize) -> Self{
        //Self(s & ((1 << PPN_WIDTH) -1))
        Self(s)
    }
}

impl From<usize> for VirtPageNum{
    fn from(s: usize) -> Self{
        Self(s & ((1 << PPN_WIDTH) -1))
    }
}

impl From<PhysAddr> for usize {
    fn from(s: PhysAddr) -> Self {
        s.0
    }
}

impl From<PhysPageNum> for usize {
    fn from(s: PhysPageNum) -> Self {
        s.0
    }
}

impl From<VirtAddr> for usize {
    fn from(s: VirtAddr) -> Self {
        s.0
    }
}

impl From<VirtPageNum> for usize {
    fn from(s: VirtPageNum) -> Self {
        s.0
    }
}

impl From<PhysAddr> for PhysPageNum {
    fn from(s: PhysAddr) -> Self {
        assert_eq!(s.page_offset(), 0);
        s.floor_page_num()
    }
}

impl From<PhysPageNum> for PhysAddr {
    fn from(s: PhysPageNum) -> Self {
        Self(s.0 << PAGE_BITS_WIDTH)
    }
}

impl From<VirtAddr> for VirtPageNum {
    fn from(s: VirtAddr) -> Self {
        //assert_eq!(s.page_offset(), 0);
        s.floor_page_num()
    }
}

impl From<VirtPageNum> for VirtAddr {
    fn from(s: VirtPageNum) -> Self {
        Self(s.0 << PAGE_BITS_WIDTH)
    }
}
