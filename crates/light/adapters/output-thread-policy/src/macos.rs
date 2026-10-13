//! Public Apple QoS calls confined to the private output worker; no process policy.
//!
//! pthread/qos.h documents requested QoS (not effective policy), direct errno returns and
//! macOS 10.10 availability. No pointers, ownership transfers or callbacks cross this ABI.

use std::ffi::{c_int, c_uint};
use std::io;

pub const USER_INTERACTIVE: c_uint = 0x21;

unsafe extern "C" {
    fn qos_class_self() -> c_uint;
    fn pthread_set_qos_class_self_np(class: c_uint, relative_priority: c_int) -> c_int;
}

pub fn requested_class() -> c_uint {
    // SAFETY: this public Apple function takes no arguments and reads only the calling thread.
    unsafe { qos_class_self() }
}

/// Classify only the calling, privately owned output thread; no real-time/process priority.
pub fn request_interactive() -> io::Result<()> {
    // SAFETY: this public API takes validated scalar class/relative-priority values, affects
    // only the calling thread, and returns an errno directly. No pointers or callbacks cross.
    let error = unsafe { pthread_set_qos_class_self_np(USER_INTERACTIVE, 0) };
    if error == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(error))
    }
}
