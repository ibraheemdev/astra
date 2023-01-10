use std::collections::HashMap;
use std::sync::Arc;

use astra::{Body, Request, Response, ResponseBuilder, Server};
use matchit::Match;

type Router = matchit::Router<fn(Request) -> Response>;
type Params = HashMap<String, String>;

// GET '/'
fn home(_: Request) -> Response {
    Response::new(Body::new("Welcome!"))
}

// GET '/user/:id'
fn get_user(req: Request) -> Response {
    // Retreive route parameters from the the request extensions
    let params = req.extensions().get::<Params>().unwrap();

    // Get the 'id' from '/user/:id'
    let id = params.get("id").unwrap();

    Response::new(Body::new(format!("User #{}", id)))
}

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

fn route(router: Arc<Router>, mut req: Request) -> Response {
    // Try to find the handler for the requested path
    match router.at(req.uri().path()) {
        // If a handler is found, insert the route parameters into the request
        // extensions, and call it
        Ok(Match { value, params }) => {
            let params = params
                .iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
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
