use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::{convert, io};

use polling::{Event, Poller};

#[derive(Clone)]
pub struct Reactor {
    shared: Arc<Shared>,
}

struct Shared {
    poller: Poller,
    next_key: AtomicUsize,
    sources: Mutex<HashMap<usize, Arc<Source>>>,
}

impl Reactor {
    pub fn new() -> io::Result<Self> {
        let shared = Arc::new(Shared {
            next_key: AtomicUsize::new(0),
            poller: Poller::new()?,
            sources: Mutex::new(HashMap::with_capacity(64)),
        });

        std::thread::Builder::new()
            .name("hyper-blocking-reactor".to_owned())
            .spawn({
                let shared = shared.clone();
                move || {
                    let mut events = Vec::with_capacity(64);
                    loop {
                        if let Err(err) = shared.poll(&mut events) {
                            log::warn!("Failed to poll reactor {}", err);
                        }

                        events.clear();
                    }
                }
            })?;

        Ok(Reactor { shared })
    }

    pub fn add_source(&self, source: impl polling::Source) -> io::Result<usize> {
        let mut sources = self.shared.sources.lock().unwrap();

        let raw = source.raw();
        let key = self.shared.next_key.fetch_add(1, Ordering::SeqCst);

        self.shared.poller.add(raw, Event::none(key))?;
        sources.insert(
            key,
            Arc::new(Source {
                raw,
                interest: Default::default(),
            }),
        );

        Ok(key)
    }

    pub fn remove_source(&self, key: usize) {
        let source = self.shared.sources.lock().unwrap().remove(&key).unwrap();
        let _ = self.shared.poller.delete(source.raw);
    }

    pub fn poll_io<T, R>(
        &self,
        inner: &T,
        direction: usize,
        key: usize,
        mut f: impl FnMut(&T) -> io::Result<R>,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<R>> {
        loop {
            match f(inner) {
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
        let source = self.shared.get_source(key).unwrap();
        let mut interest = source.interest.lock().unwrap();
        let direction = &mut interest[direction];

        if direction.notified {
            direction.notified = false;
            return Poll::Ready(Ok(()));
        }

        match direction.waker.replace(cx.waker().clone()) {
            Some(w) => w.wake(),
            None => self.shared.modify(&source, &interest, key)?,
        }

        Poll::Pending
    }
}

impl Shared {
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

        for waker in wakers.into_iter().filter_map(convert::identity) {
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
