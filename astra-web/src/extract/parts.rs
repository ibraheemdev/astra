use crate::FromRequest;
use astra::Request;

use std::convert::Infallible;

use http::{HeaderMap, Method, Uri, Version};

impl FromRequest for Request {
    type Error = Infallible;

    fn from_request(request: &mut Request) -> Result<Self, Self::Error> {
        Ok(std::mem::take(request))
    }
}

impl FromRequest for HeaderMap {
    type Error = Infallible;

    fn from_request(request: &mut Request) -> Result<Self, Self::Error> {
        Ok(request.headers().clone())
    }
}

impl FromRequest for Method {
    type Error = Infallible;

    fn from_request(request: &mut Request) -> Result<Self, Self::Error> {
        Ok(request.method().clone())
    }
}

impl FromRequest for Uri {
    type Error = Infallible;

    fn from_request(request: &mut Request) -> Result<Self, Self::Error> {
        Ok(request.uri().clone())
    }
}

impl FromRequest for Version {
    type Error = Infallible;

    fn from_request(request: &mut Request) -> Result<Self, Self::Error> {
        Ok(request.version().clone())
    }
}
