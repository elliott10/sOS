//不可修改文件名lib.rs

//不实用标准库
#![no_std]
#![feature(panic_info_message, llvm_asm)]

#![feature(alloc_error_handler, alloc_prelude)]

#[macro_use]
extern crate alloc;

use alloc::prelude::v1::*;

#[macro_export]
macro_rules! print
{
	($($args:tt)+) => ({
		use core::fmt::Write;
		//每次都创建一个新的Uart ? 内存位置始终相同
		let _ = write!(crate::uart::Uart::new(0x1000_0000), $($args)+);

	});
}

#[macro_export]
macro_rules! println
{
	() => ({
		print!("\r\n")
	});

	($fmt:expr) => ({
		print!(concat!($fmt, "\r\n"))
	});

	/*
	把args标记为"token tree" (tt)

	"+" 匹配1或更多
	"*" 匹配0或更多；
	"$fmt:expr"格式化字符串"Hello {}"
	
	*/

	($fmt:expr, $($args:tt)+) => ({
		print!(concat!($fmt, "\r\n"), $($args)+)

	});
}

#[no_mangle]
extern "C" fn eh_personality(){}

// "-> !" 函数不返回值
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
	print!("Aborting: ");
	if let Some(_p) = info.location() {
		println!(
			"line {}, file {}: {}",
			_p.line(),
			_p.file(),
			info.message().unwrap()
		);
	}else{
		println!("no information available.");
	}
	abort();
}

//关闭Rust的名字管理，原样保留函数名称
#[no_mangle]
// 使用C风格的ABI
extern "C"
fn abort() -> ! {
	loop {
		unsafe {
			llvm_asm!("wfi"::::"volatile");
		}
	}
}

extern "C" {
	static TEXT_START: usize;
	static TEXT_END: usize;
	static DATA_START: usize;
	static DATA_END: usize;
	static RODATA_START: usize;
	static RODATA_END: usize;
	static BSS_START: usize;
	static BSS_END: usize;
	static KERNEL_STACK_START: usize;
	static KERNEL_STACK_END: usize;
	static HEAP_START: usize;
	static HEAP_SIZE: usize;
	static mut KERNEL_TABLE: usize;

	static _stack_end: usize;
}

//指定内存范围进行恒等映射，虚拟地址 = 物理地址
pub fn id_map_range(root: &mut page::Table, start: usize, end: usize, bits: i64)
{
	let mut memaddr = start & !(page::PAGE_SIZE -1);
				//上舍入
	let num_kb_pages = (page::align_val(end, 12) - memaddr) / page::PAGE_SIZE;

	for _ in 0..num_kb_pages {
		page::map(root, memaddr, memaddr, bits, 0);
		memaddr += 1 << 12;
	}
}

/*
SV39 MMU 分页系统

*/

