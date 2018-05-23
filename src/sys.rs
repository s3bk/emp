#![allow(non_upper_case_globals)]

use syscall_alt::syscalls::*;
use syscall_alt::constants::SYS::*;
use libc;
use std::os::unix::prelude::*;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::mem;

pub type Errno = i32;
macro_rules! syscall {
    (@CALL $name:ident) =>
        (syscall0($name));
    (@CALL $name:ident, $a:expr) =>
        (syscall1($name, $a));
    (@CALL $name:ident, $a:expr, $b:expr) =>
        (syscall2($name, $a, $b));
    (@CALL $name:ident, $a:expr, $b:expr, $c:expr) =>
        (syscall3($name, $a, $b, $c));
    (@CALL $name:ident, $a:expr, $b:expr, $c:expr, $d:expr) =>
        (syscall4($name, $a, $b, $c, $d));
    (@CALL $name:ident, $a:expr, $b:expr, $c:expr, $d:expr, $e:expr) =>
        (syscall5($name, $a, $b, $c, $d, $e));
    (@CALL $name:ident, $a:expr, $b:expr, $c:expr, $d:expr, $e:expr, $f:expr) =>
        (syscall6($name, $a, $b, $c, $d, $e, $f));
    ($name:ident ( $($args:expr),*) -> 0) => (
        match syscall!(@CALL $name $(, $args as isize)*) {
            0  => Ok(()),
            n if n > 0 => unreachable!(),
            e => Err(-e as Errno)
        }
    );
    ($name:ident ( $($args:expr),*) -> $t:ty) => (
        match syscall!(@CALL $name $(, $args as isize)*) {
            n if n >= 0 => Ok(n as $t),
            e => Err(-e as Errno)
        }
    );
}

pub unsafe fn close(fd: RawFd) -> Result<(), Errno> {
    syscall!(SYS_close(fd) -> 0)
}

pub mod epoll {
    use super::*;
    
    #[repr(C, packed)]
    pub struct Event {
        pub events: Flags,
        pub data:   u64
    }
    bitflags! {
        pub struct Flags: u32 {
            const In             = libc::EPOLLIN as u32;
            const Out            = libc::EPOLLOUT as u32;
            const ReadHup        = libc::EPOLLRDHUP as u32;
            const Pri            = libc::EPOLLPRI as u32;
            const Err            = libc::EPOLLERR as u32;
            const Hup            = libc::EPOLLHUP as u32;
            const EdgeTriggered  = libc::EPOLLET as u32;
        }
    }
    pub enum CtlOp {
        Add = libc::EPOLL_CTL_ADD as isize,
        Del = libc::EPOLL_CTL_DEL as isize
    }
    pub unsafe fn epoll_create() -> Result<RawFd, Errno> {
        syscall!(SYS_epoll_create1(0) -> RawFd)
    }
    pub unsafe fn epoll_ctl(epoll_fd: RawFd, op: CtlOp, fd: RawFd, event: Option<&Event>) -> Result<(), Errno> {
        syscall!(SYS_epoll_ctl(epoll_fd, op, fd, event.map(|r| r as *const Event as isize).unwrap_or(0)) -> 0)
    }
    pub unsafe fn epoll_wait(fd: RawFd, set: *mut Event, num_events: usize, timeout: i32) -> Result<usize, Errno> {
        syscall!(SYS_epoll_wait(fd, set, num_events, timeout) -> usize)
    }
}

pub mod msg {
    use super::*;
    
    bitflags! {
        pub struct Flags: u32 {
            const CloseOnExec      = libc::MSG_CMSG_CLOEXEC as u32;
            const DontWait      = libc::MSG_DONTWAIT as u32;
            const ErrQueue     = libc::MSG_ERRQUEUE as u32;
            const OutOfBounds     = libc::MSG_OOB as u32;
            const Peek   = libc::MSG_PEEK as u32;
            const Truncate  = libc::MSG_TRUNC as u32;
            const WaitAll = libc::MSG_WAITALL as u32;
        }
    }

