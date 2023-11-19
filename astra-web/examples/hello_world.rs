use astra::{Request, Response};
use astra_web::{layer_fn, Next, Router};
use http::{Method, Version};

fn hello_world(_request: Request, _method: Method, _version: Version) -> &'static str {
    "Hello, World!"
}

fn get_foo() -> &'static str {
    "Foo."
}

fn create_foo() -> &'static str {
    "Created Foo."
}

fn main() {
    let router = Router::new()
        .get("/", hello_world)
        .nest(
            "/foo",
            Router::new()
                .get("/", get_foo)
                .get("/index", get_foo)
                .post("/new", create_foo),
        )
        .layer(layer_fn(logger));

    astra::Server::bind("localhost:3000")
        .serve(router.into_service())
        .unwrap()
}

fn logger(request: Request, next: Next) -> Response {
    println!("{} {}", request.method(), request.uri());
    next(request)
}
