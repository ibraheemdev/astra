use astra::{Request, Response};
use astra_web::{layer_fn, Group, Next, Router};
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
    Router::new()
        .get("/", hello_world)
        .group(
            "/foo",
            Group::new()
                .get("/", get_foo)
                .get("/index", get_foo)
                .post("/new", create_foo),
        )
        .layer(layer_fn(logger))
        .serve("localhost:3000")
        .unwrap()
}

fn logger(request: Request, next: Next) -> Response {
    println!("{} {}", request.method(), request.uri());
    next(request)
}
