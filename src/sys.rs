#![allow(non_upper_case_globals)]

use syscall_alt::syscalls::*;
use syscall_alt::constants::SYS::*;
use libc;
use std::os::unix::prelude::*;

pub mod epoll {
    use super::*;
    
    #[repr(C, packed)]
    pub struct Event {
        pub events: u32,
        pub data:   u64
    }
    bitflags! {
        pub struct Flags: u32 {
            const LevelTriggered      = libc::EPOLLET as u32;
            const In      = libc::EPOLLIN as u32;
            const Hup     = libc::EPOLLHUP as u32;
            const Out     = libc::EPOLLOUT as u32;
            const RdHup   = libc::EPOLLRDHUP as u32;
        }
    }
    pub enum CtlOp {
        Add = libc::EPOLL_CTL_ADD as isize,
        Del = libc::EPOLL_CTL_DEL as isize
    }
    pub unsafe fn create() -> isize {
        syscall1(SYS_epoll_create1, 0)
    }
    pub unsafe fn ctl(epoll_fd: RawFd, op: CtlOp, fd: RawFd, event: Option<&Event>) -> isize {
        syscall4(SYS_epoll_ctl,
            epoll_fd as isize,
            op as isize,
            fd as isize,
            event.map(|r| r as *const Event as isize).unwrap_or(0)
        )
    }
}

pub mod msg {
    use super::*;
    
    bitflags! {
        pub struct Flags: u32 {
            const CloseOnEsec      = libc::MSG_CMSG_CLOEXEC as u32;
            const DontWait      = libc::MSG_DONTWAIT as u32;
            const ErrQueue     = libc::MSG_ERRQUEUE as u32;
            const OutOfBounds     = libc::MSG_OOB as u32;
            const Peek   = libc::MSG_PEEK as u32;
            const Truncate  = libc::MSG_TRUNC as u32;
            const WaitAll = libc::MSG_WAITALL as u32;
        }
    }

    pub unsafe fn recv<S: AsRawFd>(socket: &S, buf: &mut [u8], flags: Flags) -> isize {
        syscall6(SYS_recvfrom,
            socket.as_raw_fd() as isize,
            buf.as_ptr() as isize,
            buf.len() as isize,
            flags.bits() as isize,
            0,
            0
        )
    }
}
