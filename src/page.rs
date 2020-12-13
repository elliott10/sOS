use core::{mem::size_of, ptr::null_mut};

extern "C" {
	static HEAP_START: usize;
	static HEAP_SIZE: usize;
}

static mut ALLOC_START: usize = 0;
const PAGE_ORDER: usize = 12;
pub const PAGE_SIZE: usize = 1 << 12;

pub const fn align_val(val: usize, order: usize) -> usize {
	let o = (1usize << order) - 1;
	(val + o) & !o
}

/*
struct FreePages {
	struct FreePages *next;
}
*/

//每页的1byte标志
#[repr(u8)]
pub enum PageBits {
	Empty = 0,
	Taken = 1 << 0,
	Last = 1 << 1,
}

impl PageBits {
	pub fn val(self) -> u8 {
		self as u8
	}
}

pub struct Page {
	flags: u8,
}

impl Page {
	pub fn is_last(&self) -> bool {
		if self.flags & PageBits::Last.val() != 0 {
			true
		}else{
			false
		}
	}
	pub fn is_taken(&self) -> bool {
		if self.flags & PageBits::Taken.val() != 0 {
			true
		}else{
			false
		}
	}
	pub fn is_free(&self) -> bool {
		!self.is_taken()
	}

	pub fn clear(&mut self) {
		self.flags = PageBits::Empty.val();
	}
	pub fn set_flag(&mut self, flag: PageBits) {
		self.flags |= flag.val();
	}
	pub fn clear_flag(&mut self, flag: PageBits) {
		self.flags &= !(flag.val());
	}
}

// ... stack | u8 Page structure | u8 page-structure | ... | 4096 bytes page | 4096 bytes page | ...
//           ^                                             ^
//        HEAP_START     真正可分配内存的起始地址: ALLOC_START = ((HEAP_START + num_pages * u8 ) + (PAGE_SIZE - 1)) & !(PAGE_SIZE - 1)
// ALLOC_START地址进行了上舍入,对齐到4096页边界； 可能不为整页的地址~~~~~~~~~~~~~~~~~~~~~~~~~~~
pub fn init() {
	unsafe {
		let num_pages = HEAP_SIZE / PAGE_SIZE;
		let ptr = HEAP_START as *mut Page;

		for i in 0..num_pages {
			(*ptr.add(i)).clear();
		}

		ALLOC_START = align_val(HEAP_START + num_pages * size_of::<Page,>(), PAGE_ORDER);
	}
}

//
//参数是申请分配的页个数; usize 动态大小的无符号整数
pub fn alloc(pages: usize) -> *mut u8 {
	assert!(pages > 0);
	unsafe {
		let num_pages = HEAP_SIZE / PAGE_SIZE;
		let ptr = HEAP_START as *mut Page;
		//开始搜索连续的页空间; 页数量边界？
		for i in 0..num_pages - pages {
			let mut found = false;

			if (*ptr.add(i)).is_free() {
				found = true;
				for j in i..i + pages { // 取值i, i+1, i+2 .. i+pages-1
					if (*ptr.add(j)).is_taken(){
						found = false;
						break;
					}

				}
			}
			
			if found {
				for k in i..i + pages -1 {
					(*ptr.add(k)).set_flag(PageBits::Taken);
				}

				//Last page
				(*ptr.add(i+pages-1)).set_flag(PageBits::Taken);
				(*ptr.add(i+pages-1)).set_flag(PageBits::Last);

				return (ALLOC_START + PAGE_SIZE * i) as *mut u8;
			}

		}

		// no countiguous allocation was found
		null_mut()
	}
}

pub fn dealloc(ptr: *mut u8) {
	assert!(!ptr.is_null());

	unsafe {
		// 每4096 bytes页分配一个Page structure 即页描述符(0 1 2...) 
		let addr = HEAP_START + (ptr as usize - ALLOC_START) / PAGE_SIZE;
		assert!(addr >= HEAP_START && addr < HEAP_START + HEAP_SIZE);

		let mut p = addr as *mut Page;

		while (*p).is_taken() && !(*p).is_last() {
			(*p).clear();
			p = p.add(1);
		}

		assert!(
			(*p).is_last() == true, "Possible double-free detected! (Not taken found before last)"
		);

		//the last page 最后一页
		(*p).clear();
	}
}

