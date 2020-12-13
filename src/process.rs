
use crate::cpu;
use crate::page;
use alloc::collections::vec_deque::VecDeque;

const STACK_PAGES: usize = 2;
const STACK_ADDR: usize = 0xf_0000_0000;
const PROCESS_STARTING_ADDR: usize = 0x2000_0000;

static mut PROCESS_LIST: Option<VecDeque<Process>> = None;
static mut NEXT_PID: u16 = 1;

fn init_proces() {
	loop{}
}

pub fn add_process_default(pr: fn()) {
	unsafe {
		if let Some(mut pl) = PROCESS_LIST.take() {
			let p = Process:new_default(pr);
			pl.push_back(p);
			PROCESS_LIST.replace(pl);
		}
	}
}

pub fn init() -> usize {
	unsafe {
		PROCESS_LIST = Some(VecDeque::with_capacity(5));
		add_process_default(init_process);

		let pl = PROCESS_LIST.take().unwrap();
		let p = pl.front().unwrap().frame;
		let frame = &p as *const TrapFrame as usize;
		mscratch_write(frame);
		satp_write(build_satp(SatpMode::Sv39, 1, pl.front().unwrap().root as usize));
		satp_fence_asid(1);
		PROCESS_LIST.replace(pl);
		PROCESS_STARTING_ADDR
	}
}

pub enum ProcessState {
	Running,
	Sleeping,
	Waiting,
	Dead,
}

#[repr(C)]
pub struct Process {
	frame: TrapFrame,
	stack: *mut u8,
	program_counter: usize,
	pid:   u16,
	root:  *mut Table,
	state: ProcessState,
	data:  ProcessData
}

impl Process {
	pub fn new_default(func: fn()) -> Self {
		let func_addr = func as usize;
		let mut ret_proc = 
			Process { frame: TrapFrame::zero(),
			          stack: alloc(STACK_PAGES),
				  program_counter: PROCESS_STARTING_ADDR,
				  pid:   unsafe { NEXT_PID },
				  root:  zalloc(1) as *mut Table,
				  state: ProcessState::Waiting,
				  data:  ProcessData::zero() };
		unsafe {
			NEXT_PID += 1;
		}

		ret_proc.frame.regs[2] = 	stack_addr + page_size * stack_pages;

		let pt;
		unsafe {
			pt = &mut *ret_proc.root;
		}
		let saddr = ret_proc.stack as usize;

		for i in 0..STACK_PAGES {
			let addr = i * PAGE_SIZE;
			map(pt, STACK_ADDR + addr, saddr + addr, EntryBits::UserReadWrite.val(), 0);
		}

		map(pt, PROCESS_STARTING_ADDR, func_addr, EntryBits::UserReadExecute.val(),0);
		map(pt, PROCESS_STARTING_ADDR + 0x1001, func_addr + 0x1001, EntryBits::UserReadExecute.val(),0);

		ret_proc
	}
}

impl Drop for Process {
	fn drop(&mut self) {
		dealloc(self.stack);
		unsafe {
			unmap(&mut *self.root);
		}
		dealloc(self.root as *mut u8);
	}
}

pub struct ProcessData {
	cw_path: [u8; 128]
}

impl ProcessData {
	pub fn zero() -> Self {
		ProcessData { 
			cwd_path: [0; 128]
		}
	}
}

