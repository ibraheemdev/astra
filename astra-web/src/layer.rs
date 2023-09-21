use astra::{Request, Response};

pub trait Layer: Send + Clone + Sync + 'static {
    fn call(&self, request: Request, next: impl Fn(Request) -> Response) -> Response;

    fn layer<L>(self, layer: L) -> And<Self, L>
    where
        L: Layer,
        Self: Sized,
    {
        And {
            inner: self,
            outer: layer,
        }
    }
}

impl Layer for () {
    fn call(&self, request: Request, next: impl Fn(Request) -> Response) -> Response {
        next(request)
    }
}

pub struct And<I, O> {
    inner: I,
    outer: O,
}

impl<I, O> Clone for And<I, O>
where
    I: Clone,
    O: Clone,
{
    fn clone(&self) -> Self {
        And {
            inner: self.inner.clone(),
            outer: self.outer.clone(),
        }
    }
}

impl<I, O> Layer for And<I, O>
where
    I: Layer,
    O: Layer,
{
    fn call(&self, request: Request, next: impl Fn(Request) -> Response) -> Response {
        let next = |request| self.inner.call(request, &next);
        self.outer.call(request, next)
    }
}

pub type Next<'a> = &'a dyn Fn(Request) -> Response;

pub fn layer_fn<F>(f: F) -> impl Layer
where
    for<'n> F: Fn(Request, Next<'n>) -> Response + Send + Clone + Sync + 'static,
{
    #[derive(Clone)]
    struct Impl<F>(F);

    impl<F> Layer for Impl<F>
    where
        for<'n> F: Fn(Request, Next<'n>) -> Response + Send + Clone + Sync + 'static,
    {
        fn call(&self, request: Request, next: impl Fn(Request) -> Response) -> Response {
            (self.0)(request, &next)
        }
    }

    Impl(f)
}
