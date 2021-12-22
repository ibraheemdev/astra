use astra::{Body, Request, Response, Server};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn main() {
    let counter = Arc::new(AtomicUsize::new(0));

    Server::bind("localhost:3000")
        // Pass a handle to the counter along with the request
        .serve(move |req| handle(counter.clone(), req))
        .expect("serve failed");
}

fn handle(counter: Arc<AtomicUsize>, _req: Request) -> Response {
    // Add 1 to the counter, and fetch the current value
    let n = counter.fetch_add(1, Ordering::Relaxed);
    log::debug!("Request #{}", n);

    Response::new(Body::new("Hello world!"))
}
