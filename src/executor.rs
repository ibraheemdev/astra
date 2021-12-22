use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake};
use std::thread::{self, Thread};
use std::time::Duration;

pub fn block_on<F>(mut fut: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    pub struct Unpark(Thread);

    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    let mut fut = unsafe { Pin::new_unchecked(&mut fut) };
    let waker = Arc::new(Unpark(thread::current())).into();
    let mut cx = Context::from_waker(&waker);

    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(res) => break res,
            Poll::Pending => thread::park(),
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
    queue: VecDeque<Box<dyn FnOnce() + Send>>,
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
                keep_alive: keep_alive.unwrap_or(Duration::from_secs(6)),
                max_workers: max_workers.unwrap_or(num_cpus::get() * 10),
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
        shared.queue.push_back(Box::new(|| block_on(fut)));

        if shared.idle == 0 {
            if shared.workers != self.inner.max_workers {
                shared.workers += 1;
                let inner = self.inner.clone();
                thread::spawn(move || inner.run());
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
        let mut shared = self.shared.lock().unwrap();

        'alive: loop {
            while let Some(task) = shared.queue.pop_front() {
                drop(shared);
                task();
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
