use crate::reactor::{READ, WRITE};
use crate::runtime;

use std::io::{self, Read, Write};
use std::net::{self as sys, Shutdown};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub struct TcpStream {
    sys: sys::TcpStream,
    key: usize,
}

impl TcpStream {
    pub fn from_std(sys: sys::TcpStream) -> io::Result<Self> {
        sys.set_nonblocking(true)?;
        runtime::reactor()
            .register(&sys)
            .map(|key| Self { sys, key })
    }

    pub fn sys(&self) -> &sys::TcpStream {
        &self.sys
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let unfilled = buf.initialize_unfilled();

        runtime::reactor()
            .poll_source(READ, self.key, || (&self.sys).read(unfilled), cx)
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
        runtime::reactor().poll_source(WRITE, self.key, || (&self.sys).write(buf), cx)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        runtime::reactor().poll_source(WRITE, self.key, || (&self.sys).flush(), cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(self.sys.shutdown(Shutdown::Write))
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let _ = runtime::reactor().deregister(self.key);
    }
}
