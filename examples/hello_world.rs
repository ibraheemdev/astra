use hyper_blocking::{Body, Response, Server};

fn main() {
    Server::bind("localhost:3000")
        .max_workers(usize::MAX)
        .serve(|_req| Response::new(Body::from("Hello World")))
        .expect("failed to serve");
}
