use crate::cpu::{TrapFrame, mscratch_write, satp_write, satp_fence_asid, build_satp, SatpMode};
use crate::page::{alloc, dealloc, map,unmap, zalloc, EntryBits, Table, PAGE_SIZE};
use alloc::collections::vec_deque::VecDeque;

//每个进程的栈分配2个页
const STACK_PAGES: usize = 2;
const STACK_ADDR: usize = 0x1_0000_0000;
const PROCESS_STARTING_ADDR: usize = 0x8000_0000;
//进程的开始执行地址, user mode

//进程列表，使用了global allocator
pub static mut PROCESS_LIST: Option<VecDeque<Process>> = None;
static mut NEXT_PID: u16 = 1;

extern "C" {
	fn make_syscall(a: usize) -> usize;
}

fn init_process() {
	let mut i: usize = 0;
	//运行在U态
	loop {
		i += 1;
		if i > 70_000_000 {
			unsafe {
				make_syscall(1);
			}
			i = 0;
		}
	}
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

		//返回第一条指令的执行地址, 有MMU
		//func_vaddr
		(*p).pc
	}
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
	frame: *mut TrapFrame,
	stack: *mut u8,
	pid:   u16,
	root:  *mut Table,
	state: ProcessState,
	data:  ProcessData,
	sleep_until: usize,
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
				  root:  zalloc(1) as *mut Table,
				  state: ProcessState::Running,
				  data:  ProcessData::zero(),
				  sleep_until: 0 };

		unsafe {
			satp_fence_asid(NEXT_PID as usize);
			//之后需要改进成原子加，防止调度导致的变量加法错误
			NEXT_PID += 1;
		}

		let saddr = ret_proc.stack as usize;
		unsafe {
			(*ret_proc.frame).pc = func_vaddr;
			//x2 = sp栈指针, 移动到申请到内存的底部
			(*ret_proc.frame).regs[2] = STACK_ADDR + PAGE_SIZE * STACK_PAGES;
		}

		let pt;
		unsafe {
			pt = &mut *ret_proc.root;
			(*ret_proc.frame).satp = build_satp(SatpMode::Sv39, ret_proc.pid as usize, ret_proc.root as usize);
		}

		//把栈stack映射到用户空间的虚拟内存
		for i in 0..STACK_PAGES {
			let addr = i * PAGE_SIZE;
			map(pt, STACK_ADDR + addr, saddr + addr, EntryBits::UserReadWrite.val(), 0);
			//println!("Set process stack from 0x{:016x} -> 0x{:016x}", STACK_ADDR + addr, saddr + addr);
		}

		//映射PC等
		for i in 0..=100 {
			let modifier = i * 0x1000;
			map(pt, func_vaddr + modifier, func_addr + modifier, EntryBits::UserReadWriteExecute.val(), 0);
		}

		//make_syscall函数，现在在kernel里运行一个进程; 到时从块设备载入时，我们可载入指令到内存的任何处
		map(pt, 0x8000_0000, 0x8000_0000, EntryBits::UserReadExecute.val(), 0);
		ret_proc
	}

	pub fn get_frame_address(&self) -> usize {
		self.frame as usize
	}
	pub fn get_program_counter(&self) -> usize {
		unsafe { (*self.frame).pc }
	}
	pub fn get_table_address(&self) -> usize {
		self.root as usize
	}
	pub fn get_state(&self) -> &ProcessState {
		&self.state
	}
	pub fn get_pid (&self) -> u16 {
		self.pid
	}
	pub fn get_sleep_until(&self) -> usize {
		self.sleep_until
	}
}

//堆上的内存
impl Drop for Process {
	fn drop(&mut self) {
		dealloc(self.stack);
		unsafe {
			unmap(&mut *self.root);
		}
		//手动清root根页表
		dealloc(self.root as *mut u8);
	}
}

//进程的私有数据
pub struct ProcessData {
	cwd_path: [u8; 128]
}

impl ProcessData {
	pub fn zero() -> Self {
		ProcessData { 
			cwd_path: [0; 128]
		}
	}
}