// 每个页4096 bytes
// 参数是要分配的页个数
pub fn zalloc(pages: usize) -> *mut u8 {
	let ret = alloc(pages);
	if !ret.is_null() {
		let size = (PAGE_SIZE * pages) / 8;
		let big_ptr = ret as *mut u64; // 指向8节的指针
		for i in 0..size { // 取 0 1 2 .. size-1
			unsafe {
				(*big_ptr.add(i)) = 0;
			}
		}
	}
	ret
}

pub fn print_page_allocations() {
	unsafe {
		let num_pages = HEAP_SIZE / PAGE_SIZE;
		let mut beg = HEAP_START as *const Page;
		let end = beg.add(num_pages);
		let alloc_beg = ALLOC_START;
		let alloc_end = ALLOC_START + num_pages * PAGE_SIZE;
		println!();
		println!("PAGE ALLOCATION TABLE\nPage Descriptors:{:p} -> {:p}\nPHYS:            0x{:x} -> 0x{:x}", beg, end, alloc_beg, alloc_end);
		println!("~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~");
		let mut num = 0;
		while beg < end {
			if (*beg).is_taken() {
				let start = beg as usize; //页描述符地址, (beg - HEAP_START)为相隔的页描述符个数
				let memaddr = ALLOC_START + (start - HEAP_START) * PAGE_SIZE;
				print!("0x{:x} => ", memaddr);
				loop {
					num +=1;
					if (*beg).is_last() {
						let end = beg as usize;
						let memaddr = ALLOC_START + (end - HEAP_START) * PAGE_SIZE + PAGE_SIZE - 1;
						print!("0x{:x}: {:>3} page(s)", memaddr, (end - start + 1));
						println!(".");
						break

					}
					beg = beg.add(1);
				}
			}
			beg = beg.add(1);
		}
		println!("~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~");
		println!("Allocated: {:>6} pages ({:>10} bytes).", num, num * PAGE_SIZE);
		println!("Free     : {:>6} pages ({:>10} bytes).", num_pages - num, (num_pages - num) * PAGE_SIZE);
		println!();

	}
}

//////////////////////////////

#[repr(i64)]
#[derive(Copy, Clone)]
pub enum EntryBits {
	None = 0,
	Valid = 1 << 0,
	Read = 1 << 1,
	Write = 1 << 2,
	Execute = 1 << 3,
	User = 1 << 4,
	Global = 1 << 5,
	Access = 1 << 6,
	Dirty = 1 << 7,

	ReadWrite = 1 << 1 | 1 << 2,
	ReadExecute = 1 << 1 | 1 << 3,
	ReadWriteExecute = 1 << 1 | 1 << 2 | 1 << 3,

	UserReadWrite = 1 << 1 | 1 << 2 | 1 << 4 ,
	UserReadExecute = 1 << 1 | 1 << 3 | 1 << 4,
	UserReadWriteExecute = 1 << 1 | 1 << 2 | 1 << 3 | 1 << 4,
}

impl EntryBits {
	pub fn val(self) -> i64 {
		self as i64
	}
}

pub struct Table {
	//            i64数据类型
	pub entries: [Entry; 512],
}

impl Table {
	pub fn len() -> usize {
		512
	}
}

pub struct Entry {
	pub entry: i64,
}

impl Entry {
	pub fn set_entry(&mut self, entry: i64) {
		self.entry = entry;
	}

	pub fn get_entry(& self) -> i64 {
		self.entry
	}

	pub fn is_valid(&self) -> bool {
		self.get_entry() & EntryBits::Valid.val() != 0
	}

	pub fn is_invalid(&self) -> bool {
		!self.is_valid()
	}

	// 叶子会有一个或多个RWX置位
	pub fn is_leaf(&self) -> bool {
		self.get_entry() & 0xe !=0
	}

	pub fn is_branch(&self) -> bool {
		!self.is_leaf()
	}

}

// Page Table Entry:
//  63      54 53    28 27    19 18    10 9   8  7   6   5   4   3   2   1   0
// | Reserved | PPN[2] | PPN[1] | PPN[0] | RSW | D | A | G | U | X | W | R | V |
//      10        26       9       9        2    1   1   1   1   1   1   1   1

// Virtual Address:
// | VPN[2] | VPN[1] | VPN[0] | page offset |
//    9        9        9          12
//
// Physical Address:
// | PPN[2] | PPN[1] | PPN[0] | page offset |
//    26       9        9          12

