use astra::{Body, Response, Server};

fn main() {
    Server::bind("localhost:3000")
        .serve(|_req| Response::new(Body::new("Hello World!")))
        .expect("failed to start server");
}
