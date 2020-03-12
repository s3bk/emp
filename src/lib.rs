#![feature(generators, generator_trait, const_type_id, thread_local, libc, box_syntax, nll)]

#[macro_use] extern crate bitflags;
#[macro_use] extern crate log;

#[macro_use]
pub mod macros;
pub mod message;
pub mod dispatch;
pub mod epoll;
pub mod net;
pub mod sys;


pub mod prelude {
    pub use crate::message::*;
    pub use crate::dispatch::*;
    pub use crate::net::*;
}
