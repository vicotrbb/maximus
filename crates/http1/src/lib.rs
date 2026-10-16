//! HTTP/1.1 integration with completion I/O, bounded parsing and graceful drain.
#![forbid(unsafe_code)]
mod activity;
pub use activity::Activity;
use compio::io::{AsyncRead, AsyncWrite, compat::AsyncStream, util::Splittable};
use futures_util::{FutureExt, pin_mut, select_biased};
use http_body_util::BodyExt;
use hyper::{body::Incoming, service::service_fn};
use hyper_util::rt::TokioIo;
use maximus_core::{Body, Handler, IntoResponse, Limits, Request, Response, StatusCode};
use maximus_runtime::Shutdown;
use std::{
    cell::Cell,
    convert::Infallible,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio_util::compat::FuturesAsyncReadCompatExt;

/// Hyper requires Send + Sync timers. Runtime-checked wrapper enforces the worker affinity.
/// These timers are constructed, polled and dropped entirely within one local connection task.
pub struct Timer;
struct LocalSleep(send_wrapper::SendWrapper<Pin<Box<dyn Future<Output = ()>>>>);
impl Future for LocalSleep {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        Pin::new(&mut self.0).poll(cx)
    }
}
impl hyper::rt::Sleep for LocalSleep {}
impl hyper::rt::Timer for Timer {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn hyper::rt::Sleep>> {
        self.sleep_until(Instant::now() + duration)
    }
    fn sleep_until(&self, deadline: Instant) -> Pin<Box<dyn hyper::rt::Sleep>> {
        Box::pin(LocalSleep(send_wrapper::SendWrapper::new(Box::pin(
            compio::time::sleep_until(deadline),
        ))))
    }
}
#[derive(Clone, Copy)]
pub struct Executor;
impl<F: Future<Output = ()> + 'static> hyper::rt::Executor<F> for Executor {
    fn execute(&self, future: F) {
        compio::runtime::spawn(future).detach();
    }
}

/// Shared protocol policy, used after the wire parser and before application dispatch.
pub async fn dispatch<H: Handler>(
    request: hyper::Request<Incoming>,
    handler: Rc<H>,
    limits: Rc<Limits>,
) -> Response {
    let (parts, body) = request.into_parts();
    let request = Request::new(hyper::Request::from_parts(
        parts,
        Body::incoming(body, limits.body_bytes),
    ));
    dispatch_request(request, handler, limits).await
}

pub async fn dispatch_request<H: Handler>(
    request: Request,
    handler: Rc<H>,
    limits: Rc<Limits>,
) -> Response {
    let head = request.method() == hyper::Method::HEAD;
    suppress_body(dispatch_inner(request, handler, limits).await, head)
}

