use crate::net::{ConnectionInfo, Reactor};
use crate::{executor, Body, Request, Response};

use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::net::{TcpListener, ToSocketAddrs};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use hyper::rt::Executor;
use hyper::server::conn::Http;

/// An HTTP server.
///
/// ```no_run
/// use astra::{Body, Request, Response, Server};
///
/// Server::bind("localhost:3000")
///     .serve(|mut req: Request, _| {
///         println!("incoming {:?}", req.uri());
///         Response::new(Body::new("Hello World!"))
///     })
///     .expect("failed to start server");
/// ```
///
/// See the [crate-level documentation](crate#how-does-it-work) for details.
#[derive(Default)]
pub struct Server {
    listener: Option<TcpListener>,
    http1_keep_alive: Option<bool>,
    http1_half_close: Option<bool>,
    http1_max_buf_size: Option<usize>,
    http1_pipeline_flush: Option<bool>,
    http1_writev: Option<bool>,
    http1_title_case_headers: Option<bool>,
    http1_preserve_header_case: Option<bool>,
    http1_only: Option<bool>,
    http2_only: Option<bool>,
    http2_initial_stream_window_size: Option<u32>,
    http2_initial_connection_window_size: Option<u32>,
    http2_adaptive_window: Option<bool>,
    http2_max_frame_size: Option<u32>,
    http2_max_concurrent_streams: Option<u32>,
    http2_max_send_buf_size: Option<usize>,
    worker_keep_alive: Option<Duration>,
    max_workers: Option<usize>,
}

/// A service capable of responding to an HTTP request.
///
/// This trait is automatically implemented for functions
/// from a [`Request`] to a [`Response`], but implementing
/// it manually allows for stateful services:
///
/// ```no_run
/// use astra::{Request, Response, Server, Service, Body, ConnectionInfo};
/// use std::sync::Mutex;
///
/// struct MyService {
///     count: Mutex<usize>,
/// }
///
/// impl Service for MyService {
///     fn call(&self, request: Request, _: ConnectionInfo) -> Response {
///         let mut count = self.count.lock().unwrap();
///         *count += 1;
///         println!("request #{}", *count);
///         Response::new(Body::new("Hello world"))
///     }
/// }
///
///
/// Server::bind("localhost:3000")
///     .serve(MyService { count: Mutex::new(0) })
///     .expect("failed to start server");
/// ```
pub trait Service: Send + Sync + 'static {
    fn call(&self, request: Request, connection_info: ConnectionInfo) -> Response;
}

impl<F> Service for F
where
    F: Fn(Request, ConnectionInfo) -> Response + Send + Sync + 'static,
{
    fn call(&self, request: Request, connection_info: ConnectionInfo) -> Response {
        (self)(request, connection_info)

    }
}

impl Server {
    /// Binds a server to the provided address.
    ///
    /// ```no_run
    /// use astra::Server;
    /// use std::net::SocketAddr;
    ///
    /// let server = Server::bind("localhost:3000");
    /// let server = Server::bind(SocketAddr::from(([127, 0, 0, 1], 3000)));
    /// ```
    ///
    /// # Panics
    ///
    /// This method will panic if binding to the address fails.
    pub fn bind(addr: impl ToSocketAddrs) -> Server {
        let listener = std::net::TcpListener::bind(addr).expect("failed to bind listener");

        Server {
            listener: Some(listener),
            ..Default::default()
        }
    }

    /// Serve incoming connections with the provided service.
    ///
    /// ```no_run
    /// use astra::{Body, Request, Response, Server};
    ///
    /// Server::bind("localhost:3000")
    ///     .serve(|mut req: Request, _| {
    ///         println!("incoming {:?}", req.uri());
    ///         Response::new(Body::new("Hello World!"))
    ///     })
    ///     .expect("failed to start server");
    /// ```
    pub fn serve<S>(self, service: S) -> io::Result<()>
    where
        S: Service,
    {
        let executor = executor::Executor::new(self.max_workers, self.worker_keep_alive);
        let mut http = Http::new().with_executor(executor.clone());
        self.configure(&mut http);

        let service = Arc::new(service);
        let reactor = Reactor::new().expect("failed to create reactor");

        for conn in self.listener.unwrap().incoming() {
            let conn = conn.and_then(|stream| reactor.register(stream))?;
            let service = service.clone();
            let builder = http.clone();
            let connection_info = conn.connection_info.clone();
            executor.execute(async move {
                if let Err(err) = builder
                    .clone()
                    .serve_connection(conn, service::HyperService(service, connection_info))
                    .await
                {
                    log::error!("error serving connection: {}", err);
                }
            });
        }

        Ok(())
    }

