use std::os::unix::io::{RawFd, AsRawFd};
use sys::*;
pub use sys::epoll::Event;
use std::ops::Deref;
use dispatch::{Dispatcher, PreparedCoro, Sleep, Cid, ProcessYield};

pub struct EPoll {
    fd: RawFd
}

thread_local! {
    static POLL: EPoll = EPoll::new();
}

#[derive(Copy, Clone, Debug)]
pub struct WakeUp(epoll::Flags);

impl EPoll {
    pub fn new() -> EPoll {
        let fd = unsafe { epoll::epoll_create() }.unwrap();
        EPoll { fd }
    }
    fn add(&self, fd: RawFd, event: epoll::Event) {
        unsafe {
            epoll::epoll_ctl(self.fd, epoll::CtlOp::Add, fd, Some(&event)).expect("epoll_ctl");
        }
    }
    fn remove(&self, fd: RawFd) {
        unsafe {
            epoll::epoll_ctl(self.fd, epoll::CtlOp::Del, fd, None).expect("epoll_ctl");
        }
    }
    fn wait(&self, set: &mut Vec<epoll::Event>) -> Result<(), Errno> {
        if set.capacity() < 10 {
            set.reserve(10);
        }
        unsafe {
            match epoll::epoll_wait(self.fd, set.as_mut_ptr(), set.capacity(), -1) {
                Ok(n) => {
                    set.set_len(n);
                    Ok(())
                },
                Err(e) => {
                    set.set_len(0);
                    Err(e)
                }
            }
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

pub fn register<F: AsRawFd>(f: F, event: epoll::Event) -> Registered<F> {
    let fd = f.as_raw_fd();
    POLL.with(|p| p.add(fd, event));
    Registered { inner: f }
}

pub fn sleeper() -> PreparedCoro {
    Dispatcher::prepare_spawn(move |_, inbox| move || {
        let mut events = Vec::with_capacity(1024);
        loop {
            while let Some(e) = inbox.get() {
                recv!(e => {
                    Sleep, _ => {
                        match POLL.with(|p| p.wait(&mut events)) {
                            Ok(()) => {
                                for i in 0 .. events.len() {
                                    let epoll::Event { events, data } = events[i];
                                    send!(Cid(data as u32), WakeUp(events));
                                }
                                events.clear();
                            },
                            Err(e) => {
                                eprintln!("epoll::wait -> {:?}", e);
                                exit!(1, "epoll failed");
                            }
                        }
                    }
                })
            }
            
            yield ProcessYield::Empty;
        }
    })
}
