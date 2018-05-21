#![feature(generators, generator_trait, get_type_id, const_type_id, thread_local, libc, box_syntax)]

extern crate bincode;
extern crate serde;
extern crate libc;
extern crate syscall_alt;
#[macro_use] extern crate bitflags;

#[macro_use]
pub mod macros;
pub mod message;
pub mod dispatch;
pub mod epoll;
pub mod net;
mod sys;


pub mod prelude {
    pub use message::*;
    pub use dispatch::*;
}
