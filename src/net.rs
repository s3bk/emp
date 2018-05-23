use prelude::*;
use epoll;
use sys;
use libc;
use std::net::{IpAddr, Ipv4Addr};
use std::os::unix::io::{RawFd, AsRawFd};
use std::{mem, slice};
use sys::epoll::{Flags};
use epoll::WakeUp;

const MIN_RECV_SIZE: usize = 128;

#[derive(Debug)]
pub struct Line(pub String);

#[derive(Debug)]
pub struct Closed;

struct Socket {
    fd: RawFd
}
impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}
impl Socket {
    fn listen(addr: IpAddr, port: u16, backlog: i32) -> Socket {
        use sys::sock::*;
        let fd = unsafe {
            let fd = socket(SockDomain::IPv4, SockType::Tcp).unwrap();
            libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_REUSEADDR, &1i32 as *const _ as _, 4);
            match addr {
                IpAddr::V4(ip) => bind(fd, (ip, port)),
                IpAddr::V6(ip) => bind(fd, (ip, port))
            }.unwrap();
            listen(fd, backlog).unwrap();
            
            fd
        };
        Socket { fd }
    }
    fn accept(&self) -> Option<Connection> {
        let r = unsafe { sys::sock::accept::<(Ipv4Addr, u16)>(self.fd, sys::sock::Flags::NonBlock) };
        match r {
            Ok((fd, remote)) => Some(Connection { fd, remote }),
            Err(libc::EWOULDBLOCK) => None,
            Err(e) => panic!("got error {}", e)
        }
    }
}
impl Drop for Socket {
    fn drop(&mut self) {
        unsafe { sys::close(self.fd) };
    }
}
#[derive(Debug)]
pub struct Connection {
    fd: RawFd,
    remote: (Ipv4Addr, u16)
}
impl Connection {
    pub fn recv_into(&self, buf: &mut Vec<u8>) -> Option<usize> {
        let start = buf.len();
        let mut size = buf.capacity() - start;
        // make sure we have some space to read into
        if size < MIN_RECV_SIZE {
            buf.reserve(MIN_RECV_SIZE);
            size = buf.capacity() - start;
        }
        
        let r = unsafe {
            let gap = slice::from_raw_parts_mut(buf.as_mut_ptr().offset(start as isize), size);
            sys::msg::recv(self.fd, gap, sys::msg::Flags::DontWait)
        };
        match r {
            Ok(0) => Some(0),
            Ok(n) => {
                unsafe {
                    buf.set_len(start + n);
                }
                Some(n)
            },
            Err(libc::EWOULDBLOCK) => None,
            Err(e) => panic!("got error: {}", e)
        }
    }
    pub fn remote(&self) -> (Ipv4Addr, u16) { self.remote }
}
impl AsRawFd for Connection {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        unsafe { sys::close(self.fd) };
    }
}
    
pub fn line_reader(conn: Connection, reciever: Cid) -> PreparedCoro {
    Dispatcher::prepare_spawn(|id, inbox| {
        let event = epoll::Event { events: Flags::In, data: id.0 as u64 };
        let registration = epoll::register(conn, event);
        move || {
            let mut cursor = 0; // end of pending data
            let mut buf = Vec::with_capacity(2*MIN_RECV_SIZE);
            
            loop {
                while let Some(e) = inbox.get() {
                    recv!(e => {
                        WakeUp, _ => {
                            let n = match registration.recv_into(&mut buf) {
                                None => {
                                    yield ProcessYield::Io;
                                    continue;
                                },
                                Some(0) => {
                                    send!(reciever, Closed);
                                    return ProcessExit::Done;
                                },
                                Some(n) => n
                            };
                            if let Some(end) = buf[cursor .. cursor + n].iter().position(|&b| b == b'\n') {
                                let remaining = buf.split_off(end+1);
                                let line = mem::replace(&mut buf, remaining);
                                cursor = 0;
                                
                                if let Ok(mut line) = String::from_utf8(line) {
                                    line.pop();
                                    send!(reciever, Line(line));
                                }
                            }
                        }
                    })
                }
                
                yield ProcessYield::Empty;
            }
        }
    })
}

pub fn listener(addr: IpAddr, port: u16, reciever: Cid) -> PreparedCoro {
    Dispatcher::prepare_spawn(move |id, inbox| {
        let socket = Socket::listen(addr, port, 10);
        let event = epoll::Event { events: Flags::In, data: id.0 as u64 };
        let socket = epoll::register(socket, event);
        move || {
            loop {
                while let Some(e) = inbox.get() {
                    recv!(e => {
                        WakeUp, _ => {
                            match socket.accept() {
                                None => {
                                    yield ProcessYield::Io;
                                    continue;
                                }
                                Some(c) => {
                                    send!(reciever, c);
                                    break;
                                }
                            }
                        }
                    })
                }
                
                yield ProcessYield::Empty;
            }
        }
    })
}
    
