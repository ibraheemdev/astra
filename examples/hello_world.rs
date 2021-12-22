use hyper_blocking::{Body, Request, Response, Server};

fn main() {
    Server::bind("localhost:3000")
        .serve(|mut req: Request| {
            println!("incoming {:?}", req.uri());

            for chunk in req.body_mut() {
                println!("body chunk {:?}", chunk);
            }

            Response::new(Body::new("Hello World!"))
        })
        .expect("failed to start server");
}
