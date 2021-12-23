use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use polling::{Event, Poller};

use crate::runtime;

pub struct Reactor {
    poller: Poller,
    next_key: AtomicUsize,
    sources: Mutex<HashMap<usize, Arc<Source>>>,
}

impl Reactor {
    pub fn new() -> io::Result<Self> {
        let reactor = Self {
            next_key: AtomicUsize::new(0),
            poller: Poller::new()?,
            sources: Mutex::new(HashMap::with_capacity(64)),
        };

        std::thread::Builder::new()
            .name("astra-reactor".to_owned())
            .spawn(move || runtime::reactor().run())?;

        Ok(reactor)
    }

    pub fn register(&self, source: impl polling::Source) -> io::Result<usize> {
        let mut sources = self.sources.lock().unwrap();
        let key = self.next_key.fetch_add(1, Ordering::SeqCst);

        let raw = source.raw();

        self.poller.add(raw, Event::none(key))?;
        sources.insert(
            key,
            Arc::new(Source {
                raw,
                interest: Default::default(),
            }),
        );

        Ok(key)
    }

    pub fn deregister(&self, key: usize) -> io::Result<()> {
        let mut sources = self.sources.lock().unwrap();
        let source = sources.remove(&key).unwrap();
        self.poller.delete(source.raw)
    }

    pub fn poll_source<R>(
        &self,
        direction: usize,
        key: usize,
        mut f: impl FnMut() -> io::Result<R>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<R>> {
        loop {
            match f() {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }

            if self.poll_ready(key, direction, cx)?.is_pending() {
                return Poll::Pending;
            }
        }
    }

    fn poll_ready(
        &self,
        key: usize,
        direction: usize,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        let source = self.get_source(key).unwrap();
        let mut interest = source.interest.lock().unwrap();
        let direction = &mut interest[direction];

        if direction.notified {
            direction.notified = false;
            return Poll::Ready(Ok(()));
        }

        match direction.waker.replace(cx.waker().clone()) {
            Some(w) => w.wake(),
            None => self.modify(&source, &interest, key)?,
        }

        Poll::Pending
    }

    fn run(&self) -> io::Result<()> {
        let mut events = Vec::with_capacity(64);
        loop {
            if let Err(err) = self.poll(&mut events) {
                log::warn!("Failed to poll reactor {}", err);
            }

            events.clear();
        }
    }

    fn poll(&self, events: &mut Vec<Event>) -> io::Result<()> {
        if let Err(err) = self.poller.wait(events, None) {
            if err.kind() != io::ErrorKind::Interrupted {
                return Err(err);
            }

            return Ok(());
        }

        let mut wakers = Vec::new();

        for event in events.iter() {
            let source = match self.get_source(event.key) {
                Some(source) => source.clone(),
                None => continue,
            };

            let mut interest = source.interest.lock().unwrap();

            if event.readable {
                interest[READ].notified = true;
                wakers.push(interest[READ].waker.take());
            }

            if event.writable {
                interest[WRITE].notified = true;
                wakers.push(interest[WRITE].waker.take());
            }

            if interest.iter().any(|i| i.waker.is_some()) {
                self.modify(&source, &interest, event.key)?;
            }
        }

        for waker in wakers.into_iter().flatten() {
            waker.wake();
        }

        Ok(())
    }

    fn modify(&self, source: &Source, interest: &[Interest; 2], key: usize) -> io::Result<()> {
        let event = Event {
            key,
            readable: interest[READ].waker.is_some(),
            writable: interest[WRITE].waker.is_some(),
        };

        self.poller.modify(source.raw, event)
    }

    fn get_source(&self, key: usize) -> Option<Arc<Source>> {
        let sources = self.sources.lock().unwrap();
        sources.get(&key).cloned()
    }
}

pub const READ: usize = 0;
pub const WRITE: usize = 1;

struct Source {
    #[cfg(unix)]
    raw: std::os::unix::io::RawFd,
    #[cfg(windows)]
    raw: std::os::windows::io::RawSocket,
    interest: Mutex<[Interest; 2]>,
}

#[derive(Default)]
struct Interest {
    waker: Option<Waker>,
    notified: bool,
}
