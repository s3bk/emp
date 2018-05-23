use std::os::unix::io::{RawFd, AsRawFd};
use sys::*;
use std::ops::Deref;

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
        let fd = unsafe { epoll::epoll_create() }.unwrap();
        EPoll { fd }
    }
    fn add(&self, fd: RawFd, flags: epoll::Flags, data: u64) {
        let event = epoll::Event {
            events: flags.bits() as u32,
            data:   data
        };
        unsafe {
            epoll::epoll_ctl(self.fd, epoll::CtlOp::Add, fd, Some(&event));
        }
    }
    fn remove(&self, fd: RawFd) {
        unsafe {
            epoll::epoll_ctl(self.fd, epoll::CtlOp::Del, fd, None);
        }
    }
}
impl Drop for EPoll {
    fn drop(&mut self) {
        unsafe {
            close(self.fd);
        }
    }
}

pub struct Registered<F: AsRawFd> {
    inner: F
}
impl<F: AsRawFd> Deref for Registered<F> {
    type Target = F;
    fn deref(&self) -> &F {
        &self.inner
    }
}
impl<F: AsRawFd> Registered<F> {
    pub fn unregister(self) -> F {
        let fd = self.inner.as_raw_fd();
        POLL.with(|p| p.remove(fd));
        self.inner
    }
}

pub fn register<F: AsRawFd>(f: F, flags: epoll::Flags, data: u64) -> Registered<F> {
    let fd = f.as_raw_fd();
    POLL.with(|p| p.add(fd, flags, data));
    Registered { inner: f }
}