    /// Sets the maximum number of threads in the worker pool.
    ///
    /// By default, the limit is 15 threads per CPU core.
    pub fn max_workers(mut self, val: usize) -> Self {
        self.max_workers = Some(val);
        self
    }

    /// Sets how long to keep alive an idle thread in the worker pool.
    ///
    /// By default, the timeout is set to 6 seconds.
    pub fn worker_keep_alive(mut self, val: Duration) -> Self {
        self.worker_keep_alive = Some(val);
        self
    }

    /// Sets whether to use keep-alive for HTTP/1 connections.
    ///
    /// Default is `true`.
    pub fn http1_keep_alive(mut self, val: bool) -> Self {
        self.http1_keep_alive = Some(val);
        self
    }

    /// Set whether HTTP/1 connections should support half-closures.
    ///
    /// Clients can chose to shutdown their write-side while waiting
    /// for the server to respond. Setting this to `true` will
    /// prevent closing the connection immediately if `read`
    /// detects an EOF in the middle of a request.
    ///
    /// Default is `false`.
    pub fn http1_half_close(mut self, val: bool) -> Self {
        self.http1_half_close = Some(val);
        self
    }

    /// Set the maximum buffer size.
    ///
    /// Default is ~ 400kb.
    pub fn http1_max_buf_size(mut self, val: usize) -> Self {
        self.http1_max_buf_size = Some(val);
        self
    }

    /// Sets whether to bunch up HTTP/1 writes until the read buffer is empty.
    ///
    /// This isn't really desirable in most cases, only really being useful in
    /// silly pipeline benchmarks.
    pub fn http1_pipeline_flush(mut self, val: bool) -> Self {
        self.http1_pipeline_flush = Some(val);
        self
    }

    /// Set whether HTTP/1 connections should try to use vectored writes,
    /// or always flatten into a single buffer.
    ///
    /// Note that setting this to false may mean more copies of body data,
    /// but may also improve performance when an IO transport doesn't
    /// support vectored writes well, such as most TLS implementations.
    ///
    /// Setting this to true will force hyper to use queued strategy
    /// which may eliminate unnecessary cloning on some TLS backends
    ///
    /// Default is `auto`. In this mode hyper will try to guess which
    /// mode to use
    pub fn http1_writev(mut self, enabled: bool) -> Self {
        self.http1_writev = Some(enabled);
        self
    }

    /// Set whether HTTP/1 connections will write header names as title case at
    /// the socket level.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is false.
    pub fn http1_title_case_headers(mut self, val: bool) -> Self {
        self.http1_title_case_headers = Some(val);
        self
    }

    /// Set whether to support preserving original header cases.
    ///
    /// Currently, this will record the original cases received, and store them
    /// in a private extension on the `Request`. It will also look for and use
    /// such an extension in any provided `Response`.
    ///
    /// Since the relevant extension is still private, there is no way to
    /// interact with the original cases. The only effect this can have now is
    /// to forward the cases in a proxy-like fashion.
    ///
    /// Note that this setting does not affect HTTP/2.
    ///
    /// Default is false.
    pub fn http1_preserve_header_case(mut self, val: bool) -> Self {
        self.http1_preserve_header_case = Some(val);
        self
    }

    /// Sets whether HTTP/1 is required.
    ///
    /// Default is `false`.
    pub fn http1_only(mut self, val: bool) -> Self {
        self.http1_only = Some(val);
        self
    }

    /// Sets whether HTTP/2 is required.
    ///
    /// Default is `false`.
    pub fn http2_only(mut self, val: bool) -> Self {
        self.http2_only = Some(val);
        self
    }

