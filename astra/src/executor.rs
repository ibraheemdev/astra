use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake};
use std::thread::{self, Thread};
use std::time::Duration;

use crate::runtime;

pub struct Executor {
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
    pub fn new() -> Self {
        Self {
            shared: Mutex::new(Shared {
                queue: VecDeque::new(),
                workers: 0,
                idle: 0,
                notified: 0,
            }),
            condvar: Condvar::new(),
            keep_alive: Duration::from_secs(6),
            max_workers: num_cpus::get() * 15,
        }
    }
}

impl Executor {
    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let mut shared = self.shared.lock().unwrap();
        shared.queue.push_back(Box::new(f));

        if shared.idle == 0 {
            if shared.workers != self.max_workers {
                shared.workers += 1;
                std::thread::Builder::new()
                    .name("astra-worker".to_owned())
                    .spawn(move || runtime::executor().run())
                    .unwrap();
            }
        } else {
            shared.idle -= 1;
            shared.notified += 1;
            self.condvar.notify_one();
        }
    }

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
