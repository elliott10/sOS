use crate::process::{ProcessState, PROCESS_LIST, PROCESS_LIST_MUTEX};
use crate::cpu::get_mtime;

pub fn schedule() -> usize {
	let mut frame_addr: usize = 0x1111;
	unsafe {
		if PROCESS_LIST_MUTEX.try_lock() == false {
			return 0;
		}

		if let Some(mut pl) = PROCESS_LIST.take() {
			loop {
				//队列向左旋转1个，相当于会每次调度左旋一次
				pl.rotate_left(1);
				if let Some(prc) = pl.front_mut() {
					match prc.state {
						ProcessState::Running => {
							frame_addr = prc.frame as usize;
							break;
						},
						ProcessState::Sleeping => {
							if prc.sleep_until <= get_mtime() {
								prc.state = ProcessState::Running;
								frame_addr = prc.frame as usize;
								break;
							}
						},
						_ => {},
					}

				}
			}
			PROCESS_LIST.replace(pl);
		}else{
			println!("could not take process list");
		}

	PROCESS_LIST_MUTEX.unlock();
	}
	//至少应该有init进程
	frame_addr
}
