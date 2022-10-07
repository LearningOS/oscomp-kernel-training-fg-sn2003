pub mod vfs;
pub mod file;
pub mod block_cache;
pub mod fat32;
pub mod fifo;
pub mod devfs;
pub mod syslog;
pub mod virt_file;
pub mod procfs;
mod mount_manager;


use syslog::*;
use vfs::*;
use block_cache::{get_block_cache};
use fat32::FAT32FileSystem;
pub use file::*;
pub use mount_manager::*;
pub use devfs::*;
pub use procfs::*;

use log::*;

#[allow(unused)]
pub fn fs_init() {
    println!("[kernel] fs: initing vfs");
    mount_manager::init().expect("monut_manager init fail");
    println!("[kernel] fs: make buffer file at /buf");
    mknod("/buf".into(), FileType::RegularFile, FilePerm::NONE);

    println!("[kernel] fs: make devfs, mount devfs to /dev");
    mknod("/dev".into(), FileType::Directory, FilePerm::NONE);
    mount("/dev".into(), "/".into(), "devfs");  //dev路径为“/”表示不需要块设备

    println!("[kernel] fs: make procfs, mount devfs to /proc");
    mknod("/proc".into(), FileType::Directory, FilePerm::NONE);
    mount("/proc".into(), "/".into(), "procfs");  //dev路径为“/”表示不需要块设备

    println!("[kernel] fs: make syslog at /");
    mknod("/syslog".into(), FileType::RegularFile, FilePerm::NONE);
}
