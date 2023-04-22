use astra::{Body, ConnectionInfo, Request, Response, ResponseBuilder, Server};

fn main() {
    Server::bind("localhost:3000")
        .serve(handle)
        .expect("serve failed");
}

fn handle(_req: Request, info: ConnectionInfo) -> Response {
    // Get the ip address of the client
    let peer_addr = match info.peer_addr() {
        Some(addr) => addr,
        None => {
            log::error!("Could not get the clients ip address");
            return ResponseBuilder::new()
                .status(500)
                .body(Body::empty())
                .unwrap();
        }
    };

    let peer_ip = peer_addr.ip();
    Response::new(Body::new(format!("Hello {}!", peer_ip)))
}
