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

    // should this be mutable?
    // isn't unsafe, there's nothing unsafe about getting this value
    pub fn raw(&mut self) -> *mut () {
        return self.ptr as *mut (); // this is apparently rust for void*
    }
}

impl Drop for MappedRegion {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr, self.sz);
        }
    }
}
