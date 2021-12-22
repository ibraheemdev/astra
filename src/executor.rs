use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake};
use std::thread::{self, Thread};
use std::time::Duration;

pub fn block_on<F>(mut fut: F) -> F::Output
where
    F: Future,
{
    pub struct Unpark {
        thread: Thread,
        unparked: AtomicBool,
    }

    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            if !self.unparked.swap(true, Ordering::Relaxed) {
                self.thread.unpark();
            }
        }
    }

    let unpark = Arc::new(Unpark {
        thread: thread::current(),
        unparked: false.into(),
    });

    let waker = unpark.clone().into();
    let mut cx = Context::from_waker(&waker);

    let mut fut = unsafe { Pin::new_unchecked(&mut fut) };
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(res) => break res,
            Poll::Pending => {
                if !unpark.unparked.swap(false, Ordering::Acquire) {
                    thread::park();
                    unpark.unparked.store(false, Ordering::Release);
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
                max_workers: max_workers.unwrap_or(num_cpus::get() * 15),
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
