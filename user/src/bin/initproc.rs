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
    run_libc_testcode();
    run_lua_testcode();
    run_busybox_testcode();
    run_lmbench_remain_tests();
    lmbench_proxy();
    //run_lmbench_testcode();
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
    //lmbench_all lmdd label=\"File /var/tmp/XXX write bandwidth:\" of=/var/tmp/XXX move=1m fsync=1 print=3
    let pid = fork();
    if pid == 0 {
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_ctx\0", "-P\0", "1\0", "-s\0", "32\0", "2\0", "4\0","8\0","16\0","24\0","32\0","64\0","96\0"), Vec::new());
        exec("lmbench_all\0", vec!("lmbench_all\0", "lmdd\0", "label=\"File /var/tmp/XXX write bandwidth:\"", "of=/var/tmp/XXX\0", "move=1m\0", "fsync=1\0", "print=3\0"), Vec::new());

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

fn lmbench_proxy() {
    let cmds = get_cmd_strings();
    for str in cmds {
        let pid = fork();
        if pid == 0 {
            let cmd = parse_str_to_cmds(str.to_string());
            let cmd: Vec<&str> = cmd.iter().map(|s| s.as_str()).collect();
            println!("[debug] cmd = {:?}", cmd);
            exec(cmd[0], cmd, Vec::new());
            //let mut str = str.to_string();
            //str.push(0 as char);
            //exec("./busybox\0", vec!("sh\0", "-c\0", str.as_str()), Vec::new());
            panic!();
        } else {
            wait(&mut 0);
        }
    }
}

fn parse_str_to_cmds(cmd: String) -> Vec<String> {
    let cmd: Vec<&str> = cmd.as_str().splitn(20, ' ').collect();
    let mut words = Vec::new();
    for word in cmd {
        let mut a = word.to_string();
        a.push(0 as char);
        words.push(a);
    }
    words
}

pub fn get_cmd_strings() -> Vec<&'static str> {
    let mut all =
"busybox echo context switch overhead
lmbench_all lat_ctx -P 1 -s 32 2 4 8 16 24 32 64 96
lmbench_all lat_syscall -P 1 null
lmbench_all lat_syscall -P 1 read
lmbench_all lat_syscall -P 1 write
busybox mkdir -p /var/tmp
busybox touch /var/tmp/lmbench
lmbench_all lat_syscall -P 1 stat /var/tmp/lmbench
lmbench_all lat_syscall -P 1 fstat /var/tmp/lmbench
lmbench_all lat_syscall -P 1 open /var/tmp/lmbench
lmbench_all lat_select -n 100 -P 1 file
lmbench_all lat_sig -P 1 install
lmbench_all lat_sig -P 1 catch
lmbench_all lat_sig -P 1 prot lat_sig
lmbench_all lat_pipe -P 1
lmbench_all lat_proc -P 1 fork
lmbench_all lat_proc -P 1 exec
busybox cp hello /tmp
lmbench_all lat_proc -P 1 shell
lmbench_all lat_pagefault -P 1 /var/tmp/XXX
lmbench_all lat_mmap -P 1 512k /var/tmp/XXX
busybox echo Bandwidth measurements
lmbench_all bw_file_rd -P 1 512k io_only /var/tmp/XXX
lmbench_all bw_file_rd -P 1 512k open2close /var/tmp/XXX
lmbench_all bw_mmap_rd -P 1 512k mmap_only /var/tmp/XXX
lmbench_all bw_mmap_rd -P 1 512k open2close /var/tmp/XXX
lmbench_all bw_pipe -P 1
busybox echo file system latency
lmbench_all lat_fs /var/tmp
";
    let mut strings = Vec::new();
    for string in all.lines() {
        strings.push(string);
    }
    strings
}
