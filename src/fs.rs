
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Inode {
pub mode:   u16,
pub nlinks: u16,
pub uid:    u16,
pub gid:    u16,
pub size:   u32,
pub atime:  u32,
pub mtime:  u32,
pub ctime:  u32,
pub zones:  [u32; 10]
}