// level: 0 -> 4K页, 1 -> 2M页, 2 -> 1G页 
pub fn map(root: &mut Table, vaddr: usize, paddr: usize, bits: i64, level: usize) {
	//    0xe置位了RWX, bits会在最后页中使用，需要保证有效 
	assert!(bits & 0xe !=0);

	//掩码 0x1ff = 0b1_1111_1111 (9位）
	let vpn = [
		(vaddr >> 12) & 0x1ff, //VPN[0]
		(vaddr >> 21) & 0x1ff, //VPN[1]
		(vaddr >> 30) & 0x1ff, //VPN[2]
	];

	let ppn = [
		(paddr >> 12) & 0x1ff,
		(paddr >> 21) & 0x1ff,
		(paddr >> 30) & 0x3ff_fff, //26 bits
	];
	// 而"page offset"不用管，因为会直接从虚拟地址复制到物理地址

	//页表项定位
	let mut v = &mut root.entries[vpn[2]];

	//rev()只是反转顺序 1,0
	for i in (level..2).rev() {
		if !v.is_valid() {
			let page = zalloc(1); //创建新页表
			// 分配得到4096页对齐的物理地址，为了匹配上64位的页表条目, 右移2位
			v.set_entry((page as i64 >> 2) | EntryBits::Valid.val());
		}
		// 0x3ff = 0b 11_1111_1111, 只保留页表条目中的PPN[2|1|0], 并左移2位变成物理地址形式
		//该地址值 作为一个Entry的指针
		let entry = ((v.get_entry() & !0x3ff) << 2) as *mut Entry;
		v = unsafe {
			//注意 从值变为另外一个Entry指针
			entry.add(vpn[i]).as_mut().unwrap()
		};
	}

	//此时v应该是VPN[0]的Entry
	//页表条目和物理地址的转移位数不一样，少2
	let entry = (ppn[2] << 28) as i64 |
		    (ppn[1] << 19) as i64 |
		    (ppn[0] << 10) as i64 |
		    bits | EntryBits::Valid.val();

	v.set_entry(entry);
}

//只清页表，不会清root根页表，应为它常嵌在进程结构中
//不清页内存
pub fn unmap(root: &mut Table) {
	for lv2 in 0..Table::len(){
		let ref entry_lv2 = root.entries[lv2];
		if entry_lv2.is_valid() && entry_lv2.is_branch() {
			let memaddr_lv1 = (entry_lv2.get_entry() & !0x3ff) << 2;
			// Make table_lv1 a mutable reference instead of a pointer.
			let table_lv1 = unsafe { (memaddr_lv1 as *mut Table).as_mut().unwrap() };
			for lv1 in 0..Table::len() {
				let ref entry_lv1 = table_lv1.entries[lv1];
				if entry_lv1.is_valid() && entry_lv1.is_branch()
				{
					let memaddr_lv0 = (entry_lv1.get_entry() & !0x3ff) << 2;
					dealloc(memaddr_lv0 as *mut u8); //请VPN[0]表
				}
			}
			dealloc(memaddr_lv1 as *mut u8); //清VPN[1]表
		}
	}
	// page页不要清掉吗？
}

pub fn virt_to_phys(root: &Table, vaddr: usize) -> Option<usize> {
	let vpn = [
		(vaddr >> 12) & 0x1ff,
		(vaddr >> 21) & 0x1ff,
		(vaddr >> 30) & 0x1ff,
		];

	let mut v = &root.entries[vpn[2]];
	for i in (0..=2).rev() {
		if v.is_invalid() {
			break;
		}else if v.is_leaf() {
			// offset mask掩码, 0b 1_1111_1111  
			let off_mask = (1 << (12 + i * 9)) - 1;
			//保留虚拟地址末尾处的位
			let vaddr_pgoff = vaddr & off_mask;
			let addr = ((v.get_entry() << 2) as usize) & !off_mask;
			return Some(addr | vaddr_pgoff);
		}

		let entry = ((v.get_entry() & !0x3ff) << 2) as *const Entry;
		v = unsafe { 
			// 假如i=0时会是叶子，不会到达这里
			entry.add(vpn[i - 1]).as_ref().unwrap()
		};
	}
	None
}

