extern crate libc;

static SIZE: usize = 2<<20;

mod mem;
mod util;

use crate::util::*;
use crate::mem::*;

fn main() {
    let map = MappedRegion::join("test", SIZE);
    if let Err(errno) = map {
        eprintln!("Failed to mmap some memory with {} ({})",
                  strerror(errno), errno);
    }
}
