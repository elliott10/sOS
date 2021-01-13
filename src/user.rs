
const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_YIELD: usize = 124;
const SYSCALL_GET_TIME: usize = 169;

pub const STDOUT: usize = 1;

pub fn syscall(id: usize, arg0: usize, arg1: usize, arg2: usize) -> isize {
	let mut ret: isize;
	unsafe{
		llvm_asm!("ecall"
		     :"={x10}"(ret) //x10 = a0
		     :"{x10}"(arg0), "{x11}"(arg1), "{x12}"(arg2), "{x17}"(id)
		     :"memory"
		     :"volatile");
	}
	ret
}

pub fn sys_yield() -> isize {
	syscall(SYSCALL_YIELD, 0, 0, 0)
}

pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
	syscall(SYSCALL_WRITE, fd, buffer.as_ptr() as usize, buffer.len())
}

pub fn sys_exit(state: i32) -> isize {
	syscall(SYSCALL_EXIT, state as usize, 0, 0)
}

pub fn init_process() {
	let mut i: usize = 0;
	sys_write(STDOUT, "\ninit from U mode\n".as_bytes());

	//运行在U态
    loop {
        i += 1;
        if i > 70_000_000 {
            unsafe {
                syscall(1, 0, 0, 0);
            }
            i = 0;

            unsafe {
                llvm_asm!("ebreak"::::"volatile");
            }

            sys_exit(0);//目前必须调用SYSCALL_EXIT来结束一个进程，不然出错
        }
	}
}