async fn dispatch_inner<H: Handler>(
    mut request: Request,
    handler: Rc<H>,
    limits: Rc<Limits>,
) -> Response {
    if let Err(error) = maximus_router::validate_path(request.uri().path()) {
        return error.into_response();
    }
    let hosts = request.headers().get_all("host").iter().count();
    if hosts > 1 || request.version() == hyper::Version::HTTP_11 && hosts != 1 {
        return (
            StatusCode::BAD_REQUEST,
            "exactly one Host header is required",
        )
            .into_response();
    }
    if request
        .uri()
        .authority()
        .is_some_and(|authority| !valid_authority(authority.as_str()))
        || request.headers().get("host").is_some_and(|value| {
            value
                .to_str()
                .ok()
                .is_none_or(|host| !valid_authority(host))
        })
    {
        return (StatusCode::BAD_REQUEST, "invalid Host authority").into_response();
    }
    match content_length(request.headers()) {
        Err(error) => return error.into_response(),
        Ok(Some(length)) if length > limits.body_bytes as u64 => {
            return StatusCode::PAYLOAD_TOO_LARGE.into_response();
        }
        _ => {}
    }
    if request.headers().len() > limits.headers
        || request.headers().iter().fold(0usize, |sum, (name, value)| {
            sum.saturating_add(name.as_str().len() + value.len() + 4)
        }) > limits.header_bytes
    {
        return StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE.into_response();
    }
    // RFC 9113 §8.2.3 / RFC 9114 §4.2.1: generic handlers receive one Cookie field.
    if matches!(
        request.version(),
        hyper::Version::HTTP_2 | hyper::Version::HTTP_3
    ) && request.headers().get_all("cookie").iter().count() > 1
    {
        let mut combined = Vec::new();
        for (index, value) in request.headers().get_all("cookie").iter().enumerate() {
            if index != 0 {
                combined.extend_from_slice(b"; ");
            }
            combined.extend_from_slice(value.as_bytes());
        }
        let mut value = maximus_core::HeaderValue::from_bytes(&combined)
            .expect("valid Cookie fields joined with a valid delimiter");
        value.set_sensitive(true);
        request.headers_mut().insert("cookie", value);
    }
    match compio::time::timeout(limits.request_timeout, handler.handle(request)).await {
        Ok(response) => response.map(|body| {
            if body.is_buffered() {
                return body;
            }
            let timeout = limits.request_timeout;
            Body::frames(futures_util::stream::unfold(
                Some(body),
                move |state| async move {
                    let mut body = state?;
                    match compio::time::timeout(timeout, body.frame()).await {
                        Ok(Some(frame)) => Some((frame, Some(body))),
                        Ok(None) => None,
                        Err(_) => Some((
                            Err(maximus_core::Error::new(
                                StatusCode::REQUEST_TIMEOUT,
                                "response stream stalled",
                            )),
                            None,
                        )),
                    }
                },
            ))
        }),
        Err(_) => StatusCode::REQUEST_TIMEOUT.into_response(),
    }
}

/// Strict shared Content-Length validation; wire engines may already have normalized duplicates.
pub fn content_length(
    headers: &maximus_core::HeaderMap,
) -> Result<Option<u64>, maximus_core::Error> {
    let mut values = headers.get_all("content-length").iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some()
        || value.is_empty()
        || !value.as_bytes().iter().all(u8::is_ascii_digit)
    {
        return Err(maximus_core::Error::bad_request("invalid Content-Length"));
    }
    value
        .to_str()
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Some)
        .ok_or_else(|| maximus_core::Error::bad_request("invalid Content-Length"))
}

fn valid_authority(value: &str) -> bool {
    if value.is_empty()
        || value.contains('@')
        || value.parse::<maximus_core::http::uri::Authority>().is_err()
    {
        return false;
    }
    let port = if let Some(literal) = value.strip_prefix('[') {
        let Some((address, suffix)) = literal.split_once(']') else {
            return false;
        };
        let future = address
            .strip_prefix(['v', 'V'])
            .and_then(|s| s.split_once('.'))
            .is_some_and(|(version, name)| {
                !version.is_empty()
                    && version.bytes().all(|b| b.is_ascii_hexdigit())
                    && !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:".contains(&b))
            });
        if address.parse::<std::net::Ipv6Addr>().is_err() && !future {
            return false;
        }
        if suffix.is_empty() {
            None
        } else {
            Some(match suffix.strip_prefix(':') {
                Some(port) => port,
                None => return false,
            })
        }
    } else {
        value.split_once(':').map(|(_, port)| port)
    };
    port.is_none_or(|port| port.bytes().all(|b| b.is_ascii_digit()))
}

/// Apply method/status body semantics consistently across all wire engines.
fn suppress_body(mut response: Response, head: bool) -> Response {
    use maximus_core::HttpBody;
    if head
        || response.status().is_informational()
        || response.status() == StatusCode::NO_CONTENT
        || response.status() == StatusCode::NOT_MODIFIED
    {
        if head
            && !response.headers().contains_key("content-length")
            && let Some(size) = response.body().size_hint().exact()
        {
            response.headers_mut().insert("content-length", size.into());
        }
        if response.status().is_informational() || response.status() == StatusCode::NO_CONTENT {
            response.headers_mut().remove("content-length");
        }
        *response.body_mut() = Body::empty();
    }
    response
}

