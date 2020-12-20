use crate::cpu::{dump_registers,TrapFrame,CONTEXT_SWITCH_TIME};
use crate::{plic, uart};
use crate::syscall::do_syscall;
use crate::sched::schedule;
use crate::rust_switch_to_user;
use crate::page::{virt_to_phys, Table};

#[no_mangle]
extern "C" fn m_trap(epc: usize, tval: usize, cause: usize, hart: usize, _status: usize, frame: *mut TrapFrame) -> usize {
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
				  // CLINT timer
				  /*
				  //设置下一次时钟中断的触发
				  let mtimecmp = 0x0200_4000 as *mut u64; //该寄存器存储下一次触发的时间
				  let mtime = 0x0200_bff8 as *const u64;
				  //QEMU的频率是10_000_000 Hz, 触发在一秒后
				  mtimecmp.write_volatile(mtime.read_volatile() + 10_000_000);
				  */

				  let new_frame = schedule();

				  //time slicing时间切片来进行进程调度, 每秒调度另外一个进程
				  schedule_next_context_switch(1);

				  if new_frame != 0 {
					  rust_switch_to_user(new_frame);
				  }
			  },
			  11 => {
				  // PLIC
				  //println!("Machine external interrupt(PLIC) CPU#{}", hart);
				  plic::handle_interrupt();
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
				//while true {}
			},
			3 => {
				// breakpoint
				println!("BKPT\n\n");
				return_pc += 2;
			},
			/////////
			// ecall指令触发的system call
			//RISCV所有指令都是: 32位或16位压缩指令，ecall没有压缩形式故一直是32位
			8 => {
				// Environment (system) call from User mode
				//println!("E-call from User mode! CPU#{} -> 0x{:08x}", hart, epc);
				unsafe {
					do_syscall(return_pc, frame);
					//注意接下来的进程切换，pc需要正确
					let frame = schedule();
					schedule_next_context_switch(1);
					rust_switch_to_user(frame);
				}
				//return_pc += 4;
			},
			9 => {
				// Environment (system) call from Supervisor mode
				println!("E-call from Supervisor mode! CPU#{} -> 0x{:08x}", hart, epc);
				unsafe {
					do_syscall(return_pc, frame);
					let frame = schedule();
					schedule_next_context_switch(1);
					rust_switch_to_user(frame);
				}
			},
			11 => {
				// Environment (system) call from Machine mode
				panic!("E-call from Machine mode! CPU#{} -> 0x{:08x}\n", hart, epc);
			},
			/////////


			// Page faults
			12 => {
				// Instruction page fault
				unsafe {
				println!("PID:{}, Instruction page fault CPU#{} -> 0x{:08x}: 0x{:08x}",(*frame).pid, hart, epc, tval);
				}

				loop {} //直到我们有个调度器删除的功能
			},
			13 => {
				// Load page fault
				unsafe {
				println!("PID:{}, Load page fault CPU#{}, mstatus: {:#x} -> 0x{:08x}: 0x{:08x}",(*frame).pid, hart, _status, epc, tval);
				}
				dump_registers(frame);
				loop {} //直到我们有个调度器删除的功能
			},
			15 => {
				// Store page fault
				unsafe {
				let mt = (((*frame).satp << 12) & 0xffffffff) as *mut Table;
				let mt = &mut *mt;
				let paddr = virt_to_phys(mt, epc).unwrap(); 
				println!("PID:{}, Store page fault CPU#{}, mstatus: {:#x} -> 0x{:08x}: 0x{:08x}, table:{:p}, paddr:0x{:x}",(*frame).pid, hart, _status, epc, tval, mt, paddr as usize);
				}
				dump_registers(frame);
				loop {} //直到我们有个调度器删除的功能
			},
			_ => {
				panic!("Unhandled sync trap CPU#{} -> {}\n", hart, cause_num);
			}
		}
	}

	//返回更新了的PC
	return_pc
}

pub const MMIO_MTIMECMP: *mut u64 = 0x0200_4000usize as *mut u64;
pub const MMIO_MTIME: *const u64 = 0x0200_BFF8 as *const u64;

pub fn schedule_next_context_switch(qm: u16) {
	unsafe {
		MMIO_MTIMECMP.write_volatile(MMIO_MTIME.read_volatile().wrapping_add(CONTEXT_SWITCH_TIME * qm as u64));
	}
}

