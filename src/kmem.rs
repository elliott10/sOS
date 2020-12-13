use crate::page::{align_val, zalloc, Table, PAGE_SIZE};
use core::{mem::size_of, ptr::null_mut};

#[repr(usize)]
enum AllocListFlags {
	Taken = 1 << 63,
}
impl AllocListFlags {
	pub fn val(self) -> usize {
		self as usize
	}
}

struct AllocList {
	pub flags_size: usize,
}

impl AllocList {
	pub fn is_taken(&self) -> bool {
		self.flags_size & AllocListFlags::Taken.val() != 0
	}
	pub fn is_free(&self) -> bool {
		!self.is_taken()
	}
	pub fn set_taken(&mut self) {
		self.flags_size |= AllocListFlags::Taken.val();
	}
	pub fn set_free(&mut self) {
		self.flags_size &= !AllocListFlags::Taken.val();
	}
	pub fn set_size(&mut self, sz: usize) {
		let k = self.is_taken();
		self.flags_size = sz & !AllocListFlags::Taken.val();
		if k {
			self.flags_size |= AllocListFlags::Taken.val();
		}
	}
	pub fn get_size(&self) -> usize {
		self.flags_size & !AllocListFlags::Taken.val()
	}
}

static mut KMEM_HEAD: *mut AllocList = null_mut();
static mut KMEM_ALLOC: usize = 0;
static mut KMEM_PAGE_TABLE: *mut Table = null_mut();

pub fn get_head() -> *mut u8 {
	unsafe { KMEM_HEAD as *mut u8 }
}

pub fn get_page_table() -> *mut Table {
	unsafe { KMEM_PAGE_TABLE as *mut Table }
}

pub fn get_num_allocations() -> usize {
	unsafe { KMEM_ALLOC }
}

// Initialize kernel's memory
pub fn init() {
	unsafe {
		// 64 * 4096 = 262K
		let k_alloc = zalloc(64);
		assert!(!k_alloc.is_null());
		KMEM_ALLOC = 64;
		KMEM_HEAD = k_alloc as *mut AllocList;
		(*KMEM_HEAD).set_free();
		(*KMEM_HEAD).set_size(KMEM_ALLOC * PAGE_SIZE);
		KMEM_PAGE_TABLE = zalloc(1) as *mut Table;
	}
}

pub fn kzmalloc(sz: usize) -> *mut u8{
	let size = align_val(sz, 3);
	let ret = kmalloc(size);

	if !ret.is_null() {
		for i in 0..size {
			unsafe {
				(*ret.add(i)) = 0;
			}
		}
	}
	ret
}

pub fn kmalloc(sz: usize) -> *mut u8 {
	unsafe {
				//上舍入 整8字节, 还有一个8字节的AllocList结构
		let size = align_val(sz, 3) + size_of::<AllocList>();
		let mut head = KMEM_HEAD;

		//指针移动到kernel内存结尾
		let tail = (KMEM_HEAD as *mut u8).add(KMEM_ALLOC * PAGE_SIZE) as *mut AllocList;
		while head < tail {
			if (*head).is_free() && size <= (*head).get_size() {
				let chunk_size = (*head).get_size();
				let rem = chunk_size - size;
				(*head).set_taken();
				if rem > size_of::<AllocList>() {
					//剩余的内核空间
					let next = (head as *mut u8).add(size) as *mut AllocList;
					(*next).set_free();
					(*next).set_size(rem);

					(*head).set_size(size);
				}else{
					//剩余空间太小，全部取走
					(*head).set_size(chunk_size);
				}
				//移到AllocList结构后
				return head.add(1) as *mut u8;
			}else {
				//当前不是空闲内存，移向下一个空闲空间
				head = (head as *mut u8).add((*head).get_size()) as *mut AllocList;
			}
		}
	}
	//内核空间的大块chunks不足
	null_mut()
}

pub fn kfree(ptr: *mut u8) {
	unsafe {
		if !ptr.is_null() {
			//取出前置的AllocList结构
			let p = (ptr as *mut AllocList).offset(-1);
			if (*p).is_taken() {
				(*p).set_free();
			}
			//合并碎片
			coalesce();
		}
	}
}

//把小块合并成大块
pub fn coalesce() {
	unsafe {
		let mut head = KMEM_HEAD;
		let tail = (KMEM_HEAD as *mut u8).add(KMEM_ALLOC * PAGE_SIZE) as *mut AllocList;
		while head < tail {
			let next = (head as *mut u8).add((*head).get_size()) as *mut AllocList;
			if (*head).get_size() == 0{
				//可能堆坏了，double free或咋了,next指针无法往前移，导致无限循环
				break;
			}else if next >= tail {
				break;
			}else if (*head).is_free() && (*next).is_free() {
				(*head).set_size((*head).get_size() + (*next).get_size());
			}

			head = (head as *mut u8).add((*head).get_size()) as *mut AllocList;
		}
	}
}

//print kmem table
pub fn print_table() {
	unsafe {
		let mut head = KMEM_HEAD;
		let tail = (KMEM_HEAD as *mut u8).add(KMEM_ALLOC * PAGE_SIZE) as *mut AllocList;
		println!("~~~~~KMEM Table~~~~~");
		while head < tail {
			println!("{:p}: Length = {:<10} Taken = {}", head, (*head).get_size(), (*head).is_taken());
			head = (head as *mut u8).add((*head).get_size()) as *mut AllocList;
		}
		println!("~~~~~~~~~~~~~~~~~~~~");
	}
}

//可以使用core库中的数据结构:链表或B-tree

use core::alloc::{GlobalAlloc, Layout};

struct OsGlobalAlloc;

unsafe impl GlobalAlloc for OsGlobalAlloc {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		kzmalloc(layout.size())
	}

	unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
		kfree(ptr);
	}
}

#[global_allocator]
static GA: OsGlobalAlloc = OsGlobalAlloc {};


#[alloc_error_handler]

pub fn alloc_error(l: Layout) -> ! {
	panic!("Allocator failed to allocate {} bytes with {}-bytes alignment.", l.size(), l.align());
}

