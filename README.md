# Astra

[![Crate](https://img.shields.io/crates/v/astra?style=for-the-badge)](https://crates.io/crates/astra)
[![Github](https://img.shields.io/badge/github-astra-success?style=for-the-badge)](https://github.com/ibraheemdev/astra)
[![Docs](https://img.shields.io/badge/docs.rs-0.2.0-4d76ae?style=for-the-badge)](https://docs.rs/astra)

A blocking HTTP server built on top of [`hyper`](https://github.com/hyperium/hyper).

```rust,no_run
use astra::{Body, Response, Server};

fn main() {
    Server::bind("localhost:3000")
        .serve(|_req, _info| Response::new(Body::new("Hello World!")))
        .expect("serve failed");
}
```

## How Does It Work?

Hyper is built on async I/O and depends on it to run correctly. To avoid depending on a large crate like Tokio, Astra runs a small evented I/O loop on a background thread and dispatches connections to it's own worker pool. The difference is that instead of tasks yielding to a userspace runtime like Tokio, they yield to the operating system. This means that request handlers can use standard I/O primitives without worrying about blocking the runtime:

```rust,no_run
use astra::{Body, ResponseBuilder, Server};
use std::time::Duration;

fn main() {
    Server::bind("localhost:3000")
        .serve(|_req, _info| {
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

Astra supports both HTTP/1 and HTTP/2 with most the configuration options that Hyper exposes. Features that depend on Tokio however, such as [`http2_keep_alive_while_idle`](https://docs.rs/hyper/latest/hyper/client/struct.Builder.html#method.http2_keep_alive_while_idle), are not supported and blocked on [better hyper support](https://github.com/hyperium/hyper/issues/2846).

Astra is currently an HTTP *server* library only. The client API is unimplemented.

## Security

One of the issues with blocking I/O is that it is susceptible to attacks such as [Slowloris](https://www.cloudflare.com/learning/ddos/ddos-attack-tools/slowloris/). Because of this, it is important to run your server behind an async reverse proxy such as Nginx. You were likely going to doing this anyways for TLS support, but it is something to keep in mind.

## But Is It Fast?

Many of the references you'll find about thread-per-request performance are very outdated, often referencing bottlenecks from a time where [C10k](http://www.kegel.com/c10k.html) was peak scale. Since then, thread creation has gotten significantly cheaper, and context switching overhead has been reduced drastically. Modern OS schedulers are much better than they are given credit for, and it is now very feasible to serve upwards of tens of thousands of concurrent connections using blocking I/O.

In naive "Hello World" style HTTP benchmarks, Astra is likely to lag behind Tokio. This is partly because Astra has to pay the cost of both threading *and* async I/O to be compatible with Hyper. However, as more work is done per request, especially pure blocking I/O, the difference diminishes. As always, you should measure your own use case, but Astra's performance may surprise you.

That being said, one of Astra's main use cases is running a lightweight server with minimal dependencies, and avoiding the complexity that comes with async, so any potential performance tradeoffs might not be be relevant.
