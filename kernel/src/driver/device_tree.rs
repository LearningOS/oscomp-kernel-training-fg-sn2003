use core::slice;

use device_tree::{DeviceTree, Node};
use log::*;

struct DtbHeader {
    magic: u32,
    size: u32,
}

const DEVICE_TREE_MAGIC: u32 = 0xd00dfeed;

fn walk_dt_node(dt: &Node, intc_only: bool) {
    if let Ok(compatible) = dt.prop_str("compatible") {
        println!("compatible = {}", compatible);
    }
    for child in dt.children.iter() {
        walk_dt_node(child, intc_only);
    }
}

pub fn init(dtb: usize) {
    // let header = unsafe { &*(dtb as *const DtbHeader) };
    // let magic = u32::from_be(header.magic);
    // if magic == DEVICE_TREE_MAGIC {
    //     let size = u32::from_be(header.size);
    //     let dtb_data = unsafe { slice::from_raw_parts(dtb as *const u8, size as usize) };
    //     if let Ok(dt) = DeviceTree::load(dtb_data) {
    //         // find interrupt controller first
    //         walk_dt_node(&dt.root, true);
    //         walk_dt_node(&dt.root, false);
    //     }
    // }
    let device_tree = unsafe {
        DeviceTree::load_from_raw_pointer(dtb as _).expect("load the device tree fail")
    };
    device_tree.root.walk(&mut |node: &Node| {
        println!("node = {:?}", node);
        true
    });

}