use crate::cpu::TrapFrame;
use crate::{plic, uart};

#[no_mangle]
extern "C" fn m_trap(epc: usize, tval: usize, cause: usize, hart: usize, status: usize, frame: &mut TrapFrame) -> usize {
	//在M态接管所有traps
	let is_async = {
		if cause >> 63 & 1 == 1 {
			true
		}else{
			false
		}
	};

	let cause_num = cause & 0xfff;
	let mut return_pc = epc;
	if is_async {
		// Asynchronous trap 异步陷入
		match cause_num {
			  3 => {
				  println!("Machine software interrupt CPU#{}", hart);
			  },
			  7 => unsafe {
				  //设置下一次时钟中断的触发
				  let mtimecmp = 0x0200_4000 as *mut u64;
				  let mtime = 0x0200_bff8 as *const u64;
				  //QEMU的频率是10_000_000 Hz, 触发在一秒后
				  mtimecmp.write_volatile(mtime.read_volatile() + 10_000_000);
			  },
			  11 => {
				  // PLIC
				  //println!("Machine external interrupt(PLIC) CPU#{}", hart);
				  if let Some(interrupt) = plic::next() {
					  match interrupt {
						  10 => { //UART中断ID是10
							  let mut my_uart = uart::Uart::new(0x1000_0000);
							  if let Some(c) = my_uart.get() {
								  match c {
									  0x7f => { //0x8 [backspace] ; 而实际qemu运行，[backspace]键输出0x7f, 表示del
										  print!("<<");
									  },
									       10 | 13 => { // 新行或回车
										       println!();
									       },
									       _ => {
										       print!("{}", c as char);
									       },
								  }
							  }
						  },
						     _ => {
							     println!("Non-UART external interrupt: {}", interrupt);
						     }
					  }
					  //这将复位pending的中断，允许UART再次中断。
					  //否则，UART将被“卡住”
					  plic::complete(interrupt);
				  }
			  },
			  _ => {
				  panic!("Unhandled async trap CPU#{} -> {}\n", hart, cause_num);
			  }
		}
	}else{
		// Synchronous trap 同步陷入
		match cause_num {
			2 => {
				// Illegal instruction
				panic!("Illegal instruction CPU#{} -> 0x{:08x}: 0x{:08x}\n", hart, epc, tval);
			},
			/////////
			// ecall指令触发的system call
			8 => {
				// Environment (system) call from User mode
				println!("E-call from User mode! CPU#{} -> 0x{:08x}", hart, epc);
				return_pc += 4;
			},
			9 => {
				// Environment (system) call from Supervisor mode
				println!("E-call from Supervisor mode! CPU#{} -> 0x{:08x}", hart, epc);
				return_pc += 4;
			},
			11 => {
				// Environment (system) call from Machine mode
				panic!("E-call from Machine mode! CPU#{} -> 0x{:08x}\n", hart, epc);
			},
			/////////


			// Page faults
			12 => {
				// Instruction page fault
				println!("Instruction page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
				return_pc += 4;
			},
			13 => {
				// Load page fault
				println!("Load page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
				return_pc += 4;
			},
			15 => {
				// Store page fault
				println!("Store page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
				return_pc += 4;
			},
			_ => {
				panic!("Unhandled sync trap CPU#{} -> {}\n", hart, cause_num);
			}
		}
	}

	//返回更新了的PC
	return_pc
}
