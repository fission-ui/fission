//! Linux-only external fault injector, loaded into a child test process.
//! Make Rust's filesystem canonicalization fail while normal file I/O works.
use std::ffi::{c_char, c_int};

extern "C" {
    fn __errno_location() -> *mut c_int;
}

#[no_mangle]
pub extern "C" fn realpath(_path: *const c_char, _resolved: *mut c_char) -> *mut c_char {
    // The injected failure models a filesystem with no final-path resolution.
    unsafe {
        *__errno_location() = 95;
    } // Linux ENOTSUP
    std::ptr::null_mut()
}
