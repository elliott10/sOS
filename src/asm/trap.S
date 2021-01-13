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

#启用备用宏模式, 可以用循环了
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
    #mscratch存放TrapFrame的地址 
    #交换了t6和mscratch的值
    csrrw t6, mscratch, t6

    #循环, 默认用了t6, 最底下的x31寄存器
    .set i, 0
    .rept 31
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
    csrr a5, mscratch

    sd a0, 520(t5) #保存pc

    //la t0, KERNEL_STACK_END //原本的内核栈岂止不是被弄坏啦???
    //ld sp, 0(t0)

//怎么在内联汇编里使用全局变量? KERNEL_TRAP_FRAME[0].trap_stack
//载入 trap stack; 每次栈指针被重置，所以注意如果有了嵌套中断(默认RISCV不支持嵌套中断)，会搞乱这个stack
    ld   sp, 560(t5) 

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

// mret将pc设置为mepc，通过将mstatus的MPIE域复制到MIE来恢复之前的中断使能设置，
// 并将权限模式设置为mstatus的MPP域中的值，即恢复之前的权限。
    mret



//
// 当发生Trap时，CPU硬件自动把控制状态寄存器mstatus中的MIE位置零以禁用中断，并把先前的MIE值保留到 MPIE 中；
// 把发生异常之前的权限模式保留在mstatus的MPP域中，再把权限模式更改为M态；
// 注意：默认的，任何权限模式下发生的异常，控制权都会被移交到M模式的异常处理程序；
// MPP域的编码可以为：11(M态), 01(S态), 00(U态)
//

.global switch_to_user
switch_to_user:
    #Old func: a0 = FrameTrap, a1 = MEPC, a2 = SATP
    #Now: a0 = FrameTrap
    csrw mscratch, a0 //直接就修改了mscratch而没保存可能会出问题呀!!!

# Load program counter
ld		a1, 520(a0)
# Load satp
ld		a2, 512(a0)
# Load processor mode
#ld		a3, 552(a0)
# Pid
#ld		a4, 544(a0)

    # U态MPP(12:11)=00, MPIE = 1 << 7
    li t0, 1 << 7 | 1 << 5

# mode bits a3

    csrw mstatus, t0

    csrw mepc, a1
    csrw satp, a2

    li t1, 0xaaa
    csrw mie, t1

    la t2, m_trap_vector
    csrw mtvec, t2

    #使MMU刷新TLB, 因为多进程时，可以指定PID作为地址空间标识符ASID; 目前不是最高效的方式
    sfence.vma

    #把所有寄存器等载入到当前CPU
    mv t6, a0
    .set i, 1
    .rept 31
    	load_gp %i, t6
	.set i, i+1
    .endr

    #后执行MEPC处的指令
    mret

.global make_syscall
make_syscall:
	# We're setting this up to work with libgloss
	# They want a7 to be the system call number and all parameters
	# in a0 - a5
	mv	a7, a0
	mv	a0, a1
	mv	a1, a2
	mv	a2, a3
	mv	a3, a4
	mv	a4, a5
	mv	a5, a6
	ecall
	ret

.global do_asm
do_asm:
	csrr a0, mstatus
	ret

# trap handler
.global asm_trap_vector
asm_trap_vector:
    mret

