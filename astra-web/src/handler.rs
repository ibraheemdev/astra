use crate::{FromRequest, IntoResponse, Layer};
use astra::{Request, Response};

use std::marker::PhantomData;

pub trait Handler<T>: Send + Sync + 'static {
    type Response: IntoResponse;

    fn call(&self, request: Request) -> Self::Response;

    fn layer<L>(self, layer: L) -> Layered<Self, T, L>
    where
        L: Layer,
        Self: Sized,
    {
        Layered {
            layer,
            handler: self,
            _r: PhantomData,
        }
    }
}

pub struct Layered<H, T, L> {
    layer: L,
    handler: H,
    _r: PhantomData<T>,
}

impl<H, T, W> Handler<Request> for Layered<H, T, W>
where
    H: Handler<T>,
    T: FromRequest,
    W: Layer,
{
    type Response = Response;

    fn call(&self, request: Request) -> Self::Response {
        let next = |req| self.handler.call(req).into_response();
        self.layer.call(request, next)
    }
}

pub(crate) type Erased = Box<dyn Handler<(), Response = Response>>;

impl Handler<Request> for Erased {
    type Response = Response;

    fn call(&self, request: Request) -> Self::Response {
        (**self).call(request)
    }
}

pub(crate) fn erase<H, R>(handler: H) -> Erased
where
    H: Handler<R>,
    R: FromRequest,
    H::Response: IntoResponse,
{
    Box::new(Erase {
        inner: handler,
        _r: PhantomData,
    })
}

struct Erase<T, R> {
    inner: T,
    _r: PhantomData<R>,
}

impl<T, R> Handler<()> for Erase<T, R>
where
    T: Handler<R>,
    R: FromRequest,
    T::Response: IntoResponse,
{
    type Response = Response;

    fn call(&self, request: Request) -> Self::Response {
        self.inner.call(request).into_response()
    }
}

macro_rules! variadic_handler ({ $($param:ident)* } => {
    impl<Func, R, $($param,)*> Handler<($($param,)*)> for Func
    where
        Func: Fn($($param),*) -> R + Send + Sync + 'static,
        $($param: FromRequest,)*
        R: IntoResponse
    {
        type Response = Response;

        #[inline]
        #[allow(non_snake_case, unused)]
        fn call(&self, mut request: Request) -> Self::Response {
            $(
                let $param = match $param::from_request(&mut request) {
                    Ok(value) => value,
                    Err(err) => return err.into_response()
                };
            )*

            (self)($($param),*).into_response()
        }
    }
});

variadic_handler! {}
variadic_handler! { A }
variadic_handler! { A B }
variadic_handler! { A B C }
variadic_handler! { A B C D }
variadic_handler! { A B C D E }
variadic_handler! { A B C D E F }
variadic_handler! { A B C D E F G }
variadic_handler! { A B C D E F G H }
variadic_handler! { A B C D E F G H I }
variadic_handler! { A B C D E F G H I J }
variadic_handler! { A B C D E F G H I J K }
variadic_handler! { A B C D E F G H I J K L }
variadic_handler! { A B C D E F G H I J K L M }
variadic_handler! { A B C D E F G H I J K L M N }
variadic_handler! { A B C D E F G H I J K L M N O }
variadic_handler! { A B C D E F G H I J K L M N O P }
