mod parts;
mod state;
mod path;

pub use state::{State, StateError};

use crate::IntoResponse;

use astra::{Request, Response};

use std::convert::Infallible;

pub trait FromRequest: Sized + Send + Sync + 'static {
    type Error: IntoResponse;

    fn from_request(request: &mut Request) -> Result<Self, Self::Error>;
}

impl FromRequest for () {
    type Error = Infallible;

    fn from_request(_: &mut Request) -> Result<Self, Self::Error> {
        Ok(())
    }
}

impl<T: FromRequest> FromRequest for Option<T> {
    type Error = Infallible;

    fn from_request(request: &mut Request) -> Result<Self, Self::Error> {
        Ok(T::from_request(request).ok())
    }
}

impl<T> FromRequest for Result<T, T::Error>
where
    T: FromRequest,
    T::Error: Send + Sync,
{
    type Error = Infallible;

    fn from_request(request: &mut Request) -> Result<Self, Self::Error> {
        Ok(T::from_request(request))
    }
}

pub trait RequestExt: Sized {
    fn extract<T>(&mut self) -> Result<T, T::Error>
    where
        T: FromRequest;
}

impl RequestExt for Request {
    fn extract<T>(&mut self) -> Result<T, T::Error>
    where
        T: FromRequest,
    {
        T::from_request(self)
    }
}

macro_rules! extract_tuples ({ $($param:ident)* } => {
    impl<$($param,)*> FromRequest for ($($param,)*)
    where
        $($param: FromRequest,)*
    {
        type Error = Response;

        #[inline]
        #[allow(non_snake_case, unused)]
        fn from_request(request: &mut Request) -> Result<Self, Self::Error> {
            $(
                let $param = match $param::from_request(request) {
                    Ok(value) => value,
                    Err(err) => return Err(err.into_response())
                };
            )*

            Ok(($($param,)*))
        }
    }
});

extract_tuples! { A }
extract_tuples! { A B }
extract_tuples! { A B C }
extract_tuples! { A B C D }
extract_tuples! { A B C D E }
extract_tuples! { A B C D E F }
extract_tuples! { A B C D E F G }
extract_tuples! { A B C D E F G H }
extract_tuples! { A B C D E F G H I }
extract_tuples! { A B C D E F G H I J }
extract_tuples! { A B C D E F G H I J K }
extract_tuples! { A B C D E F G H I J K L }
extract_tuples! { A B C D E F G H I J K L M }
extract_tuples! { A B C D E F G H I J K L M N }
extract_tuples! { A B C D E F G H I J K L M N O }
extract_tuples! { A B C D E F G H I J K L M N O P }
