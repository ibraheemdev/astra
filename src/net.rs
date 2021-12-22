use crate::reactor::{Reactor, READ, WRITE};

use std::io::{self, Read, Write};
use std::net::{Shutdown, ToSocketAddrs};
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::server::accept::Accept;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub struct TcpListener {
    inner: std::net::TcpListener,
    reactor: Reactor,
    key: usize,
}

impl TcpListener {
    pub fn bind(reactor: Reactor, addrs: impl ToSocketAddrs) -> io::Result<Self> {
        let inner = std::net::TcpListener::bind(addrs)?;
        inner.set_nonblocking(true)?;
        let key = reactor.add_source(&inner)?;
        Ok(TcpListener {
            reactor,
            inner,
            key,
        })
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        let _ = self.reactor.remove_source(self.key);
    }
}

impl Accept for TcpListener {
    type Conn = TcpStream;
    type Error = io::Error;

    fn poll_accept(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<io::Result<TcpStream>>> {
        self.reactor
            .poll_io(&self.inner, READ, self.key, |inner| inner.accept(), cx)
            .map(|result| {
                let (inner, _) = result?;
                inner.set_nonblocking(true)?;

                Ok(TcpStream {
                    reactor: self.reactor.clone(),
                    key: self.reactor.add_source(&inner)?,
                    inner,
                })
            })
            .map(Some)
    }
}

pub struct TcpStream {
    inner: std::net::TcpStream,
    reactor: Reactor,
    key: usize,
}

impl AsyncRead for TcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let unfilled = buf.initialize_unfilled();

        self.reactor
            .poll_io(
                &self.inner,
                READ,
                self.key,
                |mut inner| inner.read(unfilled),
                cx,
            )
            .map_ok(|read| {
                buf.advance(read);
            })
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.reactor.poll_io(
            &self.inner,
            WRITE,
            self.key,
            |mut inner| inner.write(buf),
            cx,
        )
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.reactor
            .poll_io(&self.inner, WRITE, self.key, |mut inner| inner.flush(), cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(self.inner.shutdown(Shutdown::Write))
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let _ = self.reactor.remove_source(self.key);
    }
}
