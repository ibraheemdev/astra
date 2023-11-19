use crate::{FromRequest, IntoResponse};
use astra::{Body, Response, ResponseBuilder};
use http::StatusCode;

#[derive(Debug, Default, Clone, Copy)]
pub struct State<T>(T);

impl<T> FromRequest for State<T>
where
    T: Clone + Send + Sync + 'static,
{
    type Error = StateError;

    fn from_request(request: &mut astra::Request) -> Result<Self, Self::Error> {
        request.extensions().get().cloned().ok_or(StateError)
    }
}

pub struct StateError;

impl IntoResponse for StateError {
    fn into_response(self) -> Response {
        ResponseBuilder::new()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap()
    }
}
