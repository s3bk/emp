//! EventFd binding
//!
//! This crate implements a simple binding for Linux eventfd(). See
//! eventfd(2) for specific details of behaviour.

use libc;
use std::os::unix::io::{AsRawFd, RawFd};
use std::mem;
use syscall_alt::syscalls::{syscall2};
use syscall_alt::constants::SYS::SYS_eventfd2;

#[derive(Debug)]
pub enum EventFdError {
    InvalidFlags,
    TooManyOpenFilesInProcess,
    TooManyOpenFilesInSystem,
    KernelOutOfMemory,
    KernelError
}

pub struct EventFd {
    fd: RawFd
}

impl EventFd {
    /// Create a new EventFd. Flags is the bitwise OR of EFD_* constants, or 0 for no flags.
    /// The underlying file descriptor is closed when the EventFd instance's lifetime ends.
    ///
    /// TODO: work out how to integrate this FD into the wider world
    /// of fds. There's currently no way to poll/select on the fd.
    pub fn new(initval: usize, flags: i32) -> Result<EventFd, EventFdError> {
        let res = unsafe {
            syscall2(SYS_eventfd2, initval as isize, flags as isize)
        };
        match res as i32 {
            fd if fd > 0 => Ok(EventFd { fd: fd }),
            libc::EINVAL => Err(EventFdError::InvalidFlags),
            libc::EMFILE => Err(EventFdError::TooManyOpenFilesInProcess),
            libc::ENFILE => Err(EventFdError::TooManyOpenFilesInSystem),
            libc::ENOMEM => Err(EventFdError::KernelOutOfMemory),
            libc::ENODEV => Err(EventFdError::KernelError),
            e => panic!("error code: {:?}", e)
        }
    }

    /// Read the current value of the eventfd. This will block until
    /// the value is non-zero. In semaphore mode this will only ever
    /// decrement the count by 1 and return 1; otherwise it atomically
    /// returns the current value and sets it to zero.
    pub fn read(&self) -> Result<u64, EventFdError> {
        let mut buf = [0u8; 8];
        unsafe {
            libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len());
        }
        let val = unsafe { mem::transmute(buf) };
        Ok(val)
    }

    /// Add to the current value. Blocks if the value would wrap u64.
    pub fn write(&self, val: u64) -> Result<(), EventFdError> {
        let buf: [u8; 8] = unsafe { mem::transmute(val) };
        unsafe {
            libc::write(self.fd, buf.as_ptr() as *mut libc::c_void, buf.len());
        }
        Ok(())
    }
}

impl AsRawFd for EventFd {
    /// Return the raw underlying fd. The caller must make sure self's
    /// lifetime is longer than any users of the fd.
    fn as_raw_fd(&self) -> RawFd {
        self.fd as RawFd
    }
}

impl Drop for EventFd {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

