# bootloader for SOS

/*
.option rvc 可以被汇编生成16位宽的压缩指令;
.option norvc 不可以被汇编生成压缩指令

.option push 暂时将当前的选项设置保存起来，从而允许之后使用.option伪操作指定新的选项;
.option pop 将最近保存的选项设置恢复出来重新生效;
*/

.option norvc
.section .data


.section .text.init
.global _start
_start:

.option push
.option norelax
    la gp, _global_pointer
.option pop

#关闭MMU
    csrw satp, zero

### HART(hardware thread)硬件线程
#也有些板子会把mhartid存在hart的a0寄存器；
#这时hart需要等待一个处理器间中断IPI
    csrr t0, mhartid
    bnez t0, 3f #非0核，跳转到3位置执行

#BSS节清零
    la a0, _bss_start
    la a1, _bss_end
    bgeu a0, a1, 2f

1:
# sd: store double word (64 bits)
    sd zero, (a0)
    addi a0, a0, 8
    bltu a0, a1, 1b

2:
# 进入Rust
    #li t5, 0xffff
    #csrw medeleg, t5
    #csrw mideleg, t5

    la sp, _stack_end

    #  置位[12:11]M态的MPP=3, 第7位和3位是使能中断,还需操作mie寄存器
    li t0, (0b11<<11) | (1<<7) | (1<<3)
    # li t0, (0b11<<11)
    csrw mstatus, t0

    #"machine exception program counter" mepc，ret后会执行到mepc
    #la t1, kmain
    la t1, kinit
    csrw mepc, t1

    #触发中断或异常后调用到mtvec
    la t2, m_trap_vector
    csrw mtvec, t2

    li t3, (1<<3)|(1<<7)|(1<<11)
    csrw mie, t3

    #关中断，kinit()初始化
    #csrw mie, zero

#测试获取这些id寄存器
csrr a0, misa
csrr a1, mvendorid
csrr a2, marchid
csrr a3, mimpid
csrr a4, mhartid
csrr a5, mstatus

    #设置返回地址，到时候进入supervisor mode
    la ra, 2f
    mret #跳转到mepc，后进入Rust

2:
#kinit()函数从Rust返回到这

#设置mstatus寄存器
# 0b01 << 11 : Previous protection mode is 1 (MPP=01 [Supervisor]).
# 1 << 7     : Previous machine interrupt-enable bit is 1 (MPIE=1 [Enabled])
# 1 << 5     : Previous interrupt-enable bit is 1 (SPIE=1 [Enabled]).
#只设置之前bits，因为mret指令将会写现在的bits
    li t0, (0b01 << 11) | (1 << 7) | (1 << 5)
    csrw mstatus, t0

    la t2, m_trap_vector
    csrw mtvec, t2

#mie中断使能寄存器
# 0xaaa = MEIP/SEIP and MTIP/STIP and MSIP/SSIP
    li t2, 0xaaa
    csrw mie, t2

#设置S模式状态寄存器sstatus
# 1 << 8    : Supervisor's previous protection mode is 1 (SPP=1 [Supervisor]).
# 1 << 5    : Supervisor's previous interrupt-enable bit is 1 (SPIE=1 [Enabled]).
# 1 << 1    : Supervisor's interrupt-enable bit will be set to 1 after sret.
#只设置之前bits，因为sret指令将会写现在的bits
//    li t0, (1<<8)|(1<<5)
//    csrw sstatus, t0

#设置机器中断委派寄存器mideleg
# 1 << 1   : Software interrupt delegated to supervisor mode
# 1 << 5   : Timer interrupt delegated to supervisor mode
# 1 << 9   : External interrupt delegated to supervisor mode
#默认所有traps（中断或异常）自动举给M态(mode3)，我们委派后告诉CPU只举给S态(mode1)
//    li t2, (1<<1)|(1<<5)|(1<<9)
//    csrw mideleg, t2

#设置S模式中断使能寄存器
# 1 << 1    : Supervisor software interrupt enable (SSIE=1 [Enabled])
# 1 << 5    : Supervisor timer interrupt enable (STIE=1 [Enabled])
# 1 << 9    : Supervisor external interrupt enable (SEIE=1 [Enabled])
//    csrw sie, t2

#设置stvec (supervisor trap vector)寄存器
#本质上这是一个函数指针，最后两位可为00或01
# 00        : All exceptions set pc to BASE
# 01        : Asynchronous interrupts set pc to BASE + 4 x scause
//    la t3, asm_trap_vector
//    csrw stvec, t3

/*
    #由kinit()返回值到a0, 使能MMU
    csrw satp, a0
    #使得MMU去抓取新的SATP寄存器以及相关的页表，而不是老的cache
    sfence.vma

    #进入S态，并使能中断
    sret

另：
置位MMP=01,使能中断，mret后进入rust函数，处于S态
li t0, (1<<11)|(1<<5)
csrw mstatus, t0

*/
la t1, kmain
csrw mepc, t1

la ra, 4f
mret

3:
//停泊的核心都会运行在这; 一般来说，每个核有自己单独的寄存器和L1 Cache；
//设置好只在收到软件中断(SIPI)时唤醒;
//SIPI (Software Intra-Processor Interrupt)是通过写CLINT地址: base_address + hart * 4 ;
//CLINT的MMIO基址0x0200_0000
//对额外的核我们只用来跑用户程序

    la sp, _stack_end
    li t0, 0x10000
    csrr a0, mhartid
    mul t0, t0, a0
    sub sp, sp, t0
    // sp = _stack_end - 0x10000 * mhartid, 每个核hart分成单独的栈

    //在M态，开启中断
    li t0, 0b11 << 11 | (1 << 7)
    csrw mstatus, t0

    //允许MSIP软中断。
    //这样就可以通过hart #0核来唤醒其他核
    li t3, (1 << 3)
    csrw mie, t3

    #准备进入Rust初始化,初始化会给每个核单独的TrapFrame
    la t1, kinit_hart
    csrw mepc, t1

    la t2, m_trap_vector
    csrw mtvec, t2

    //return to 忙等循环
    la ra, 4f
    mret

#忙等循环
4:
#wfi = wait for interrupt, 关闭所有东西 
    wfi
    j 4b