    /// Sets the [`SETTINGS_INITIAL_WINDOW_SIZE`][spec] option for HTTP2
    /// stream-level flow control.
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    ///
    /// [spec]: https://http2.github.io/http2-spec/#SETTINGS_INITIAL_WINDOW_SIZE
    pub fn http2_initial_stream_window_size(mut self, sz: impl Into<Option<u32>>) -> Self {
        self.http2_initial_stream_window_size = sz.into();
        self
    }

    /// Sets the max connection-level flow control for HTTP2
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    pub fn http2_initial_connection_window_size(mut self, sz: impl Into<Option<u32>>) -> Self {
        self.http2_initial_connection_window_size = sz.into();
        self
    }

    /// Sets whether to use an adaptive flow control.
    ///
    /// Enabling this will override the limits set in
    /// `http2_initial_stream_window_size` and
    /// `http2_initial_connection_window_size`.
    pub fn http2_adaptive_window(mut self, enabled: bool) -> Self {
        self.http2_adaptive_window = Some(enabled);
        self
    }

    /// Sets the maximum frame size to use for HTTP2.
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, hyper will use a default.
    pub fn http2_max_frame_size(mut self, sz: impl Into<Option<u32>>) -> Self {
        self.http2_max_frame_size = sz.into();
        self
    }

    /// Sets the [`SETTINGS_MAX_CONCURRENT_STREAMS`][spec] option for HTTP2
    /// connections.
    ///
    /// Default is no limit (`std::u32::MAX`). Passing `None` will do nothing.
    ///
    /// [spec]: https://http2.github.io/http2-spec/#SETTINGS_MAX_CONCURRENT_STREAMS
    pub fn http2_max_concurrent_streams(mut self, max: impl Into<Option<u32>>) -> Self {
        self.http2_max_concurrent_streams = max.into();
        self
    }

    /// Set the maximum write buffer size for each HTTP/2 stream.
    ///
    /// Default is currently ~400KB, but may change.
    ///
    /// # Panics
    ///
    /// The value must be no larger than `u32::MAX`.
    pub fn http2_max_send_buf_size(mut self, max: usize) -> Self {
        self.http2_max_send_buf_size = Some(max);
        self
    }

    fn configure<T>(&self, http: &mut Http<T>) {
        macro_rules! configure {
            ($self:ident, $other:expr, [$($option:ident),* $(,)?], [$($other_option:ident => $this_option:ident),* $(,)?]) => {{
                $(
                    if let Some(val) = $self.$option {
                        $other.$option(val);
                    }
                )*
                $(
                    if let Some(val) = $self.$this_option {
                        $other.$other_option(val);
                    }
                )*
            }};
        }

        configure!(
            self,
            http,
            [
                http1_keep_alive,
                http1_half_close,
                http1_writev,
                http1_title_case_headers,
                http1_preserve_header_case,
                http1_only,
                http2_only,
                http2_initial_stream_window_size,
                http2_initial_connection_window_size,
                http2_adaptive_window,
                http2_max_frame_size,
                http2_max_concurrent_streams,
                http2_max_send_buf_size
            ],
            [
                max_buf_size => http1_max_buf_size,
                pipeline_flush => http1_pipeline_flush,
            ]
        );
    }
}

mod service {
    use crate::net::ConnectionInfo;
    use super::*;

    type HyperRequest = hyper::Request<hyper::Body>;

    pub struct HyperService<S>(pub Arc<S>, pub ConnectionInfo);

    impl<S> hyper::service::Service<HyperRequest> for HyperService<S>
    where
        S: Service,
    {
        type Response = Response;
        type Error = Infallible;
        type Future = Lazy<S>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: HyperRequest) -> Self::Future {
            Lazy(self.0.clone(), Some(req), self.1.clone())
        }
    }

    pub struct Lazy<S>(Arc<S>, Option<HyperRequest>, ConnectionInfo);

    impl<S> Future for Lazy<S>
    where
        S: Service,
    {
        type Output = Result<Response, Infallible>;

        fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            let (parts, body) = self.1.take().unwrap().into_parts();
            let response = self.0.call(Request::from_parts(parts, Body(body)), self.2.clone());
            Poll::Ready(Ok(response))
        }
    }
}
