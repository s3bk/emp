extern crate libc;

use std::os::unix::io::RawFd;
use libc::{c_void, c_int};

pub struct EPoll {
    fd: RawFd
}

#[thread_local]
static mut POLL: EPoll { fd: -1 }


#[repr(C, packed)]
struct Event {
    events: u32,
    data:   u64
}

bitflags! {
    struct Flags: u32 {
    
impl EPoll {
    pub fn new(size: usize) -> EPoll {
        fd = epoll_create();
        assert!(fd >= 0);
        EPoll { fd }
    }
    fn add(&mut self, fd: RawFd, flags: , data: u64) {
        let event = Event {
            events: flags | EPOLLT,
            data: 
        epoll_ctl(self.fd, EPOLL_CTL_ADD, fd, &event as *const _ as isize);
    }
    fn remove(&mut self, fd: RawFd)
}

struct RegisteredFd {
    fd: RawFd
}
impl RegisteredFd {
    pub fn unregister(self) -> RawFd {
        POLL.remove(self.fd);
        self.fd
    }
}



pub fn register(fd: RawFd) -> RegisteredFd {
    POLL.add(fd, 
