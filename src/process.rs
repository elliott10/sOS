use crate::cpu::{TrapFrame, mscratch_write, satp_write, satp_fence_asid, build_satp, SatpMode};
use crate::page::{alloc, dealloc, map,unmap, zalloc, EntryBits, Table, PAGE_SIZE};
use crate::user::init_process;
use crate::cpu::get_mtime;
use crate::fs::Inode;
use crate::lock::Mutex;
use alloc::collections::{vec_deque::VecDeque, BTreeMap};
use alloc::string::String;
use core::ptr::null_mut;

//每个进程的栈分配2个页
const STACK_PAGES: usize = 2;
const STACK_ADDR: usize = 0x1_0000_0000;
const PROCESS_STARTING_ADDR: usize = 0x2000_0000;
//进程的开始执行地址, user mode

//进程列表，使用了global allocator
pub static mut PROCESS_LIST: Option<VecDeque<Process>> = None;
pub static mut PROCESS_LIST_MUTEX: Mutex = Mutex::new();
static mut NEXT_PID: u16 = 1;

pub fn set_running(pid: u16) -> bool {
	let mut retval = false;
	unsafe {
		if let Some(mut pl) = PROCESS_LIST.take() {
			for proc in pl.iter_mut() {
				if proc.pid == pid {
					proc.state = ProcessState::Running;
					retval = true;
					break;
				}
			}
			PROCESS_LIST.replace(pl);
		}
	}
	//println!("Set PID:{} running...", pid);
	retval
}

pub fn set_waiting(pid: u16) -> bool {
	let mut retval = false;
	unsafe {
		if let Some(mut pl) = PROCESS_LIST.take() {
			for proc in pl.iter_mut() {
				if proc.pid == pid {
					proc.state = ProcessState::Waiting;
					retval = true;
					break;
				}
			}
			PROCESS_LIST.replace(pl);
		}
	}
	//println!("Set PID:{} waiting...", pid);
	retval
}

/// Sleep a process
pub fn set_sleeping(pid: u16, duration: usize) -> bool {
	// Yes, this is O(n). A better idea here would be a static list
	// of process pointers.
	let mut retval = false;
	unsafe {
		if let Some(mut pl) = PROCESS_LIST.take() {
			for proc in pl.iter_mut() {
				if proc.pid == pid {
					proc.state = ProcessState::Sleeping;
					proc.sleep_until = get_mtime() + duration;
					retval = true;
					break;
				}
			}
			// Now, we no longer need the owned Deque, so we hand it
			// back by replacing the PROCESS_LIST's None with the
			// Some(pl).
			PROCESS_LIST.replace(pl);
		}
	}
	retval
}

pub fn add_process_default(pr: fn()) {
	unsafe {
		//转移出来了Deque的所有权通过.take(), 转移后此时PROCESS_LIST是None
		//这样允许互斥"mutual exclusion"
		if let Some(mut pl) = PROCESS_LIST.take() {
			let p = Process::new_default(pr);
			pl.push_back(p);

			//现在不再需要拥有Deque的所有权，还回去
			PROCESS_LIST.replace(pl);
		}
		//TODO: 多核时还需要继续抓取process list
	}
}

//创建init进程,只调用一次
//现在在kernel，之后会调用shell
pub fn init() -> usize {
	unsafe {
		PROCESS_LIST_MUTEX.spin_lock();
		//初始化PROCESS_LIST, 队列的容量15
		PROCESS_LIST = Some(VecDeque::with_capacity(15));
		add_process_default(init_process);

		//只是想要TrapFrame的内存地址。可优化？
		let pl = PROCESS_LIST.take().unwrap();
		let p = pl.front().unwrap().frame;

		/*
		let func_vaddr = pl.front().unwrap().program_counter;
		let frame = p as *const TrapFrame as usize;
		println!("Init's frame is at 0x{:08x}", frame);

		mscratch_write(frame);
		satp_write(build_satp(SatpMode::Sv39, 1, pl.front().unwrap().root as usize));
		//PID = 1, 使用ASID当做PID
		satp_fence_asid(1);
		*/

		PROCESS_LIST.replace(pl);
		PROCESS_LIST_MUTEX.unlock();

		//返回第一条指令的执行地址, 有MMU
		//func_vaddr
		(*p).pc
	}
}

pub fn add_kernel_process_args(func: fn(args_ptr: usize), args: usize) -> u16 {
	0
}

pub enum ProcessState {
	Running,
	Sleeping,
	Waiting,
	Dead, //进程一般不在此状态，马上会被清理
}

//一个PC(program counter)和一个运行栈stack
//C风格,方便汇编访问
#[repr(C)]
pub struct Process {
	pub frame:       *mut TrapFrame,
	pub stack:       *mut u8,
	pub pid:         u16,
	pub mmu_table:   *mut Table,
	pub state:       ProcessState,
	pub data:        ProcessData,
	pub sleep_until: usize,
	pub program:	 *mut u8,
	pub brk:         usize,
}

