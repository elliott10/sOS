use core::ptr::null_mut;

#[repr(usize)]
pub enum SatpMode {
	Off = 0,
	Sv39 = 8,
	Sv48 = 9,
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
}

impl TrapFrame {
	pub const fn zero() -> Self {
		TrapFrame {
			regs: [0; 32],
			fregs:[0; 32],
			satp: 0,
			pc: 0,
			hartid: 0,
			qm: 1,
		}
	}
}

pub static mut KERNEL_TRAP_FRAME: [TrapFrame; 8] = [TrapFrame::zero(); 8];

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

