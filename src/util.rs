use libc;

use std::ffi::CStr;

pub fn read_errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

pub fn strerror(errno: i32) -> String { // nasty, converting to rust string for easy printing
    unsafe {
        let c_str = libc::strerror(errno);
        CStr::from_ptr(c_str).to_str().expect("Static errno string").to_string()
    }
}

pub struct Fd {
    fd: i32,
}

impl Fd {
    pub fn from_raw(fd: i32) -> Self {
        // FIXME enforce non -1?
        Self { fd }
    }

    // unsafe because don't close the fd
    pub unsafe fn raw(&self) -> i32 {
        return self.fd;
    }
}

impl Drop for Fd {
    fn drop(&mut self) {
        // ignore value of close
        let _ = unsafe { libc::close(self.fd) };
    }
}
