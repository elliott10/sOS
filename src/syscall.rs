use crate::cpu::TrapFrame;

pub fn do_syscall(mepc: usize, frame: *mut TrapFrame) -> usize {
	let syscall_number;
	unsafe {
		//x10 = a0 函数参数或返回值
		syscall_number = (*frame).regs[10];
	}
	match syscall_number {
		0 => {
			println!("You called the exit system call!");
			mepc + 4
		},
		1 => {
			println!("Test syscall");
			mepc + 4
		},
		_ => {
			println!("Unknown syscall number {}", syscall_number);
			mepc + 4
			//触发中断的ecall指令是32位，即4字节
		}
	}
}
