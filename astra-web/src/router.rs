use crate::handler::{self, Handler};
use crate::{FromRequest, IntoResponse, Layer};
use astra::{Body, Request, Response, ResponseBuilder};

use std::collections::HashMap;
use std::sync::Arc;

use http::{Method, StatusCode};

pub struct Router<L> {
    pub routes: Vec<(Method, Vec<(String, handler::Erased)>)>,
    pub layer: L,
}

struct Params(HashMap<String, String>);

impl Router<()> {
    pub fn new() -> Router<()> {
        Router {
            routes: Vec::new(),
            layer: (),
        }
    }
}

impl<L> Router<L>
where
    L: Layer,
{
    pub fn layer<O>(self, layer: O) -> Router<impl Layer>
    where
        O: Layer,
    {
        Router {
            layer: self.layer.layer(layer),
            routes: self.routes,
        }
    }

    pub fn route<H, R>(self, path: &str, method: Method, handler: H) -> Router<L>
    where
        H: Handler<R>,
        H::Response: IntoResponse,
        R: FromRequest,
    {
        self.route_erased(path, method, handler::erase(handler))
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

    pub fn route_erased(
        mut self,
        path: &str,
        method: Method,
        handler: handler::Erased,
    ) -> Router<L> {
        if let Some(routes) = self.routes_mut(&method) {
            routes.push((path.to_owned(), handler));
            return self;
        }

        self.routes.push((method, vec![(path.to_owned(), handler)]));
        self
    }

    pub fn nest<O>(mut self, prefix: &str, router: Router<O>) -> Router<L>
    where
        O: Layer,
    {
        let mut prefix = prefix.to_owned();
        if !prefix.ends_with("/") {
            prefix.push('/');
        }

        for (method, routes) in router.routes {
            for (path, handler) in routes {
                let path = format!("{}{}", prefix, path);
                let handler = handler.layer(router.layer.clone());
                self = self.route_erased(&path, method.clone(), handler::erase(handler));
            }
        }

        self
    }

    fn routes_mut(&mut self, method: &Method) -> Option<&mut Vec<(String, handler::Erased)>> {
        self.routes
            .iter_mut()
            .find(|(m, _)| m == method)
            .map(|(_, router)| router)
    }

    pub fn into_service(self) -> impl astra::Service {
        let router = SharedRouter::new(self);
        move |req, _| router.serve(req)
    }
}

#[derive(Clone)]
pub struct SharedRouter<L> {
    pub routes: Arc<Vec<(Method, matchit::Router<handler::Erased>)>>,
    pub layer: L,
}

impl<L> SharedRouter<L> {
    pub fn new(router: Router<L>) -> SharedRouter<L> {
        let routes = router.routes.into_iter().map(|(method, routes)| {
            let mut router = matchit::Router::new();
            for (path, handler) in routes {
                router.insert(path, handler).unwrap();
            }
            (method, router)
        });

        SharedRouter {
            routes: Arc::new(routes.collect()),
            layer: router.layer,
        }
    }
}

impl<L> SharedRouter<L>
where
    L: Layer,
{
    pub fn serve(&self, mut request: Request) -> Response {
        let path = request.uri().path();

        match self.routes(request.method()) {
            Some(router) => match router.at(path) {
                Ok(matched) => {
                    let handler = matched.value;

                    let params = matched
                        .params
                        .iter()
                        .map(|(k, v)| (k.to_owned(), v.to_owned()))
                        .collect::<HashMap<_, _>>();

                    request.extensions_mut().insert(Params(params));

                    return self.layer.call(request, |req| handler.call(req));
                }
                Err(_) => ResponseBuilder::new()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::empty())
                    .unwrap(),
            },
            None => ResponseBuilder::new()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::empty())
                .unwrap(),
        }
    }

    fn routes(&self, method: &Method) -> Option<&matchit::Router<handler::Erased>> {
        self.routes
            .iter()
            .find(|(m, _)| m == method)
            .map(|(_, router)| router)
    }
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