//Entry Point
#[no_mangle]
//extern "C" fn kinit() -> usize {
extern "C" fn kinit() {
	uart::Uart::new(0x1000_0000).init();
	page::init();
	kmem::init();

	let root_ptr = kmem::get_page_table();
	let root_u = root_ptr as usize;
	let mut root = unsafe {
		root_ptr.as_mut().unwrap()
	};
	let kheap_head = kmem::get_head() as usize;
	let total_pages = kmem::get_num_allocations();
	println!();
	println!();

	unsafe {
		println!("TEXT:        0x{:x} -> 0x{:x}", TEXT_START, TEXT_END);
		println!("RODATA:      0x{:x} -> 0x{:x}", RODATA_START, RODATA_END);
		println!("DATA:        0x{:x} -> 0x{:x}", DATA_START, DATA_END);
		println!("BSS:         0x{:x} -> 0x{:x}", BSS_START, BSS_END);
		println!("STACK:       0x{:x} -> 0x{:x}", KERNEL_STACK_START, KERNEL_STACK_END);
		println!("Kernel HEAP: 0x{:x} -> 0x{:x}", kheap_head, kheap_head + total_pages * 4096);
	}

	//指定内存范围进行恒等映射，虚拟地址 = 物理地址
	//kernel堆内存, 可读可写
	id_map_range(&mut root, kheap_head, kheap_head + total_pages * 4096, page::EntryBits::ReadWrite.val());
	//与下面的HEAP映射重复了？

	unsafe {
		id_map_range(
			&mut root,
			HEAP_START,
			HEAP_START + HEAP_SIZE / page::PAGE_SIZE,
			page::EntryBits::ReadWrite.val(),
			);

		id_map_range(
			&mut root,
			TEXT_START,
			TEXT_END,
			page::EntryBits::ReadExecute.val(),
			);
		id_map_range(
			&mut root,
			RODATA_START,
			RODATA_END,
			page::EntryBits::ReadExecute.val(),
			);
		//.text和.rodata都在text, 见.lds

		id_map_range(
			&mut root,
			DATA_START,
			DATA_END,
			page::EntryBits::ReadWrite.val(),
			);
		id_map_range(
			&mut root,
			BSS_START,
			BSS_END,
			page::EntryBits::ReadWrite.val(),
			);
		id_map_range(
			&mut root,
			KERNEL_STACK_START,
			KERNEL_STACK_END,
			page::EntryBits::ReadWrite.val(),
			);
	}

	/*
	//使用页表把一个虚拟地址映射到物理地址
	// UART
	page::map( &mut root, 0x1000_0000, 0x1000_0000, page::EntryBits::ReadWrite.val(), 0);
	// CLINT
	// -> MSIP
	page::map( &mut root, 0x0200_0000, 0x0200_0000, page::EntryBits::ReadWrite.val(), 0);
	// -> MTIMECMP
	page::map( &mut root, 0x0200_b000, 0x0200_b000, page::EntryBits::ReadWrite.val(), 0);
	// -> MTIME
	page::map( &mut root, 0x0200_c000, 0x0200_c000, page::EntryBits::ReadWrite.val(), 0);
	*/

	//UART
	id_map_range(
			&mut root,
			0x1000_0000,
			0x1000_0100,
			page::EntryBits::ReadWrite.val(),
		    );

	//CLINT -> MSIP
	id_map_range(
			&mut root,
			0x0200_0000,
			0x0200_ffff,
			page::EntryBits::ReadWrite.val(),
		    );

	// PLIC
	id_map_range(
			&mut root,
			0x0c00_0000,
			0x0c00_2001,
			page::EntryBits::ReadWrite.val(),
		    );
	id_map_range(
			&mut root,
			0x0c20_0000,
			0x0c20_8001,
			page::EntryBits::ReadWrite.val(),
		    );
	/*
	page::print_page_allocations();

	//地址翻译，通过虚拟地址获取物理地址, 用户进程会用到
	let p = 0x8005_7000 as usize;
	let m = page::virt_to_phys(&root, p).unwrap_or(0);
	println!("Walk 0x{:x} = 0x{:x}", p, m);
	*/

	/////
	let satp_value = cpu::build_satp(cpu::SatpMode::Sv39, 0, root_u);
	unsafe {
		//切换上下文时保存寄存器等的TrapFrame, 其物理地址写入mscratch寄存器
		cpu::mscratch_write((&mut cpu::KERNEL_TRAP_FRAME[0] as *mut cpu::TrapFrame) as usize);
		cpu::sscratch_write(cpu::mscratch_read());
		cpu::KERNEL_TRAP_FRAME[0].satp = satp_value;
		//TrapFrame栈指针移到新申请1页的底部
		cpu::KERNEL_TRAP_FRAME[0].trap_stack = page::zalloc(1).add(page::PAGE_SIZE);

		//TrapFrame使用的内存地址和栈地址: 虚拟地址 = 物理地址
		id_map_range(&mut root, cpu::KERNEL_TRAP_FRAME[0].trap_stack.sub(page::PAGE_SIZE) as usize, cpu::KERNEL_TRAP_FRAME[0].trap_stack as usize, page::EntryBits::ReadWrite.val());
		id_map_range(&mut root, cpu::mscratch_read(), cpu::mscratch_read() + core::mem::size_of::<cpu::TrapFrame,>(), page::EntryBits::ReadWrite.val());

		//演示地址翻译
		page::print_page_allocations();
		let p = cpu::KERNEL_TRAP_FRAME[0].trap_stack as usize -1;
		let m = page::virt_to_phys(&root, p).unwrap_or(0);
		println!("Walk Trap Stack 0x{:x} = 0x{:x}", p, m);
	}

	println!("Setting 0x{:x}", satp_value);
	println!("Scratch reg = 0x{:x}", cpu::mscratch_read());
	cpu::satp_write(satp_value);
	cpu::satp_fence_asid(0);
	// sfence.vma 刷新TLB缓存



// SATP寄存器
//  63      60 59          44 43                0
// |MODE(WARL)|  ASID(WARL)  |     PPN(WARL)     |
//     4           16               44

	/*
	//保存kernel根页表
	unsafe {
		KERNEL_TABLE = root_u;
	}
	//返回值给boot.S中的SATP寄存器, 然后将切换进supervisor mode
	//内核根页表, 除于4K, 把低12位清掉
	(root_u >> 12) | (8 << 60)
	*/

	//使能MMU mode: Sv39, SATP的高四位第63,62,61,60决定了选哪种模式
	// 0 = Bare (no translation)
	// 8 = Sv39
	// 9 = Sv48
}

