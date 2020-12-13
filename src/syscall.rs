
use crate::cpu::TrapFrame;

pub fn do_syscall(mepc: usize, frame: *mut TrapFrame) -> usize {
	let syscall_number;
	unsafe {
		syscall_number = (*frame).regs[10];
	}
	match syscall_number {
		0 => {
			mepc + 4
		},
		1 => {
			println!("Test syscall");
			mepc + 4
		},
		_ => {
			println!("Unknown syscall number {}", syscall_number);
			mepc + 4
		}
	}
}
