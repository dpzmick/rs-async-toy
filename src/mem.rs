use crate::util::*;

use libc;
use std::ffi::CString;

static HUGETLBFS: &str = "/mnt/hugetlb";

pub struct MappedRegion {
    ptr: *mut libc::c_void,
    sz:  usize,
}

impl MappedRegion {
    pub fn create(name: &str, size: usize)  -> Result<Self, i32> {
        unsafe {
            let path = CString::new(Self::_path(name).as_bytes()).unwrap();
            let fd = libc::open(path.as_ptr(), libc::O_RDWR | libc::O_CREAT, 0o777);
            if fd < 0 { return Err(read_errno()); }

            let fd = Fd::from_raw(fd); // managed fd

            // resize the file to the appropriate size
            let errno = libc::posix_fallocate(fd.raw(), 0, size as libc::off_t);
            if errno != 0 { return Err(errno); }

            return Self::_join(fd, size);
        }
    }

    pub fn join(name: &str, size: usize) -> Result<Self, i32> {
        unsafe {
            let path = CString::new(Self::_path(name).as_bytes()).unwrap();
            let fd = libc::open(path.as_ptr(), libc::O_RDWR);
            if fd < 0 { return Err(read_errno()); }

            return Self::_join(Fd::from_raw(fd), size);
        }
        
    }

    fn _path(name: &str) -> String { format!("{}/{}", HUGETLBFS, name) }

    unsafe fn _join(fd: Fd, size: usize) -> Result<Self, i32> {
        let mapped = libc::mmap(std::ptr::null_mut(), size,
                                libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED,
                                fd.raw(), 0);
        std::mem::drop(fd);

        if mapped == libc::MAP_FAILED { return Err(read_errno()); }

        return Ok(Self{
            ptr: mapped,
            sz:  size,
        })
    }

    pub fn as_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr as *mut u8, self.sz) }
    }

    pub fn as_slices(&mut self, sizes: &Vec<usize>) -> Option<Vec<&mut [u8]>> {
        let req_sz: usize = sizes.iter().sum();
        if req_sz > self.sz { return None; }

        unsafe {
            let mut curr = self.ptr;
            let mut ret = Vec::new();
            for size in sizes {
                ret.push(std::slice::from_raw_parts_mut(curr as *mut u8, *size));
                curr = curr.add(*size);
            }

            return Some( ret );
        }
    }
}

impl Drop for MappedRegion {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr, self.sz);
        }
    }
}
