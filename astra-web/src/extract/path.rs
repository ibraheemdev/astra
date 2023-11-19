use astra::Body;
use http::StatusCode;

use crate::{FromRequest, IntoResponse};

#[derive(Debug)]
pub struct Path<T>(pub T);

impl<T> FromRequest for Path<T>
where
    T: Send + Sync + 'static,
{
    type Error = PathError;

    fn from_request(_request: &mut astra::Request) -> Result<Self, Self::Error> {
        todo!()
    }
}

pub struct PathError {}

impl IntoResponse for PathError {
    fn into_response(self) -> astra::Response {
        astra::ResponseBuilder::new()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::empty())
            .unwrap()
    }
}

impl<T> std::ops::Deref for Path<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
