# Trap陷入，本质是CPU通知内核的一种方式；
#
# RISCV发生中断时，CPU都会切换到M态，然后跳转到mtvec寄存器所指向的中断处理函数;
#同步中断， 当前执行指令引发的trap, 如非法指令;
#异步中断， 外部引发的，如时钟中断;

//  MXLEN-1                    2 1        0
// |    BASE(MXLEN-1:2](WARL)   |MODE(WARL)|
//          MXLEN-2                  2
//Base: 中断处理函数地址
//Mode: 
//0 -> Direct, 所有中断的pc = Base, 中断处理函数地址需要4-byte对齐
//1 -> Vectored, 异步中断的pc = BASE + 4*cause

.option norvc

#启用备用宏模式
.altmacro
.set NUM_GP_REGS, 32 #每种上下文的寄存器数
.set NUM_FP_REGS, 32
.set REG_SIZE, 8
.set MAX_CPUS, 8

//寄存器
// x0 = zero 常数0
// x1 = ra 返回地址
// x2 = sp 栈指针
// x3 = gp 全局指针
// x4 = tp 线程指针
// x5 ~ x7 = t0 ~ t2 临时存储
// x8 = s0/fp 保存用寄存器/帧指针（配合栈指针界定一个函数的栈）
// x9 = s1 保存用寄存器
// x10 ~ x11 = a0 ~ a1 函数参数/返回值
// x12 ~ x17 = a2 ~ a7 函数参数
// x18 ~ x27 = s2 ~ s11 保存用寄存器
// x28 ~ x31 = t3 ~ t6 临时存储
//
//使用宏来保存和恢复多个寄存器
.macro save_gp i, basereg=t6
	sd x\i, ((\i)*REG_SIZE)(\basereg)
.endm

// t6 + 3*REG_SIZE -> x3
.macro load_gp i, basereg=t6
	ld x\i, ((\i)*REG_SIZE)(\basereg)
.endm
.macro save_fp i, basereg=t6
	fsd f\i, ((NUM_GP_REGS+(\i))*REG_SIZE)(\basereg)
.endm
.macro load_fp i, basereg=t6
	fld f\i, ((NUM_GP_REGS+(\i))*REG_SIZE)(\basereg)
.endm

#(RISC-V compressed instructions)
#指令不可被压缩成16位, 强制trap vector的所有指令为32位
#此时要确保每个trap vector函数地址为4的倍数,因为mtvec寄存器最后两位用来设置模式
.section .text
.global m_trap_vector
.align 4
m_trap_vector:
    #U态的中断: sscratch = kernel_addr;
    #S态的中断: sscratch = 0;
    #
    #交换了t6和mscratch的值
    csrrw t6, mscratch, t6

    #循环, 默认用了t6, 最底下的x31寄存器
    .set i, 1
    .rept 30
    	save_gp %i
	.set i, i+1
    .endr

    #此时保存t6寄存器
    mv t5, t6
    csrr t6, mscratch
    save_gp 31, t5

    #恢复kernel TrapFrame到mscratch
    csrw mscratch, t5

    #准备好6个参数,进入函数m_trap
    csrr a0, mepc
    csrr a1, mtval
    csrr a2, mcause
    csrr a3, mhartid
    csrr a4, mstatus
    mv   a5, t5
    ld   sp, 520(a5) //trap stack
    call m_trap

    #函数返回值到a0
    csrw mepc, a0

    #恢复所有GP寄存器
    #循环运行31次
    csrr t6, mscratch
    .set i, 1
    .rept 31
    	load_gp %i
	.set i, i+1
    .endr

    mret

.global switch_to_user
switch_to_user:
    csrw mscratch, a0

    li t0, 1 << 7 | 1 << 5
    csrw mstatus, t0
    csrw mepc, a1
    csrw satp, a2
    li t1, 0xaaa
    csrw mie, t1
    la t2, m_trap_vector
    csrw mtvec, t2

    sfence.vma

    mv t6, a0
    .set i, 1
    .rept 31
    	load_gp %i, t6
	.set i, i+1
    .endr

    mret

.global make_syscall
make_syscall:
    ecall
    ret

# trap handler
.global asm_trap_vector
asm_trap_vector:
    mret

