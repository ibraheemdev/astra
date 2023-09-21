use crate::executor;

use core::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{cmp, debug_assert, io};

use futures_core::Stream;
use hyper::body::HttpBody;

pub use hyper::body::Bytes;

/// An HTTP request.
///
/// See [`http::Request`](hyper::Request) and [`Body`] for details.
pub type Request = hyper::Request<Body>;

/// An HTTP response.
///
/// You can create a response with the [`new`](hyper::Response::new) method:
///
/// ```
/// # use astra::{Response, Body};
/// let response = Response::new(Body::new("Hello world!"));
/// ```
///
/// Or with a [`ResponseBuilder`]:
///
/// ```
/// # use astra::{ResponseBuilder, Body};
/// let response = ResponseBuilder::new()
///     .status(404)
///     .header("X-Custom-Foo", "Bar")
///     .body(Body::new("Page not found."))
///     .unwrap();
/// ```
///
/// See [`http::Response`](hyper::Response) and [`Body`] for details.
pub type Response = hyper::Response<Body>;

/// A builder for an HTTP response.
///
/// ```
/// use astra::{ResponseBuilder, Body};
///
/// let response = ResponseBuilder::new()
///     .status(404)
///     .header("X-Custom-Foo", "Bar")
///     .body(Body::new("Page not found."))
///     .unwrap();
/// ```
///
/// See [`http::Response`](hyper::Response) and [`Body`] for details.
pub type ResponseBuilder = hyper::http::response::Builder;

/// The streaming body of an HTTP request or response.
///
/// Data is streamed by iterating over the body, which
/// yields chunks as [`Bytes`](hyper::body::Bytes).
///
/// ```rust
/// use astra::{Request, Response, Body};
///
/// fn handle(mut req: Request) -> Response {
///     for chunk in req.body_mut() {
///         println!("body chunk {:?}", chunk);
///     }
///
///     Response::new(Body::new("Hello World!"))
/// }
/// ```
pub struct Body(pub(crate) hyper::Body);

impl Body {
    /// Create a body from a string or bytes.
    ///
    /// ```rust
    /// # use astra::Body;
    /// let string = Body::new("Hello world!");
    /// let bytes = Body::new(vec![0, 1, 0, 1, 0]);
    /// ```
    pub fn new(data: impl Into<Bytes>) -> Body {
        Body(hyper::Body::from(data.into()))
    }

    /// Create an empty body.
    pub fn empty() -> Body {
        Body(hyper::Body::empty())
    }

    /// Create a body from an implementor of [`io::Read`].
    ///
    /// ```rust
    /// use astra::{Request, Response, ResponseBuilder, Body};
    /// use std::fs::File;
    ///
    /// fn handle(_request: Request) -> Response {
    ///     let file = File::open("index.html").unwrap();
    ///
    ///     ResponseBuilder::new()
    ///         .header("Content-Type", "text/html")
    ///         .body(Body::wrap_reader(file))
    ///         .unwrap()
    /// }
    /// ```
    pub fn wrap_reader<R>(reader: R) -> Body
    where
        R: io::Read + Send + 'static,
    {
        Body(hyper::Body::wrap_stream(ReaderStream::new(reader)))
    }

    /// Create a [`BodyReader`] that implements [`std::io::Read`].
    pub fn reader(&mut self) -> BodyReader<'_> {
        BodyReader {
            body: self,
            prev_bytes: Bytes::new(),
        }
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
        executor::Parker::new()
            .block_on(self.0.data())
            .map(|res| res.map_err(|err| io::Error::new(io::ErrorKind::Other, err)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        Stream::size_hint(&self.0)
    }
}

/// Wraps [`Body`] and implements [`std::io::Read`]
pub struct BodyReader<'b> {
    body: &'b mut Body,
    prev_bytes: Bytes,
}

impl<'b> std::io::Read for BodyReader<'b> {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        let mut written = 0;
        loop {
            if buf.is_empty() {
                return Ok(written);
            }

            if !self.prev_bytes.is_empty() {
                let chunk_size = cmp::min(buf.len(), self.prev_bytes.len());
                let prev_bytes_start = self.prev_bytes.split_to(chunk_size);
                buf[..chunk_size].copy_from_slice(&prev_bytes_start[..]);
                buf = &mut buf[chunk_size..];
                written += chunk_size;
                continue;
            }

            if written != 0 {
                // pulling from an interator can block, and we have something to return
                // already, so return it
                return Ok(written);
            }

            debug_assert!(self.prev_bytes.is_empty());
            debug_assert!(written == 0);

            self.prev_bytes = if let Some(next) = self.body.next() {
                next?
            } else {
                return Ok(written);
            }
        }
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

struct ReaderStream<R> {
    reader: Option<R>,
    buf: Vec<u8>,
}

const CAP: usize = 4096;

impl<R> ReaderStream<R> {
    fn new(reader: R) -> Self {
        Self {
            reader: Some(reader),
            buf: vec![0; CAP],
        }
    }
}

impl<R> Unpin for ReaderStream<R> {}

impl<R> Stream for ReaderStream<R>
where
    R: io::Read,
{
    type Item = io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let ReaderStream { reader, buf } = &mut *self;

        let reader = match reader {
            Some(reader) => reader,
            None => return Poll::Ready(None),
        };

        if buf.capacity() == 0 {
            buf.extend_from_slice(&[0; CAP]);
        }

        match reader.read(buf) {
            Err(err) => {
                self.reader.take();
                Poll::Ready(Some(Err(err)))
            }
            Ok(0) => {
                self.reader.take();
                Poll::Ready(None)
            }
            Ok(n) => {
                let remaining = buf.split_off(n);
                let chunk = std::mem::replace(buf, remaining);
                Poll::Ready(Some(Ok(Bytes::from(chunk))))
            }
        }
    }
}
