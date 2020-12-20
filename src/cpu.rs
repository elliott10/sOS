use core::ptr::null_mut;

// The frequency of QEMU is 10 MHz
pub const FREQ: u64 = 10_000_000;
// Let's do this 250 times per second for switching
pub const CONTEXT_SWITCH_TIME: u64 = FREQ / 500;

#[repr(usize)]
pub enum SatpMode {
	Off = 0,
	Sv39 = 8,
	Sv48 = 9,
}

#[repr(usize)]
pub enum CpuMode {
	User = 0,
	Supervisor = 1,
	Machine = 3,
}

#[repr(usize)]
pub enum Registers {
	Zero = 0,
	Ra,
	Sp,
	Gp,
	Tp,
	T0,
	T1,
	T2,
	S0,
	S1,
	A0, /* 10 */
	A1,
	A2,
	A3,
	A4,
	A5,
	A6,
	A7,
	S2,
	S3,
	S4, /* 20 */
	S5,
	S6,
	S7,
	S8,
	S9,
	S10,
	S11,
	T3,
	T4,
	T5, /* 30 */
	T6
}

pub const fn gp(r: Registers) -> usize {
	r as usize
}

// Floating point registers
#[repr(usize)]
pub enum FRegisters {
	Ft0,
	Ft1,
	Ft2,
	Ft3,
	Ft4,
	Ft5,
	Ft6,
	Ft7,
	Fs0,
	Fs1,
	Fa0, /* 10 */
	Fa1,
	Fa2,
	Fa3,
	Fa4,
	Fa5,
	Fa6,
	Fa7,
	Fs2,
	Fs3,
	Fs4, /* 20 */
	Fs5,
	Fs6,
	Fs7,
	Fs8,
	Fs9,
	Fs10,
	Fs11,
	Ft8,
	Ft9,
	Ft10, /* 30 */
	Ft11
}

//结构体遵循C风格结构
#[repr(C)]
//使得Rust实现Copy和Clone特性
#[derive(Clone, Copy)]
// 会使用mscratch寄存器来保存结构体的信息
pub struct TrapFrame {
	pub regs:   [usize; 32], // 0 - 255
	pub fregs:  [usize; 32], // 256 - 511
	pub satp:   usize, // 512 - 519
	pub pc:     usize, // 520
	pub hartid: usize, // 528
	pub qm:     usize, // 536
	pub pid:    usize, // 544
	pub mode:   usize, // 552
}

impl TrapFrame {
	pub const fn zero() -> Self {
		TrapFrame {
			regs: [0; 32],
			fregs:[0; 32],
			satp:  0,
			pc:    0,
			hartid:0,
			qm:    1,
			pid:   0,
			mode:  0,
		}
	}
}

pub static mut KERNEL_TRAP_FRAME: [TrapFrame; 8] = [TrapFrame::zero(); 8];

//Sv39 mode = 8
pub const fn build_satp(mode: SatpMode, asid: usize, addr: usize) -> usize {
	(mode as usize) << 60
	| (asid & 0xffff) << 44
	| (addr >> 12) & 0xff_ffff_ffff // 保留清了低12位的PPN, 40位？
}

pub fn mhartid_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr $0, mhartid" :"=r"(rval));
		rval
	}
}

pub fn mstatus_write(val: usize) {
	unsafe {
		llvm_asm!("csrw	mstatus, $0" ::"r"(val));
	}
}

pub fn mstatus_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr	$0, mstatus":"=r"(rval));
		rval
	}
}

pub fn stvec_write(val: usize) {
	unsafe {
		llvm_asm!("csrw	stvec, $0" ::"r"(val));
	}
}

pub fn stvec_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr	$0, stvec" :"=r"(rval));
		rval
	}
}

pub fn mscratch_write(val: usize) {
	unsafe {
		llvm_asm!("csrw	mscratch, $0" ::"r"(val));
	}
}

pub fn mscratch_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr	$0, mscratch" : "=r"(rval));
		rval
	}
}

pub fn mscratch_swap(to: usize) -> usize {
	unsafe {
		let from;
		llvm_asm!("csrrw	$0, mscratch, $1" : "=r"(from) : "r"(to));
		from
	}
}

pub fn sscratch_write(val: usize) {
	unsafe {
		llvm_asm!("csrw	sscratch, $0" ::"r"(val));
	}
}

pub fn sscratch_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr	$0, sscratch" : "=r"(rval));
		rval
	}
}

pub fn sscratch_swap(to: usize) -> usize {
	unsafe {
		let from;
		llvm_asm!("csrrw	$0, sscratch, $1" : "=r"(from) : "r"(to));
		from
	}
}

pub fn sepc_write(val: usize) {
	unsafe {
		llvm_asm!("csrw sepc, $0" :: "r"(val));
	}
}

pub fn sepc_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr $0, sepc" :"=r"(rval));
		rval
	}
}

pub fn satp_write(val: usize) {
	unsafe {
		llvm_asm!("csrw satp, $0" :: "r"(val));
	}
}

pub fn satp_read() -> usize {
	unsafe {
		let rval;
		llvm_asm!("csrr $0, satp" :"=r"(rval));
		rval
	}
}

//本质会刷新整个TLB
pub fn satp_fence(vaddr: usize, asid: usize) {
	unsafe {
		llvm_asm!("sfence.vma $0, $1" :: "r"(vaddr), "r"(asid));
	}
}

//这允许我们围住一个特定的进程，而不是整个TLB
pub fn satp_fence_asid(asid: usize) {
	unsafe {
		llvm_asm!("sfence.vma zero, $0" :: "r"(asid));
	}
}

const MMIO_MTIME: *const u64 = 0x0200_BFF8 as *const u64;

pub fn get_mtime() -> usize {
	unsafe { (*MMIO_MTIME) as usize }
}

/// Copy one data from one memory location to another.
pub unsafe fn memcpy(dest: *mut u8, src: *const u8, bytes: usize) {
	let bytes_as_8 = bytes / 8;
	let dest_as_8 = dest as *mut u64;
	let src_as_8 = src as *const u64;

	for i in 0..bytes_as_8 {
		*(dest_as_8.add(i)) = *(src_as_8.add(i));
	}
	let bytes_completed = bytes_as_8 * 8;
	let bytes_remaining = bytes - bytes_completed;
	for i in bytes_completed..bytes_remaining {
		*(dest.add(i)) = *(src.add(i));
	}
}

/// Dumps the registers of a given trap frame. This is NOT the
/// current CPU registers!
pub fn dump_registers(frame: *const TrapFrame) {
	unsafe {
	print!("   ");
	println!("PC:{:#x}, SATP:{:#x}, HARTID:{:#x}, MODE:{:#x}, QM:{:#x}, PID:{:#x}",
		(*frame).pc, (*frame).satp, (*frame).hartid, (*frame).mode, (*frame).qm, (*frame).pid);
	}

	print!("   ");
	for i in 1..32 {
		if i % 4 == 0 {
			println!();
			print!("   ");
		}
		print!("x{:2}:{:08x}   ", i, unsafe { (*frame).regs[i] });
	}
	println!();
}

