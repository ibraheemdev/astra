# Astra

[![Crate](https://img.shields.io/crates/v/astra?style=for-the-badge)](https://crates.io/crates/astra)
[![Github](https://img.shields.io/badge/github-astra-success?style=for-the-badge)](https://github.com/ibraheemdev/astra)
[![Docs](https://img.shields.io/badge/docs.rs-0.1.2-4d76ae?style=for-the-badge)](https://docs.rs/astra)

Astra is a synchronous HTTP server built on top of [`hyper`](https://github.com/hyperium/hyper).

```rust,no_run
use astra::{Body, Response, Server};

fn main() {
    Server::bind("localhost:3000")
        .serve(|_req| Response::new(Body::new("Hello World!")))
        .expect("serve failed");
}
```

## How Does It Work?

Hyper is built on async I/O and depends on it to run correctly, so Astra runs a small evented I/O loop under the hood. It dispatches incoming connections to a scalable worker pool, handing off async I/O sources to hyper. The difference is that instead of yielding to a userspace runtime like tokio, workers yield to the operating system scheduler. This means that services can use standard I/O primitives without worrying about blocking an event loop:

```rust,no_run
use astra::{Body, ResponseBuilder, Server};
use std::time::Duration;

fn main() {
    Server::bind("localhost:3000")
        .serve(|_req| {
            // Putting the worker thread to sleep will allow
            // other workers to run.
            std::thread::sleep(Duration::from_secs(1));

            // Regular blocking I/O is fine too!
            let body = std::fs::read_to_string("index.html").unwrap();

            ResponseBuilder::new()
                .header("Content-Type", "text/html")
                .body(Body::new(body))
                .unwrap()
        })
        .expect("serve failed");
}
```

## Features

Astra supports both HTTP/1 and HTTP/2 with most the configuration options hyper exposes. Features that depend on tokio however, such as [`http2_keep_alive_while_idle`](https://docs.rs/hyper/latest/hyper/client/struct.Builder.html#method.http2_keep_alive_while_idle), are not supported.

Astra is currently an HTTP *server* library only, the client API is unimplemented.

## But Is It Fast?

Many of the references you'll find about thread-per-request performance are very outdated, often referencing bottlenecks from a time where [C10k](http://www.kegel.com/c10k.html) was peak scale. Since then, thread creation has gotten significantly cheaper, and context switching overhead has been reduced drastically. Modern OS schedulers are much better than they are given credit for...

In a realistic benchmark involving PostgreSQL and some HTML templating, Astra comes very close to Tokio in terms of throughput at 20k concurrent connections. While not as performant as pure blocking I/O, Astra is fast! As always, you should measure your own use case, but you can expect Astra's performance to be comparable to that of Hyper running on Tokio.
