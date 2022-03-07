use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{self as sys, Shutdown};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use mio::{Events, Token};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[derive(Clone)]
pub struct Reactor {
    shared: Arc<Shared>,
}

struct Shared {
    registry: mio::Registry,
    next_key: AtomicUsize,
    sources: Mutex<HashMap<Token, Arc<Source>>>,
}

impl Reactor {
    pub fn new() -> io::Result<Self> {
        let poll = mio::Poll::new()?;
        let shared = Arc::new(Shared {
            next_key: AtomicUsize::new(0),
            registry: poll.registry().try_clone()?,
            sources: Mutex::new(HashMap::with_capacity(64)),
        });

        std::thread::Builder::new()
            .name("astra-reactor".to_owned())
            .spawn({
                let shared = shared.clone();
                move || shared.run(poll)
            })?;

        Ok(Reactor { shared })
    }

    pub fn register(&self, sys: sys::TcpStream) -> io::Result<TcpStream> {
        sys.set_nonblocking(true)?;
        let mut sys = mio::net::TcpStream::from_std(sys);

        let token = Token(self.shared.next_key.fetch_add(1, Ordering::Relaxed));

        self.shared.registry.register(
            &mut sys,
            token,
            mio::Interest::READABLE | mio::Interest::WRITABLE,
        )?;

        let source = Arc::new(Source {
            token,
            interest: Default::default(),
        });

        {
            let mut sources = self.shared.sources.lock().unwrap();
            sources.insert(token, source.clone());
        }

        Ok(TcpStream {
            sys,
            source,
            reactor: self.clone(),
        })
    }

    fn poll_ready(
        &self,
        source: &Source,
        direction: usize,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        let mut interest = source.interest.lock().unwrap();
        let direction = &mut interest[direction];

        if direction.triggered {
            return Poll::Ready(Ok(()));
        }

        match &mut direction.waker {
            Some(existing) if existing.will_wake(cx.waker()) => {}
            _ => {
                direction.waker = Some(cx.waker().clone());
            }
        }

        Poll::Pending
    }

    fn clear_readiness(&self, source: &Source, direction: usize) {
        source.interest.lock().unwrap()[direction].triggered = false;
    }
}

impl Shared {
    fn run(&self, mut poll: mio::Poll) -> io::Result<()> {
        let mut events = Events::with_capacity(64);
        let mut wakers = Vec::new();

        loop {
            if let Err(err) = self.poll(&mut poll, &mut events, &mut wakers) {
                log::warn!("Failed to poll reactor: {}", err);
            }

            events.clear();
        }
    }

    fn poll(
        &self,
        poll: &mut mio::Poll,
        events: &mut Events,
        wakers: &mut Vec<Waker>,
    ) -> io::Result<()> {
        if let Err(err) = poll.poll(events, None) {
            if err.kind() != io::ErrorKind::Interrupted {
                return Err(err);
            }

            return Ok(());
        }

        for event in events.iter() {
            let source = {
                let sources = self.sources.lock().unwrap();
                match sources.get(&event.token()) {
                    Some(source) => source.clone(),
                    None => continue,
                }
            };

            let mut interest = source.interest.lock().unwrap();

            if event.is_readable() {
                interest[READ].triggered = true;
                if let Some(waker) = interest[READ].waker.take() {
                    wakers.push(waker);
                }
            }

            if event.is_writable() {
                interest[WRITE].triggered = true;
                if let Some(waker) = interest[WRITE].waker.take() {
                    wakers.push(waker);
                }
            }
        }

        for waker in wakers.drain(..) {
            waker.wake();
        }

        Ok(())
    }
}

pub const READ: usize = 0;
pub const WRITE: usize = 1;

struct Source {
    interest: Mutex<[Interest; 2]>,
    token: Token,
}

#[derive(Default)]
struct Interest {
    waker: Option<Waker>,
    triggered: bool,
}

pub struct TcpStream {
    sys: mio::net::TcpStream,
    reactor: Reactor,
    source: Arc<Source>,
}

impl TcpStream {
    pub fn poll_io<T>(
        &self,
        direction: usize,
        mut f: impl FnMut() -> io::Result<T>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<T>> {
        loop {
            if self
                .reactor
                .poll_ready(&self.source, direction, cx)?
                .is_pending()
            {
                return Poll::Pending;
            }

            match f() {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                    self.reactor.clear_readiness(&self.source, direction);
                }
                val => return Poll::Ready(val),
            }
        }
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let unfilled = buf.initialize_unfilled();

        self.poll_io(READ, || (&self.sys).read(unfilled), cx)
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
        self.poll_io(WRITE, || (&self.sys).write(buf), cx)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_io(WRITE, || (&self.sys).flush(), cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(self.sys.shutdown(Shutdown::Write))
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let mut sources = self.reactor.shared.sources.lock().unwrap();
        let _ = sources.remove(&self.source.token);
        let _ = self.reactor.shared.registry.deregister(&mut self.sys);
    }
}