#[no_mangle]
extern "C" fn kinit_hart(hartid: usize) {
	//全部非０harts核在这初始化
	unsafe {
		cpu::mscratch_write((&mut cpu::KERNEL_TRAP_FRAME[hartid] as *mut cpu::TrapFrame) as usize);
		//相同mscratch寄存器的值复制给Ｓ态的sscratch
		cpu::sscratch_write(cpu::mscratch_read());
		cpu::KERNEL_TRAP_FRAME[hartid].hartid = hartid;

		//需要zalloc()锁
		// We can't do the following until zalloc() is locked, but we
		// don't have locks, yet :( 
		// cpu::KERNEL_TRAP_FRAME[hartid].satp = cpu::KERNEL_TRAP_FRAME[0].satp;
		// cpu::KERNEL_TRAP_FRAME[hartid].trap_stack = page::zalloc(1);
	}
}

//kmain()运行于S态
#[no_mangle]
extern "C"
fn kmain() {

	/*
	   UART使用MMIO地址在0x10000000, 详见qemu/riscv/virt.c; 8位寄存器
	   NS16550a的收发都在同0处偏移地址

	   在root module (lib.rs)可不加"crate::"
	 */
	
	//已在M态的kinit()中初始化，这里只是获取一个指针
	let mut my_uart = uart::Uart::new(0x1000_0000);
	println!();
	println!("##############################");

	unsafe {
		println!("SOS by xiaoluoyuan@163.com\nHeap start @ 0x{:x}", KERNEL_STACK_END);
		//println!("SOS by xiaoluoyuan@163.com\nHeap start @ 0x{:x}", _stack_end); // ? 有问题
	}

	{
		//在堆上存储u32类型的数据, 应用了global allocator
		let k = Box::<u32>::new(100);
		println!("Boxed *k value = {}",*k);
		println!("Boxed  k value = {}", k);

		//向量的数据存储在堆上
		let sparkle_heart = vec![240, 159, 146, 150];
		let sparkle_heart = String::from_utf8(sparkle_heart).unwrap();
		println!("String = {}", sparkle_heart);
		kmem::print_table();
	}
	println!("\n\nEverything should now be free:");
	//到这后，Box, vec和String的内存应该被释放了，因为出了括号的范围
	kmem::print_table();

	unsafe {
		//初始化CLINT timer, 设置下一次时钟中断的触发
		let mtimecmp = 0x0200_4000 as *mut u64;
		let mtime = 0x0200_bff8 as *const u64;
		//QEMU的频率是10_000_000 Hz, 触发在一秒后
		mtimecmp.write_volatile(mtime.read_volatile() + 10_000_000);

		//试验触发一个page fault
		let v = 0x0 as *mut u64;
		v.write_volatile(0);
	}

	// VIRTIO = [1..8]
	// UART0 = 10
	// PCIE = [32..35]
	println!("Setting up interrupts and PLIC...");
	plic::set_threshold(0);
	plic::enable(10);
	plic::set_priority(10, 1);

	/*
	println!("I'm so awesome. If you start typing something, I'll show you what you typed!");
	loop {
		if let Some(c) = my_uart.get() {
			match c {
				0x7f => { //0x8 [backspace] ; 而实际qemu运行，[backspace]键输出0x7f, 表示del
					print!("<<");
				},
				10 | 13 => { // 新行或回车

					println!();
					print!(">>");
				},
				// ANSI ESC序列是多字节：0x1b 0x5b 
				0x1b => { // 'ESC'
					if let Some(next_byte) = my_uart.get() {
						if next_byte == 0x5b { // '[' 偶尔出现ABCD字母，是因为丢失了0x5b节
							if let Some(b) = my_uart.get() {
								match b as char {
									'A' => {
										println!("^");
									},
									'B' => {
										println!("v");
									},
									'C' => {
										println!("->");
									},
									'D' => {
										println!("<-");
									},
									_ => {
										println!("{{0x{:x}}} That's something else...", b);
									}
								}
							}
						}
					}
				},

				//默认
				_ => {
					print!("{{0x{:x}={}}}", c, c as char);
				}
			}
		}
	}
	*/

}

// mod 类似c++的#include
pub mod uart;
pub mod page;
pub mod kmem;
pub mod cpu;
pub mod trap;
pub mod plic;