impl Process {
	//参数是入口函数地址
	pub fn new_default(func: fn()) -> Self {
		let func_addr = func as usize;
		let func_vaddr = func_addr;
		let mut ret_proc = 
			Process { frame: zalloc(1) as *mut TrapFrame,
			          stack: alloc(STACK_PAGES),
				  pid:   unsafe { NEXT_PID },
				  mmu_table:  zalloc(1) as *mut Table,
				  state: ProcessState::Running,
				  data:  ProcessData::new(),
				  sleep_until: 0,
				  program: null_mut(),
				  brk: 0,
			};

		unsafe {
			satp_fence_asid(NEXT_PID as usize);
			//之后需要改进成原子加，防止调度导致的变量加法错误
			NEXT_PID += 1;
		}

		let saddr = ret_proc.stack as usize;
		unsafe {
			(*ret_proc.frame).pc = func_vaddr;
			(*ret_proc.frame).pid = ret_proc.pid as usize;
			//x2 = sp栈指针, 移动到申请到内存的底部
			(*ret_proc.frame).regs[2] = STACK_ADDR + PAGE_SIZE * STACK_PAGES;
		}

		let pt;
		unsafe {
			pt = &mut *ret_proc.mmu_table;
			(*ret_proc.frame).satp = build_satp(SatpMode::Sv39, ret_proc.pid as usize, ret_proc.mmu_table as usize);
			println!("Process {}, mmu table: 0x{:x}", ret_proc.pid as usize, ret_proc.mmu_table as usize);
		}

		//把栈stack映射到用户空间的虚拟内存
		for i in 0..STACK_PAGES {
			let addr = i * PAGE_SIZE;
			map(pt, STACK_ADDR + addr, saddr + addr, EntryBits::UserReadWrite.val(), 0);
			println!("Map process stack:     0x{:x}", saddr + addr);
		}

		let mut modifier = 0;
		//映射PC等
		//for i in 0..=100 {
		for i in 0..=256 { // app < 1M
			modifier = i * 0x1000;
			map(pt, func_vaddr + modifier, func_addr + modifier, EntryBits::UserReadWriteExecute.val(), 0);
		}
		println!("Map process func_addr: 0x{:x} ~ 0x{:x}", func_addr, func_addr + modifier);

		//make_syscall函数，现在在kernel里运行一个进程; 到时从块设备载入时，我们可载入指令到内存的任何处
		map(pt, 0x8000_0000, 0x8000_0000, EntryBits::UserReadExecute.val(), 0);

		/*
		for i in 0..=20 {
		modifier = i * 0x1000;
		map(pt, 0x8000_0000 + modifier, 0x8000_0000 + modifier, EntryBits::UserReadExecute.val(), 0);
		}
		println!("Map init process:      0x80000000 ~ 0x{:x}", 0x80000000 + modifier);
		*/

		ret_proc
	}

}

pub fn delete_process(pid: u16) {
	unsafe {
		if let Some(mut pl) = PROCESS_LIST.take() {
			for i in 0..pl.len() {
				let p = pl.get_mut(i).unwrap();
				if (*(*p).frame).pid as u16 == pid {
					// When the structure gets dropped, all
					// of the allocations get deallocated.
					pl.remove(i);
					break;
				}
			}
			// Now, we no longer need the owned Deque, so we hand it
			// back by replacing the PROCESS_LIST's None with the
			// Some(pl).
			PROCESS_LIST.replace(pl);
		}
	}
}

pub unsafe fn get_by_pid(pid: u16) -> *mut Process {
	let mut ret = null_mut();
	if let Some(mut pl) = PROCESS_LIST.take() {
		for i in pl.iter_mut() {
			if (*(i.frame)).pid as u16 == pid {
				ret = i as *mut Process;
				break;
			}
		}
		PROCESS_LIST.replace(pl);
	}
	if ret.is_null() {
		//println!("Get process by pid: {} failed!", pid);
	}
	ret
}

pub enum Descriptor {
	File(Inode),
	Device(usize),
	Framebuffer,
	ButtonEvents,
	AbsoluteEvents,
	Console,
	Network,
	Unknown,
}

//堆上的内存
impl Drop for Process {
	fn drop(&mut self) {
		dealloc(self.stack);
		unsafe {
			unmap(&mut *self.mmu_table);
		}
		//手动清root根页表
		dealloc(self.mmu_table as *mut u8);

		println!("Drop a process: {}", self.pid);

		dealloc(self.frame as *mut u8);
		for i in self.data.pages.drain(..) {
			dealloc(i as *mut u8);
		}
		// Kernel processes don't have a program, instead the program is linked
		// directly in the kernel.
		if !self.program.is_null() {
			dealloc(self.program);
		}

	}
}

//进程的私有数据
pub struct ProcessData {
	pub environ: BTreeMap<String, String>,
	pub fdesc: BTreeMap<u16, Descriptor>,
	pub cwd: String,
	pub pages: VecDeque<usize>,
}

impl ProcessData {
	pub fn new() -> Self {
		ProcessData { 
			environ: BTreeMap::new(),
			fdesc: BTreeMap::new(),
			cwd: String::from("/"),
			pages: VecDeque::new(),
		 }
	}
}

