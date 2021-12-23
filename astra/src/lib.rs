#![allow(clippy::return_self_not_must_use)]
#![doc = include_str!("../README.md")]

mod executor;
mod net;
mod reactor;
mod runtime;

pub use net::TcpStream;
pub use reactor::Reactor;
pub use runtime::{block_on, spawn, spawn_future};
