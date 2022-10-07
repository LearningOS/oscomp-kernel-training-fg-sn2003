// BIOS Parameter Block
// at: boot_sector
// range: 0x0B - 0x23   (24 bytes)
#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub struct BPB {
    pub bytes_per_sector        :u16,  //only support 512, otherwise panic
    pub sectors_per_cluster     :u8,
    pub reserved_sectors_num    :u16,  //The number of logical sectors before the first FAT in the file system image. 
    pub fat_num                 :u8,   //Number of File Allocation Tables. Almost always 2;
    pub omit_1                  :u16,  //Maximum number of FAT12 or FAT16 root directory entries. 0 for FAT32
    pub omit_2                  :u16,  //total logical sectors. 0 for FAT32
    pub omit_3                  :u8,   //Media descripto
    pub omit_4                  :u16,  //Logical sectors per File Allocation Table. FAT32 sets this to 0.
    pub omit_5                  :u16,  //Physical sectors per track for disks
    pub omit_6                  :u16,  //Number of heads for disks 
    pub omit_7                  :u32,  //no use
    pub totol_sectors_num       :u32   //Total logical sectors including hidden sectors. 
}

// Extended BIOS Parameter Block
// at: boot_sector
// range: 0x24 - 0x52
#[repr(packed)]
#[derive(Clone, Copy, Debug)]
pub struct EBPB {
    pub sectors_per_table   :u32,
    pub omit_1              :u16, //Drive description / mirroring flags 
    pub omit_2              :u16, //Version
    pub root_dir_cluster    :u32, //Cluster number of root directory start, typically 2 
    pub fsinfo_sector       :u16, //Logical sector number of FS Information Sector, typically 1
    pub omit_3              :u16, //First logical sector number of a copy of the three FAT32 boot sectors, typically 6
    pub reserved            :[u8; 12],
    pub omit_4              :[u8; 24],
} 


