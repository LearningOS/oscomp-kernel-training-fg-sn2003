    .section .text.entry
    .global _start
    .global _other_core_start
_start:
    #RustSBI: a0 = hartid
    #sp = boor_stack + (a0 + 1) * 4096
    la sp, boot_stack
    mv tp, a0
    addi a0, a0, 1
    slli a0, a0, 15
    add sp, sp, a0
    mv a0, tp
    call rust_main
    
_other_core_start:
    #a1 = hartid
    la sp, boot_stack
    mv tp, a1
    addi a1, a1, 1
    slli a1, a1, 12
    add sp, sp, a1
    mv a0, tp
    call other_main

    .section .bss.stack
    .global boot_stack
boot_stack:
    .space 4096 * 16
    .global boot_stack_top
boot_stack_top: