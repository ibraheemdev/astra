use astra::{Body, Request, Response, Server, ConnectionInfo};
use std::sync::atomic::{Ordering};

fn main() {

    Server::bind("localhost:3000")
        // connection_info is a second parameter like request
        .serve(move |req, connection_info| handle(req, connection_info))
        .expect("serve failed");
}

fn handle(_req: Request, connection_info: ConnectionInfo) -> Response {
    // Get the ip address of the client
    let peer_ip = connection_info.peer_addr.ip().to_string();
    log::debug!("The clients ip is  {peer_ip}");

    Response::new(Body::new(format!("Hello {}!", peer_ip)))
}