pub async fn serve<S, H>(
    stream: S,
    handler: Rc<H>,
    limits: Rc<Limits>,
    shutdown: Shutdown,
) -> Result<(), hyper::Error>
where
    S: AsyncRead + AsyncWrite + Splittable + 'static,
    S::ReadHalf: AsyncRead + Unpin + 'static,
    S::WriteHalf: AsyncWrite + Unpin + 'static,
    H: Handler,
{
    let io = AsyncStream::with_limits(limits.io_buffer_bytes, limits.io_buffer_bytes, stream);
    serve_futures(io, handler, limits, shutdown).await
}

pub fn adapt<S: futures_util::AsyncRead + futures_util::AsyncWrite + 'static>(
    stream: S,
) -> impl hyper::rt::Read + hyper::rt::Write + Send + Unpin {
    LocalIo(send_wrapper::SendWrapper::new(TokioIo::new(
        Box::pin(stream).compat(),
    )))
}

pub async fn serve_futures<S, H>(
    stream: S,
    handler: Rc<H>,
    limits: Rc<Limits>,
    shutdown: Shutdown,
) -> Result<(), hyper::Error>
where
    S: futures_util::AsyncRead + futures_util::AsyncWrite + 'static,
    H: Handler,
{
    let (stream, activity) = Activity::track(stream);
    let io = adapt(stream);
    #[cfg(feature = "websocket")]
    let upgrades = UpgradeSlot::default();
    #[cfg(feature = "websocket")]
    let service_upgrades = upgrades.clone();
    let count = Rc::new(Cell::new(0usize));
    let service_limits = limits.clone();
    let service = service_fn(move |request| {
        #[cfg(feature = "websocket")]
        let request = {
            let mut request: hyper::Request<Incoming> = request;
            request.extensions_mut().insert(service_upgrades.clone());
            request
        };
        let handler = handler.clone();
        let limits = service_limits.clone();
        let count = count.clone();
        async move {
            count.set(count.get() + 1);
            let mut response = dispatch(request, handler, limits.clone()).await;
            if count.get() >= limits.requests_per_connection || response.status().is_client_error()
            {
                response.headers_mut().insert(
                    "connection",
                    hyper::header::HeaderValue::from_static("close"),
                );
            }
            Ok::<_, Infallible>(response)
        }
    });
    let mut builder = hyper::server::conn::http1::Builder::new();
    builder
        .timer(Timer)
        .header_read_timeout(limits.header_timeout)
        .max_headers(limits.headers)
        .max_buf_size(limits.header_bytes);
    let connection = builder.serve_connection(io, service);
    #[cfg(feature = "websocket")]
    let connection = connection.with_upgrades();
    pin_mut!(connection);
    let stop = shutdown.cancelled().fuse();
    pin_mut!(stop);
    let idle = activity.idle(limits.idle_timeout).fuse();
    pin_mut!(idle);
    let result = select_biased! {
        _ = idle => return Ok(()),
        _ = stop => {
            connection.as_mut().graceful_shutdown();
            match compio::time::timeout(limits.drain_timeout, connection).await { Ok(result) => result, Err(_) => Ok(()) }
        },
        result = connection.as_mut().fuse() => result,
    };
    #[cfg(feature = "websocket")]
    if let Some(task) = upgrades.take() {
        let task = task.fuse();
        let stop = shutdown.cancelled().fuse();
        pin_mut!(task, stop);
        select_biased! {
            _ = idle => {},
            _ = stop => { let _ = compio::time::timeout(limits.drain_timeout, task).await; },
            _ = task => {},
        }
    }
    result
}

// Hyper's upgrade container requires Send even on a local executor. SendWrapper checks
// every access and drop against the creating thread, preserving completion-I/O affinity.
struct LocalIo<T>(send_wrapper::SendWrapper<T>);
impl<T: hyper::rt::Read + Unpin> hyper::rt::Read for LocalIo<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.0).poll_read(cx, buf)
    }
}
impl<T: hyper::rt::Write + Unpin> hyper::rt::Write for LocalIo<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut *self.0).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.0).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut *self.0).poll_shutdown(cx)
    }
    fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut *self.0).poll_write_vectored(cx, bufs)
    }
}

#[cfg(feature = "websocket")]
mod upgrade;
#[cfg(feature = "websocket")]
pub use upgrade::UpgradeSlot;
