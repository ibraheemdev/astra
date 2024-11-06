#![allow(clippy::return_self_not_must_use, clippy::needless_doctest_main)]
#![doc = include_str!("../README.md")]

mod executor;
mod http;
mod net;
mod server;

pub use http::{Body, BodyReader, Request, Response, ResponseBuilder};
pub use server::{ConnectionInfo, Server, Service};
