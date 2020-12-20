use crate::cpu::{dump_registers, Registers, TrapFrame, gp};
use crate::page::{map, virt_to_phys, EntryBits, Table, PAGE_SIZE, zalloc};
use crate::process::{add_kernel_process_args, delete_process, get_by_pid, set_sleeping, set_waiting, PROCESS_LIST, PROCESS_LIST_MUTEX, Descriptor};
use crate::console::{IN_LOCK, IN_BUFFER, push_queue};

use alloc::{boxed::Box, string::String};

pub unsafe fn do_syscall(mepc: usize, frame: *mut TrapFrame) {
	/*
	let syscall_number;
	unsafe {
		//x10 = a0 函数参数或返回值
		syscall_number = (*frame).regs[10];
	}
	*/
	// A7 is X17, so it's register number 17.
	let syscall_number = (*frame).regs[gp(Registers::A7)];
	// skip the ecall
	(*frame).pc = mepc + 4;
	match syscall_number {
		93 | 94 => {
			// exit and exit_group
			delete_process((*frame).pid as u16);

			//仅仅这样退出进程，出现了：Panicked；会返回到进程中并继续运行
			//需要紧接着进行新进程调度，才不出现该问题
		}
		0 => {
			println!("You called the exit system call!");
		},
		1 => {
		//	println!("Test syscall {}", syscall_number);
		},
		2 => {
			// Easy putchar
			print!("{}", (*frame).regs[Registers::A0 as usize] as u8 as char);
		}
		8 => {
			dump_registers(frame);
		}
		10 => {
			// Sleep
			set_sleeping((*frame).pid as u16, (*frame).regs[Registers::A0 as usize]);
		}
		/*
		11 => {
			// execv
			// A0 = path
			// A1 = argv
			let mut path_addr = (*frame).regs[Registers::A0 as usize];
			// If the MMU is turned on, translate.
			if (*frame).satp >> 60 != 0 {
				let p = get_by_pid((*frame).pid as u16);
				let table = ((*p).mmu_table).as_ref().unwrap();
				path_addr = virt_to_phys(table, path_addr).unwrap();
			}
			// Our path address here is now a physical address. If it came in virtual,
			// it is now physical.
			let path_bytes = path_addr as *const u8;
			let mut path = String::new();
			let mut iterator: usize = 0;
			// I really have to figure out how to change an array of bytes
			// to a string. For now, this is very C-style and mimics strcpy.
			loop {
				let ch = *path_bytes.add(iterator);
				if ch == 0 {
					break;
				}
				iterator += 1;
				path.push(ch as char);
			}
			// See if we can find the path.
			if let Ok(inode) = fs::MinixFileSystem::open(8, &path) {
				let inode_heap = Box::new(inode);

				add_kernel_process_args(exec_func, Box::into_raw(inode_heap) as usize);
				// This deletes us, which is what we want.
				delete_process((*frame).pid as u16);
			}
			else {
				// If we get here, the path couldn't be found, or for some reason
				// open failed. So, we return -1 and move on.
				println!("Could not open path '{}'.", path);
				(*frame).regs[Registers::A0 as usize] = -1isize as usize;
			}
		}
		*/
		17 => { //getcwd
			let mut buf = (*frame).regs[gp(Registers::A0)] as *mut u8;
			let size = (*frame).regs[gp(Registers::A1)];
			let process = get_by_pid((*frame).pid as u16).as_ref().unwrap();
			let mut iter = 0usize;
			if (*frame).satp >> 60 != 0 {
				let table = ((*process).mmu_table).as_mut().unwrap();
				let paddr = virt_to_phys(table, buf as usize);
				if let Some(bufaddr) = paddr {
					buf = bufaddr as *mut u8;
				}
				else {
					(*frame).regs[gp(Registers::A0)] = -1isize as usize;
					return;
				}
			}
			for i in process.data.cwd.as_bytes() {
				if iter == 0 || iter >= size {
					break;
				}
				buf.add(iter).write(*i);
				iter += 1;
			}
		}
		48 => {
		// #define SYS_faccessat 48
			(*frame).regs[gp(Registers::A0)] = -1isize as usize;
		}
		57 => {
			// #define SYS_close 57
			let fd = (*frame).regs[gp(Registers::A0)] as u16;
			let process = get_by_pid((*frame).pid as u16).as_mut().unwrap();
			if process.data.fdesc.contains_key(&fd) {
				process.data.fdesc.remove(&fd);
				(*frame).regs[gp(Registers::A0)] = 0;
			}
			else {
				(*frame).regs[gp(Registers::A0)] = -1isize as usize;
			}
			// Flush?
		}
		63 => { // sys_read
			let fd = (*frame).regs[gp(Registers::A0)] as u16;
			let mut buf = (*frame).regs[gp(Registers::A1)] as *mut u8;
			let size = (*frame).regs[gp(Registers::A2)];
			let process = get_by_pid((*frame).pid as u16).as_mut().unwrap();
			let mut ret = 0usize;
			// If we return 0, the trap handler will schedule
			// another process.
			if fd == 0 { // stdin
				IN_LOCK.spin_lock();
				if let Some(mut inb) = IN_BUFFER.take() {
					let num_elements = if inb.len() >= size { size } else { inb.len() };
					let mut buf_ptr = buf as *mut u8;
					if num_elements == 0 {
						push_queue((*frame).pid as u16);
						set_waiting((*frame).pid as u16);
					}
					else {
						for i in inb.drain(0..num_elements) {
							//使能了MMU
							if (*frame).satp >> 60 != 0 {
								let table = ((*process).mmu_table).as_mut().unwrap();
								let buf_addr = virt_to_phys(table, buf as usize);
								if buf_addr.is_none() {
									break;
								}
								buf_ptr = buf_addr.unwrap() as *mut u8;
								buf_ptr.write(i);
								ret += 1;
								//println!("R: {}", ret);
							}
							buf = buf.add(1);
							buf_ptr = buf_ptr.add(1);
						}
					}
					IN_BUFFER.replace(inb);
				}
				IN_LOCK.unlock();
			}
			(*frame).regs[gp(Registers::A0)] = ret;
		}
		64 => { // sys_write
			let fd = (*frame).regs[gp(Registers::A0)] as u16;
			let buf = (*frame).regs[gp(Registers::A1)] as *const u8;
			let size = (*frame).regs[gp(Registers::A2)];

			let process = get_by_pid((*frame).pid as u16).as_ref().unwrap();
			if fd == 1 || fd == 2 {
				// stdout / stderr
				// println!("WRITE {}, 0x{:08x}, {}", fd, buf as usize, size);
				let mut iter = 0;
				for i in 0..size {
					iter += 1;
					if (*frame).satp >> 60 != 0 {
						let table = ((*process).mmu_table).as_mut().unwrap();
						// We don't need to do the following until we reach a page boundary,
						// however that code isn't written, yet.
						let paddr = virt_to_phys(table, buf.add(i) as usize);
						if let Some(bufaddr) = paddr {
							print!("{}", *(bufaddr as *const u8) as char);
						}
						else {
							println!("Process mmu_table {:p} addr's phys is None!", table);
							break;
						}
					}
				}
				(*frame).regs[gp(Registers::A0)] = iter as usize;
			}
			else {
				let descriptor = process.data.fdesc.get(&fd);
				if descriptor.is_none() {
					(*frame).regs[gp(Registers::A0)] = 0;
					return;
				}
				else {
					let descriptor = descriptor.unwrap();
					match descriptor {
						Descriptor::Framebuffer => {

						}
						Descriptor::File(inode) => {

						
						}
						_ => {
							// unsupported
							(*frame).regs[gp(Registers::A0)] = 0;
						}
					}
				}
			}
		}
		66 => {
			(*frame).regs[gp(Registers::A0)] = -1isize as usize;
		}
		// #define SYS_fstat 80
		80 => {
			// int fstat(int filedes, struct stat *buf)
			(*frame).regs[gp(Registers::A0)] = 0;
		}
		//SYSCALL_YIELD = 124
		124 => {

		}
		//SYSCALL_GET_TIME
		169 => {
			(*frame).regs[Registers::A0 as usize] = crate::cpu::get_mtime();
		}
		172 => {
			// A0 = pid
			(*frame).regs[Registers::A0 as usize] = (*frame).pid;
		}

		_ => {
			println!("Unknown syscall number {}", syscall_number);
			//mepc + 4
			//触发中断的ecall指令是32位，即4字节
		}
	}
}


