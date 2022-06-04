use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake};
use std::thread::{self, Thread};
use std::time::Duration;

pub struct Parker {
    thread: Thread,
    parked: AtomicBool,
}

impl Parker {
    pub fn new() -> Arc<Self> {
        Arc::new(Parker {
            thread: thread::current(),
            // start off as parked to ensure wakeups
            // are seen in between polling and parking
            parked: AtomicBool::new(true),
        })
    }
}

impl Wake for Parker {
    fn wake(self: Arc<Self>) {
        if self.parked.swap(false, Ordering::Release) {
            self.thread.unpark();
        }
    }
}

impl Parker {
    pub fn block_on<F>(self: &Arc<Self>, mut fut: F) -> F::Output
    where
        F: Future,
    {
        let waker = self.clone().into();
        let mut cx = Context::from_waker(&waker);

        let mut fut = unsafe { Pin::new_unchecked(&mut fut) };
        loop {
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(res) => break res,
                Poll::Pending => {
                    // wait for a real wakeup
                    while self.parked.swap(true, Ordering::Acquire) {
                        thread::park();
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct Executor {
    inner: Arc<Inner>,
}

struct Inner {
    keep_alive: Duration,
    shared: Mutex<Shared>,
    max_workers: usize,
    condvar: Condvar,
}

struct Shared {
    queue: VecDeque<Box<dyn Future<Output = ()> + Send>>,
    workers: usize,
    idle: usize,
    notified: usize,
}

impl Executor {
    pub fn new(max_workers: Option<usize>, keep_alive: Option<Duration>) -> Self {
        Self {
            inner: Arc::new(Inner {
                shared: Mutex::new(Shared {
                    queue: VecDeque::new(),
                    workers: 0,
                    idle: 0,
                    notified: 0,
                }),
                condvar: Condvar::new(),
                keep_alive: keep_alive.unwrap_or_else(|| Duration::from_secs(6)),
                max_workers: max_workers.unwrap_or_else(|| num_cpus::get() * 15),
            }),
        }
    }
}

impl<F> hyper::rt::Executor<F> for Executor
where
    F: Future<Output = ()> + Send + 'static,
{
    fn execute(&self, fut: F) {
        let mut shared = self.inner.shared.lock().unwrap();
        shared.queue.push_back(Box::new(fut));

        if shared.idle == 0 {
            if shared.workers != self.inner.max_workers {
                shared.workers += 1;
                let inner = self.inner.clone();
                std::thread::Builder::new()
                    .name("astra-worker".to_owned())
                    .spawn(move || inner.run())
                    .unwrap();
            }
        } else {
            shared.idle -= 1;
            shared.notified += 1;
            self.inner.condvar.notify_one();
        }
    }
}

impl Inner {
    fn run(&self) {
        let parker = Parker::new();

        let mut shared = self.shared.lock().unwrap();

        'alive: loop {
            while let Some(task) = shared.queue.pop_front() {
                drop(shared);
                parker.block_on(Pin::from(task));
                shared = self.shared.lock().unwrap();
            }

            shared.idle += 1;

            loop {
                let (guard, timeout) = self.condvar.wait_timeout(shared, self.keep_alive).unwrap();
                shared = guard;

                if shared.notified != 0 {
                    shared.notified -= 1;
                    continue 'alive;
                }

                if timeout.timed_out() {
                    break 'alive;
                }
            }
        }

        shared.workers -= 1;
        shared.idle -= 1;
    }
}
