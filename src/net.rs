use prelude::*;
use epoll;
use sys;
use libc;
use std::net::TcpStream;
use std::os::unix::io::AsRawFd;
use sys::epoll::{Flags};
use epoll::WakeUp;

#[derive(Debug)]
pub struct Line(String);

#[derive(Debug)]
pub struct Closed;

pub fn line_reader(mut socket: TcpStream, reciever: Cid) -> PreparedCoro {
    socket.set_nonblocking(true);
    
    let mut buf_size = 128;
    let mut cursor = 0; // end of pending data
    let mut buf = vec![0; buf_size];
    let registration = epoll::register(socket.as_raw_fd(), Flags::LevelTriggered | Flags::In, reciever.0 as u64);
    
    Dispatcher::prepare_spawn(dispatcher! {
        WakeUp, _ => {
            let n = match unsafe { sys::msg::recv(&registration, &mut buf[cursor ..], sys::msg::Flags::DontWait) } {
                n if n > 0 => n as usize,
                0 => {
                    send!(reciever, Closed);
                    break;
                },
                n => match -n as i32 {
                    libc::EWOULDBLOCK => {
                        yield ProcessYield::Io;
                        continue;
                    },
                    e => panic!("got error: {}", e)
                }
            };
            
            if let Some(end) = buf[cursor .. cursor + n].iter().position(|&b| b == b'\n') {
                if let Ok(line) = ::std::str::from_utf8(&buf[..end]).map(String::from) {
                    send!(reciever, Line(line));
                }
            }
        }
    })
}