extern "C" {
	fn make_syscall(sysno: usize, arg0: usize, arg1: usize, arg2: usize, arg3: usize, arg4: usize, arg5: usize) -> usize;
}

fn do_make_syscall(sysno: usize, arg0: usize, arg1: usize, arg2: usize, arg3: usize, arg4: usize, arg5: usize) -> usize {
	unsafe { make_syscall(sysno, arg0, arg1, arg2, arg3, arg4, arg5) }
}

pub fn syscall_yield() {
	let _ = do_make_syscall(1, 0, 0, 0, 0, 0, 0);
}

pub fn syscall_exit() {
	let _ = do_make_syscall(93, 0, 0, 0, 0, 0, 0);
}

pub fn syscall_execv(path: *const u8, argv: usize) -> usize {
	do_make_syscall(11, path as usize, argv, 0, 0, 0, 0)
}

pub fn syscall_fs_read(dev: usize, inode: u32, buffer: *mut u8, size: u32, offset: u32) -> usize {
	do_make_syscall(63, dev, inode as usize, buffer as usize, size as usize, offset as usize, 0)
}

pub fn syscall_block_read(dev: usize, buffer: *mut u8, size: u32, offset: u32) -> u8 {
	do_make_syscall(180, dev, buffer as usize, size as usize, offset as usize, 0, 0) as u8
}

pub fn syscall_sleep(duration: usize) {
	let _ = do_make_syscall(10, duration, 0, 0, 0, 0, 0);
}

pub fn syscall_get_pid() -> u16 {
	do_make_syscall(172, 0, 0, 0, 0, 0, 0) as u16
}