    pub unsafe fn recv(fd: RawFd, buf: &mut [u8], flags: Flags) -> Result<usize, Errno> {
        syscall!(SYS_recvfrom(fd, buf.as_ptr(), buf.len(), flags.bits(), 0, 0) -> usize)
    }
}
pub mod sock {
    use super::*;
    
    bitflags! {
        pub struct Flags: u32 {
            const NonBlock    = libc::SOCK_NONBLOCK as u32;
            const CloseOnExec = libc::SOCK_CLOEXEC as u32;
        }
    }
    #[repr(i32)]
    pub enum SockDomain {
        Unix   = libc::AF_UNIX,
        IPv4   = libc::AF_INET,
        IPv6   = libc::AF_INET6,
        Ipx    = libc::AF_IPX,
        Packet = libc::AF_PACKET
    }
    #[repr(i32)]
    pub enum SockType {
        Tcp = libc::SOCK_STREAM,
        Udp = libc::SOCK_DGRAM,
        Raw = libc::SOCK_RAW
    }
        
    pub unsafe fn socket(domain: SockDomain, stype: SockType) -> Result<RawFd, Errno> {
        syscall!(SYS_socket(domain, stype, 0) -> RawFd)
    }
    pub unsafe fn listen(fd: RawFd, backlog: i32) -> Result<(), Errno> {
        syscall!(SYS_listen(fd, backlog) -> 0)
    }
    pub trait Addr {
        type Data;
        fn domain(&self) -> SockDomain;
        fn data(&self) -> Self::Data;
        fn from_data(Self::Data) -> Self;
    }
    impl Addr for (Ipv4Addr, u16) {
        type Data = libc::sockaddr_in;
        fn domain(&self) -> SockDomain { SockDomain::IPv4 }
        fn data(&self) -> Self::Data {
            libc::sockaddr_in {
                sin_family: libc::AF_INET as u16,
                sin_port: self.1.to_be(),
                sin_addr: libc::in_addr { s_addr: u32::from(self.0).to_be() },
                sin_zero: [0; 8]
            }
        }
        fn from_data(data: Self::Data) -> Self {
            let addr = u32::from_be(data.sin_addr.s_addr);
            let port = u16::from_be(data.sin_port);
            (Ipv4Addr::from(addr), port)
        }
    }
    impl Addr for (Ipv6Addr, u16) {
        type Data = libc::sockaddr_in6;
        fn domain(&self) -> SockDomain { SockDomain::IPv6 }
        fn data(&self) -> Self::Data {
            libc::sockaddr_in6 {
                sin6_family: libc::AF_INET as u16,
                sin6_port: self.1.to_be(),
                sin6_flowinfo: 0,
                sin6_addr: unsafe { mem::transmute(self.0.octets()) },
                sin6_scope_id: 0
            }
        }
        fn from_data(data: Self::Data) -> Self {
            let addr = data.sin6_addr.s6_addr;
            let port = u16::from_be(data.sin6_port);
            (Ipv6Addr::from(addr), port)
        }
    }
    pub unsafe fn bind<A: Addr>(fd: RawFd, addr: A) -> Result<(), Errno> {
        let data = addr.data();
        syscall!(SYS_bind(fd, &data as *const A::Data, mem::size_of_val(&data)) -> 0)
    }
    pub unsafe fn accept<A: Addr>(fd: RawFd, flags: Flags) -> Result<(RawFd, A), Errno> {
        let mut addr: A::Data = mem::zeroed();
        let mut len: u32 = mem::size_of::<A::Data>() as u32;
        let fd = syscall!(SYS_accept4(fd, &mut addr as *mut _, &mut len as *mut _, flags.bits()) -> RawFd)?;
        Ok((fd, A::from_data(addr)))
    }
}
