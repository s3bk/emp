use std::os::unix::io::{RawFd, AsRawFd};
use libc;
use sys::*;

pub struct EPoll {
    fd: RawFd
}

thread_local! {
    static POLL: EPoll = EPoll::new();
}

#[derive(Debug, Copy, Clone)]
pub struct WakeUp(epoll::Flags);

impl EPoll {
    pub fn new() -> EPoll {
        let fd = unsafe { epoll::create() };
        assert!(fd >= 0);
        EPoll { fd: fd as RawFd }
    }
    fn add(&self, fd: RawFd, flags: epoll::Flags, data: u64) {
        let event = epoll::Event {
            events: flags.bits() as u32,
            data:   data
        };
        unsafe {
            epoll::ctl(self.fd, epoll::CtlOp::Add, fd, Some(&event));
        }
    }
    fn remove(&self, fd: RawFd) {
        unsafe {
            epoll::ctl(self.fd, epoll::CtlOp::Del, fd, None);
        }
    }
}
impl Drop for EPoll {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

pub struct RegisteredFd {
    fd: RawFd
}
impl RegisteredFd {
    pub fn unregister(self) -> RawFd {
        POLL.with(|p| p.remove(self.fd));
        self.fd
    }
}
impl AsRawFd for RegisteredFd {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

pub fn register(fd: RawFd, flags: epoll::Flags, data: u64) -> RegisteredFd {
    POLL.with(|p| p.add(fd, flags, data));
    RegisteredFd { fd }
}
