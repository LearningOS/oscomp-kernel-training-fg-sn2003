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
    const ECHILD: isize = 10;
    if fork() == 0 {
        //run_lua_testcode();
        //run_busybox_testcode();
        run_lmbench_testcode();
        shutdown();
    } else {
        loop {
            let mut exit_code: i32 = 0;
            let pid = wait(&mut exit_code);
            if pid == -ECHILD {
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


fn final_comp() {
    run_lua_testcode();
    run_busybox_testcode();
    run_lmbench_testcode();
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
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
    }
}

fn run_lmbench_testcode() {
    let pid = fork();
    if pid == 0 {    
        exec("lmbench_testcode.sh\0", vec!("lmbench_testcode.sh\0"), Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
    }
}


fn sh() {
    let pid = fork();
    if pid == 0 {
        exec("/busybox\0", vec!["rm\0", ".ash_history\0"], Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
        exec("busybox\0", vec!("sh\0"), Vec::new());
    }
}

fn sh_proxy() -> isize {
    let mut line: String = String::new();
    let prefix = vec!("sh\0", "-c\0");
    print!(">> ");
    loop {
        let c = getchar();
        match c {
            LF | CR => {
                println!("");
                if !line.is_empty() {
                    let mut last_cmd = String::from("/busybox ");
                    line.push('\0');
                    last_cmd.push_str(line.as_str());
                    let mut cmd = prefix.clone();
                    cmd.push(last_cmd.as_str());
                    let pid = fork();
                    if pid == 0 {
                        if exec("/busybox\0", cmd, Vec::new()) == -1 {
                            println!("Error when executing!");
                            return -4;
                        }
                        exit(0);
                        unreachable!();
                    } else {
                        let mut exit_code: i32 = 0;
                        let exit_pid = waitpid(pid as usize, &mut exit_code);
                        assert_eq!(pid, exit_pid);
                        //println!("Shell: Process {} exited with code {}", pid, exit_code);
                    }
                    line.clear();
                }
                print!(">> ");
            }
            BS | DL => {
                if !line.is_empty() {
                    print!("{}", BS as char);
                    print!(" ");
                    print!("{}", BS as char);
                    line.pop();
                }
            }
            _ => {
                print!("{}", c as char);
                line.push(c as char);
            }
        }
    }
}




fn lmbench() {
    let pid = fork();
    if pid == 0 {
        //lmbench_all lat_sig -P 1 prot lat_sig
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_sig\0", "-P\0", "1\0", "prot\0", "lat_sig\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_pipe\0", "-P\0", "1\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lmdd\0", "label=\"File /XXX3 write bandwidth:\"\0", "of=/XXX3\0", "move=1m\0", "fsync=1\0", "print=3\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "bw_file_rd\0", "-P\0", "1\0", "512k\0", "io_only\0", "XXX\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_fs\0", "/var/tmp\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_mmap\0", "-P\0", "1\0", "512k\0", "/var/tmp/XXX\0"), Vec::new());
        exec("lmbench_all\0", vec!["lmbench_all\0","lat_ctx\0","-P\0","1\0","-s\0", "32\0","2\0", "4\0", "8\0", "16\0", "24\0", "32\0", "64\0"], Vec::new());
        //exec("lmbench_all\0", vec!["lmbench_all\0","lat_ctx\0","-P\0","1\0","-s\0", "32\0", "64\0"], Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_syscall\0", "-P\0", "1\0", "null\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_pagefault\0", "-P\0", "1\0", "xxx1\0"), Vec::new());        
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_syscall\0", "-P\0", "1\0", "fstat\0", "lua\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_syscall\0", "-P\0", "1\0", "stat\0", "lua\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "bw_mmap_rd\0", "-P\0", "1\0", "128k\0", "mmap_only\0", "XXX\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lmdd\0", "label=\"File /X write bandwidth:\"\0", "of=/X move=645m\0", "fsync=1\0", "print=3\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "lat_pipe\0", "-P\0", "1\0"), Vec::new());
        //exec("lmbench_all\0", vec!("lmbench_all\0", "bw_pipe\0", "-P\0", "1\0", "-M\0", "12k\0"), Vec::new());
    } else {
        let mut status: i32 = 0;
        wait(&mut status);
    }
}


