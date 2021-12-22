use hyper_blocking::{Body, Response, Server};

fn main() {
    tracing_subscriber::fmt::init();
    Server::bind("localhost:3000")
        .serve(|_req| Response::new(Body::from("Hello World")))
        .expect("failed to serve");
}
