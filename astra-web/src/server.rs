use crate::handler::Handler;
use crate::router::{Group, Routes, SharedRouter};
use crate::{FromRequest, IntoResponse, Layer};

use std::io;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use http::Method;

pub struct Router<L> {
    router: Routes<L>,
}

impl Router<()> {
    pub fn new() -> Router<()> {
        Router {
            router: Routes::new(),
        }
    }
}

impl<L> Router<L>
where
    L: Layer,
{
    pub fn serve(self, addr: impl ToSocketAddrs) -> io::Result<()> {
        let router = SharedRouter {
            routes: Arc::new(self.router.routes),
            layer: self.router.layer,
        };

        astra::Server::bind(addr).serve_clone(move |req, _| router.serve(req))
    }

    pub fn layer<O>(self, layer: O) -> Router<impl Layer>
    where
        O: Layer,
    {
        Router {
            router: self.router.layer(layer),
        }
    }

    pub fn route<H, R>(mut self, path: &str, method: Method, handler: H) -> Router<L>
    where
        H: Handler<R>,
        H::Response: IntoResponse,
        R: FromRequest,
    {
        self.router.route(path, method, handler).unwrap();
        self
    }

    pub fn group<O>(mut self, prefix: &str, group: Group<O>) -> Router<L>
    where
        O: Layer,
    {
        self.router.group(prefix, group).unwrap();
        self
    }

    pub fn get<H, R>(self, path: &str, handler: H) -> Router<L>
    where
        H: Handler<R>,
        H::Response: IntoResponse,
        R: FromRequest,
    {
        self.route(path, Method::GET, handler)
    }

    route!(put => PUT);
    route!(post => POST);
    route!(head => HEAD);
    route!(patch => PATCH);
    route!(delete => DELETE);
    route!(options => OPTIONS);
}

macro_rules! route {
    ($name:ident => $method:ident) => {
        pub fn $name<H, R>(self, path: &str, handler: H) -> Router<L>
        where
            H: Handler<R>,
            H::Response: IntoResponse,
            R: FromRequest,
        {
            self.route(path, Method::$method, handler)
        }
    };
}

pub(crate) use route;
