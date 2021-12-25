# Astra

[![Crate](https://img.shields.io/crates/v/astra?style=for-the-badge)](https://crates.io/crates/astra)
[![Github](https://img.shields.io/badge/github-astra-success?style=for-the-badge)](https://github.com/ibraheemdev/astra)
[![Docs](https://img.shields.io/badge/docs.rs-0.1.1-4d76ae?style=for-the-badge)](https://docs.rs/astra)

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

Hyper is built on async I/O and requires it to run correctly, so Astra runs a small evented I/O loop under the hood. It dispatches incoming connections to a scalable worker pool, handing off async I/O sources to hyper. The difference is that instead of yielding to a userspace runtime like tokio, workers yield to the operating system scheduler. This means that services can use standard I/O primitives without worrying about blocking an event loop:

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

The point of this library isn't performance, it's to enable usage of a mature and stable HTTP server in hyper without a heavyweight runtime like tokio. If you want a synchronous HTTP server, you probably aren't looking for top-tier performance anyways. Async I/O *will* scale better as concurrency increases, both in terms of performance and resource usage. That being said, modern OS schedulers are much better than they are often given credit for...

It is fast! Astra does very well in benchmarks at 25k concurrent connections; I haven't measured further than that. In a basic hello world benchmark, Astra yields 20% more throughput than Hyper running on Tokio. In a heavier benchmark involving Postgres and some HTML templating, throughput is around the same. However, Astra in that case uses the [`postgres`](https://github.com/sfackler/rust-postgres) library, which uses Tokio under the hood. In the same test but with pure blocking I/O ([`libpq`](https://www.postgresql.org/docs/current/libpq.html)), Astra again performs around 20% better than Tokio. In both cases, Astra performs an order of magnitude better in terms of latency. However, this comes at the cost of higher memory and CPU usage.
