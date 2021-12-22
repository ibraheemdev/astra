use crate::executor;

use core::fmt;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::body::{Bytes, HttpBody};

pub struct Body(pub(crate) hyper::Body);

impl Body {
    pub fn empty() -> Body {
        Body(hyper::Body::empty())
    }

    pub fn new(data: impl Into<Bytes>) -> Body {
        Body(hyper::Body::from(data.into()))
    }
}

impl<T> From<T> for Body
where
    Bytes: From<T>,
{
    fn from(data: T) -> Body {
        Body::new(data)
    }
}

impl Iterator for Body {
    type Item = io::Result<Bytes>;

    fn next(&mut self) -> Option<Self::Item> {
        executor::block_on(self.0.data())
            .map(|res| res.map_err(|err| io::Error::new(io::ErrorKind::Other, err)))
    }
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::empty()
    }
}

impl HttpBody for Body {
    type Data = Bytes;
    type Error = hyper::Error;

    fn poll_data(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        Pin::new(&mut self.0).poll_data(cx)
    }

    fn poll_trailers(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Option<hyper::HeaderMap>, Self::Error>> {
        Pin::new(&mut self.0).poll_trailers(cx)
    }
}
