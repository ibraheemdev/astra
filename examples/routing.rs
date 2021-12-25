use astra::{Body, Request, Response, ResponseBuilder, Server};
use matchit::{Match, Node};
use std::collections::HashMap;
use std::sync::Arc;

type Router = Node<fn(Request) -> Response>;
type Params = HashMap<String, String>;

fn main() {
    // Setup the routes
    let router = Arc::new({
        let mut router = Router::new();
        router.insert("/", home).unwrap();
        router.insert("/user/:id", get_user).unwrap();
        router
    });

    Server::bind("localhost:3000")
        // Pass the router to `route`, along with the request
        .serve(move |req| route(router.clone(), req))
        .expect("serve failed");
}

// The handler for "/"
fn home(_: Request) -> Response {
    Response::new(Body::new("Welcome!"))
}

// The handler for "/user/:id"
fn get_user(req: Request) -> Response {
    // Get the routing parameters out of the request
    // extensions, inserted by `route`
    let params = req.extensions().get::<Params>().unwrap();

    // Get the "id" from "/user/:id"
    let id = params.get("id").unwrap();

    Response::new(Body::new(format!("User #{}", id)))
}

fn route(router: Arc<Router>, mut req: Request) -> Response {
    // Try to find the handler for the requested path
    match router.at(req.uri().path()) {
        // If it is found, insert the route parameters
        // into the request extensions to be accessed
        // by the handler, and call it
        Ok(Match { value, params }) => {
            let params = params
                .iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect::<Params>();
            req.extensions_mut().insert(params);
            (value)(req)
        }
        // Otherwise return a 404
        Err(_) => ResponseBuilder::new()
            .status(404)
            .body(Body::empty())
            .unwrap(),
    }
}
