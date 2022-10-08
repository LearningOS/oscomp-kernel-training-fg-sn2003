#![no_std]
#![no_main]
#![allow(unused)]
#[macro_use]
extern crate user_lib;
#[macro_use]
extern crate alloc;

use user_lib::{*, console::getchar};
use alloc::{string::{String, ToString}, vec::{self, Vec}};
use core::arch::{asm, global_asm};
use user_lib::*;

const LF: u8 = 0x0au8;
const CR: u8 = 0x0du8;
const DL: u8 = 0x7fu8;
const BS: u8 = 0x08u8;
const LINE_START: &str = ">> ";

#[no_mangle]
fn main() -> i32 {
    println!("[initproc] enter initproc");
    if fork() == 0 {
        final_comp();
        //sh();
        shutdown();
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            if pid == -10 {
                //ECHILD
                yield_();
                continue;
            }
            println!(
                "[initproc] Released a zombie process, pid={}, exit_code={}",
                pid, exit_code,
            );
        }
    }
    0
}

fn sh() {
    let pid = fork();
    if pid == 0 {
        exec("/bin/busybox\0", vec!["rm\0", ".ash_history\0"], Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
        exec("/bin/busybox\0", vec!("ash\0"), Vec::new());
    }
}

fn final_comp() {
    //libc_test();
    //run_libc_testcode();
    //run_lua_testcode();
    //run_busybox_testcode();
    run_lmbench_testcode();
}

fn run_libc_testcode() {
    let pid = fork();
    if pid == 0 {
        exec("run-static.sh\0", vec!["run-static.sh\0"], Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
    }

    let pid = fork();
    if pid == 0 {
        exec("run-dynamic.sh\0", vec!["run-dynamic.sh\0"], Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
    }
}

fn run_lua_testcode() {
    let pid = fork();
    if pid == 0 {
        exec("lua_testcode.sh\0", vec!["lua_testcode.sh\0"], Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
    }
}

fn run_busybox_testcode() {
    let pid = fork();
    if pid == 0 {
        exec("busybox_testcode.sh\0", vec!["busybox_testcode.sh\0"], Vec::new());
        //exec("/busybox\0", vec!["find\0", "-name\0", "busybox_cmd.txt\0"], Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
    }
}

fn run_lmbench_testcode() {
    run_lmbench_remain_tests();
    let pid = fork();
    if pid == 0 {
        //Signal handler overhead: 0.0505 microseconds
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_proc\0", "-P\0", "1\0", "fork\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_ctx\0", "-P\0", "1\0", "-s\0", "32\0", "2\0", "4\0","8\0","16\0","24\0","32\0","64\0","96\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_sig\0", "-P\0", "1\0", "prot\0", "lat_sig\0"), Vec::new());
        exec("lmbench_testcode.sh\0", vec!("lmbench_testcode.sh\0"), Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
    }
}

fn run_lmbench_remain_tests() {
    let pid = fork();
    if pid == 0 {
        exec("lmbench_all\0", vec!("lmbench_all\0", "lat_ctx\0", "-P\0", "1\0", "-s\0", "32\0", "2\0", "4\0","8\0","16\0","24\0","32\0","64\0","96\0"), Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
    }
    
    // let pid = fork();
    // if pid == 0 {
    //     exec("lmbench_all\0", vec!("lmbench_all\0", "bw_pipe\0", "-P\0", "1\0"), Vec::new());
    // } else {
    //     let mut status: i32 = 0;
    //     wait(&mut status);
    // }

    let pid = fork();
    if pid == 0 {
        exec("lmbench_all\0", vec!("lmbench_all\0", "bw_file_rd\0", "-P\0", "1\0", "512k\0", "io_only\0", "/var/tmp/XXX\0"), Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
    }

    let pid = fork();
    if pid == 0 {
        exec("lmbench_all\0", vec!("lmbench_all\0", "bw_file_rd\0", "-P\0", "1\0", "512k\0", "open2close\0", "/var/tmp/XXX\0"), Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
    }

    let pid = fork();
    if pid == 0 {
        exec("lmbench_all\0", vec!("lmbench_all\0", "bw_mmap_rd\0", "-P\0", "1\0", "512k\0", "mmap_only\0", "/var/tmp/XXX\0"), Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
    }

    // let pid = fork();
    // if pid == 0 {
    //     exec("lmbench_all\0", vec!("lmbench_all\0", "bw_mmap_rd\0", "-P\0", "1\0", "512k\0", "open2close\0", "/var/tmp/XXX\0"), Vec::new());
    // } else {
    //     let mut status: i32 = 0;
    //     wait(&mut status);
    // }
}

fn libc_test() -> isize {
    println!("libc_test");
    let names = libc_test_name();
    for name in names {
        println!("test = {}", name);
        let pid = fork();
        if pid == 0 {
            let mut test = name.to_string();
            test.push('\0');            
            exec("./runtest.exe\0", vec!("-w\0","entry-dynamic.exe\0", test.as_str()), Vec::new());
            panic!();
        } else {
            wait(&mut 0);
        }
    }

    
    0
}

fn libc_test_name() -> Vec<&'static str> {
    let all = 
"setjmp";

    let names: Vec<&str> = all.splitn(150, '\n').collect();
    names
}
