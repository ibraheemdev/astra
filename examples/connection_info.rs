use astra::{Body, ConnectionInfo, Request, Response, Server};

fn main() {
    Server::bind("localhost:3000")
        .serve(handle)
        .expect("serve failed");
}

fn handle(_req: Request, info: ConnectionInfo) -> Response {
    // Get the ip address of the client
    let peer_addr = match info.peer_addr {
        Some(peer_add) => peer_add,
        None => {
            log::error!("Could not get the clients ip address");
            return Response::new(Body::new("Internal Server Error"));
        }
    };
    let peer_ip = peer_addr.ip().to_string();
    log::debug!("The clients ip is  {peer_ip}");

    Response::new(Body::new(format!("Hello {}!", peer_ip)))
}
