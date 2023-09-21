use astra::{Body, Response, ResponseBuilder};

use std::{borrow::Cow, convert::Infallible};

use bytes::Bytes;
use http::{header, StatusCode};

pub trait IntoResponse {
    fn into_response(self) -> Response;
}

impl IntoResponse for () {
    fn into_response(self) -> Response {
        Response::default()
    }
}

impl IntoResponse for Infallible {
    fn into_response(self) -> Response {
        unreachable!()
    }
}

impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}

impl<T> IntoResponse for (StatusCode, T)
where
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut response = self.1.into_response();
        *response.status_mut() = self.0;
        response
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .status(self)
            .body(Body::empty())
            .unwrap()
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            Ok(response) => response.into_response(),
            Err(error) => error.into_response(),
        }
    }
}

impl<T> IntoResponse for Option<T>
where
    T: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            Some(response) => response.into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

macro_rules! with_content_type {
    ($($ty:ty $(|$into:ident)? => $content_type:literal),* $(,)?) => { $(
        impl IntoResponse for $ty {
            fn into_response(self) -> Response {
                ResponseBuilder::new()
                    .header(header::CONTENT_TYPE, $content_type)
                    .body(Body::new(self $(.$into())?))
                    .unwrap()
            }
        })*
    }
}

with_content_type! {
    Bytes => "application/octet-stream",
    Vec<u8> => "application/octet-stream",
    &'static [u8] => "application/octet-stream",
    Cow<'static, [u8]> | into_owned => "application/octet-stream",
    String => "text/plain",
    &'static str => "text/plain",
    Cow<'static, str> | into_owned => "text/plain",
}
