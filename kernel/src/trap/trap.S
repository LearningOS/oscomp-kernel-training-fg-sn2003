    # the layout of trap_context:
    #   0-31:  x[0:31]
    #   32-63: f[0:31]
    #   64:    sstatus
    #   65:    sepc
    #   66:    kernel_satp
    #   67:    kernel_sp
    #   68:    trap_handler
    #   69:    hart_id

    .section .text.trampoline
    .globl uservec
    .globl userret
    .align 2
uservec:
    /* sscratch = trap_frame */
    csrrw sp, sscratch, sp  // sp <- trapframe
    sd x1   , 1*8(sp)
    sd x3   , 3*8(sp)
    sd x4   , 4*8(sp)
    sd x5   , 5*8(sp)
    sd x6   , 6*8(sp)
    sd x7   , 7*8(sp)
    sd x8   , 8*8(sp)
    sd x9   , 9*8(sp)
    sd x10  , 10*8(sp)
    sd x11  , 11*8(sp)
    sd x12  , 12*8(sp)
    sd x13  , 13*8(sp)
    sd x14  , 14*8(sp)
    sd x15  , 15*8(sp)
    sd x16  , 16*8(sp)
    sd x17  , 17*8(sp)
    sd x18  , 18*8(sp)
    sd x19  , 19*8(sp)
    sd x20  , 20*8(sp)
    sd x21  , 21*8(sp)
    sd x22  , 22*8(sp)
    sd x23  , 23*8(sp)
    sd x24  , 24*8(sp)
    sd x25  , 25*8(sp)
    sd x26  , 26*8(sp)
    sd x27  , 27*8(sp)
    sd x28  , 28*8(sp)
    sd x29  , 29*8(sp)
    sd x30  , 30*8(sp)
    sd x31  , 31*8(sp)
    # fsd ft0 , 32*8(sp)
    # fsd ft1 , 33*8(sp)
    # fsd ft2 , 34*8(sp)
    # fsd ft3 , 35*8(sp)
    # fsd ft4 , 36*8(sp)
    # fsd ft5 , 37*8(sp)
    # fsd ft6 , 38*8(sp)
    # fsd ft7 , 39*8(sp)
    # fsd fs0 , 40*8(sp)
    fsd fs1 , 41*8(sp)
    # fsd fa0 , 42*8(sp)
    # fsd fa1 , 43*8(sp)
    # fsd fa2 , 44*8(sp)
    # fsd fa3 , 45*8(sp)
    # fsd fa4 , 46*8(sp)
    # fsd fa5 , 47*8(sp)
    # fsd fa6 , 48*8(sp)
    # fsd fa7 , 49*8(sp)
    # fsd fs2 , 50*8(sp)
    # fsd fs3 , 51*8(sp)
    # fsd fs4 , 52*8(sp)
    # fsd fs5 , 53*8(sp)
    # fsd fs6 , 54*8(sp)
    # fsd fs7 , 55*8(sp)
    # fsd fs8 , 56*8(sp)
    # fsd fs9 , 57*8(sp)
    # fsd fs10, 58*8(sp)
    # fsd fs11, 59*8(sp)
    # fsd ft8 , 60*8(sp)
    # fsd ft9 , 61*8(sp)
    # fsd ft10, 62*8(sp)
    # fsd ft11, 63*8(sp)
    
    csrr t0 , sstatus   
    csrr t1 , sepc
    sd t0   , 64*8(sp)  
    sd t1   , 65*8(sp)
    csrr t2 , sscratch  
    sd t2   , 2*8(sp)
    ld t0   , 66*8(sp)  //kernel_satp
    ld t1   , 68*8(sp)  //trap_handler
    ld tp   , 69*8(sp)  //hart_id
    ld sp   , 67*8(sp)  //kernel_sp
    
    csrw satp, t0
    sfence.vma zero, zero
    jr t1

userret:
    /* a0 = trap_frame, a1 = satp */
    #sfence.vma zero, zero
    csrw satp, a1       
    #sfence.vma

    csrw sscratch, a0   // sscratch <- trap_frame
    mv sp, a0           // sp <- trap_frame

    sd tp, 69*8(sp)     // tp -> trap_frame.hart_id
    ld t0, 64*8(sp)     // t0 <- sstatus
    ld t1, 65*8(sp)     // t1 <- sepc
    #csrw sstatus, t0    
    csrw sepc, t1
    ld x1   , 1*8(sp)
    /* skip sp */
    ld x3   , 3*8(sp)
    ld x4   , 4*8(sp)
    ld x5   , 5*8(sp)
    ld x6   , 6*8(sp)
    ld x7   , 7*8(sp)
    ld x8   , 8*8(sp)
    ld x9   , 9*8(sp)
    ld x10  , 10*8(sp)
    ld x11  , 11*8(sp)
    ld x12  , 12*8(sp)
    ld x13  , 13*8(sp)
    ld x14  , 14*8(sp)
    ld x15  , 15*8(sp)
    ld x16  , 16*8(sp)
    ld x17  , 17*8(sp)
    ld x18  , 18*8(sp)
    ld x19  , 19*8(sp)
    ld x20  , 20*8(sp)
    ld x21  , 21*8(sp)
    ld x22  , 22*8(sp)
    ld x23  , 23*8(sp)
    ld x24  , 24*8(sp)
    ld x25  , 25*8(sp)
    ld x26  , 26*8(sp)
    ld x27  , 27*8(sp)
    ld x28  , 28*8(sp)
    ld x29  , 29*8(sp)
    ld x30  , 30*8(sp)
    ld x31  , 31*8(sp)
    # fld ft0 , 32*8(sp)
    # fld ft1 , 33*8(sp)
    # fld ft2 , 34*8(sp)
    # fld ft3 , 35*8(sp)
    # fld ft4 , 36*8(sp)
    # fld ft5 , 37*8(sp)
    # fld ft6 , 38*8(sp)
    # fld ft7 , 39*8(sp)
    # fld fs0 , 40*8(sp)
    fld fs1 , 41*8(sp)
    # fld fa0 , 42*8(sp)
    # fld fa1 , 43*8(sp)
    # fld fa2 , 44*8(sp)
    # fld fa3 , 45*8(sp)
    # fld fa4 , 46*8(sp)
    # fld fa5 , 47*8(sp)
    # fld fa6 , 48*8(sp)
    # fld fa7 , 49*8(sp)
    # fld fs2 , 50*8(sp)
    # fld fs3 , 51*8(sp)
    # fld fs4 , 52*8(sp)
    # fld fs5 , 53*8(sp)
    # fld fs6 , 54*8(sp)
    # fld fs7 , 55*8(sp)
    # fld fs8 , 56*8(sp)
    # fld fs9 , 57*8(sp)
    # fld fs10, 58*8(sp)
    # fld fs11, 59*8(sp)
    # fld ft8 , 60*8(sp)
    # fld ft9 , 61*8(sp)
    # fld ft10, 62*8(sp)
    # fld ft11, 63*8(sp)
    ld sp   , 2*8(sp)
    sret


    .section .text.signal
    .globl __user_call_sigreturn
# this exec in u mode
__user_call_sigreturn:
    li	a7,139
    ecall