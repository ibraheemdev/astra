use crate::handler::{self, Handler};
use crate::{FromRequest, IntoResponse, Layer};
use astra::{Body, Request, Response, ResponseBuilder};

use std::collections::HashMap;
use std::sync::Arc;

use http::{Method, StatusCode};

pub struct Routes<L> {
    pub routes: Vec<(Method, matchit::Router<handler::Erased>)>,
    pub layer: L,
}

#[derive(Clone)]
pub struct SharedRouter<L> {
    pub routes: Arc<Vec<(Method, matchit::Router<handler::Erased>)>>,
    pub layer: L,
}

struct Params(HashMap<String, String>);

impl Routes<()> {
    pub fn new() -> Routes<()> {
        Routes {
            routes: Vec::new(),
            layer: (),
        }
    }
}

impl<L> Routes<L>
where
    L: Layer,
{
    pub fn layer<O>(self, layer: O) -> Routes<impl Layer>
    where
        O: Layer,
    {
        Routes {
            layer: self.layer.layer(layer),
            routes: self.routes,
        }
    }

    pub fn route<H, R>(
        &mut self,
        path: &str,
        method: Method,
        handler: H,
    ) -> Result<(), matchit::InsertError>
    where
        H: Handler<R>,
        H::Response: IntoResponse,
        R: FromRequest,
    {
        self.route_erased(path, method, handler::erase(handler))
    }

    pub fn route_erased(
        &mut self,
        path: &str,
        method: Method,
        handler: handler::Erased,
    ) -> Result<(), matchit::InsertError> {
        if let Some(routes) = self.routes_mut(&method) {
            return routes.insert(path, handler);
        }

        self.routes.push((method, matchit::Router::new()));
        let router = &mut self.routes.last_mut().unwrap().1;
        router.insert(path, handler)
    }

    pub fn group<O>(&mut self, prefix: &str, group: Group<O>) -> Result<(), matchit::InsertError>
    where
        O: Layer,
    {
        let mut prefix = prefix.to_owned();
        if !prefix.ends_with("/") {
            prefix.push('/');
        }

        for (method, path, handler) in group.routes {
            let path = format!("{}{}", prefix, path);
            let handler = handler.layer(group.layer.clone());
            self.route_erased(&path, method, handler::erase(handler))?;
        }

        Ok(())
    }

    fn routes_mut(&mut self, method: &Method) -> Option<&mut matchit::Router<handler::Erased>> {
        self.routes
            .iter_mut()
            .find(|(m, _)| m == method)
            .map(|(_, router)| router)
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

pub struct Group<L> {
    pub routes: Vec<(Method, String, handler::Erased)>,
    pub layer: L,
}

impl Group<()> {
    pub fn new() -> Group<()> {
        Group {
            routes: Vec::new(),
            layer: (),
        }
    }
}

impl<L> Group<L>
where
    L: Layer,
{
    pub fn layer<O>(self, layer: O) -> Group<impl Layer>
    where
        O: Layer,
    {
        Group {
            layer: self.layer.layer(layer),
            routes: self.routes,
        }
    }

    pub fn route<H, R>(mut self, path: &str, method: Method, handler: H) -> Group<L>
    where
        H: Handler<R>,
        H::Response: IntoResponse,
        R: FromRequest,
    {
        self.routes
            .push((method, path.to_owned(), handler::erase(handler)));
        self
    }

    pub fn get<H, R>(self, path: &str, handler: H) -> Group<L>
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
        pub fn $name<H, R>(self, path: &str, handler: H) -> Group<L>
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
