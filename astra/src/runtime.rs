use crate::executor::{self, Executor};
use crate::reactor::Reactor;

use std::future::Future;

use once_cell::sync::Lazy;

static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime {
    executor: Executor::new(),
    reactor: Reactor::new().expect("failed to start reactor"),
});

struct Runtime {
    executor: Executor,
    reactor: Reactor,
}

pub fn spawn<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    RUNTIME.executor.spawn(f);
}

pub fn spawn_future<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    RUNTIME.executor.spawn(move || block_on(fut));
}

pub fn block_on<F>(fut: F) -> F::Output
where
    F: Future,
{
    executor::block_on(fut)
}

pub(crate) fn reactor() -> &'static Reactor {
    &RUNTIME.reactor
}

pub(crate) fn executor() -> &'static Executor {
    &RUNTIME.executor
}
