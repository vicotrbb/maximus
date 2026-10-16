//! Shared request, streaming body, response and statically dispatched handler API.
#![forbid(unsafe_code)]
mod body;
mod date;
#[doc(hidden)]
pub use date::response_date;
mod request;
pub use body::Body;
pub use bytes::Bytes;
pub use http::{self, HeaderMap, HeaderValue, Method, StatusCode, Uri};
pub use request::{Request, decode};
use std::{borrow::Cow, future::Future};
pub type Response = http::Response<Body>;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Error {
    pub status: StatusCode,
    pub message: Cow<'static, str>,
}
impl Error {
    pub fn new(status: StatusCode, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
    pub fn bad_request(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for Error {}

pub trait IntoResponse {
    fn into_response(self) -> Response;
}
impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}
impl IntoResponse for Body {
    fn into_response(self) -> Response {
        Response::new(self)
    }
}
impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        let mut response = Response::new(self.into());
        response.headers_mut().insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        response
    }
}
impl IntoResponse for String {
    fn into_response(self) -> Response {
        let mut response = Response::new(self.into());
        response.headers_mut().insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        response
    }
}
impl IntoResponse for Bytes {
    fn into_response(self) -> Response {
        Response::new(self.into())
    }
}
impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        (self, Body::empty()).into_response()
    }
}
impl<R: IntoResponse> IntoResponse for (StatusCode, R) {
    fn into_response(self) -> Response {
        let mut response = self.1.into_response();
        *response.status_mut() = self.0;
        response
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.status, self.message.into_owned()).into_response()
    }
}
impl<T: IntoResponse, E: IntoResponse> IntoResponse for std::result::Result<T, E> {
    fn into_response(self) -> Response {
        match self {
            Ok(value) => value.into_response(),
            Err(error) => error.into_response(),
        }
    }
}

/// Futures stay on the worker and need not implement `Send`.
#[allow(async_fn_in_trait)]
pub trait Handler: 'static {
    async fn handle(&self, request: Request) -> Response;
}
impl<F, Fut, R> Handler for F
where
    F: Fn(Request) -> Fut + 'static,
    Fut: Future<Output = R>,
    R: IntoResponse,
{
    async fn handle(&self, request: Request) -> Response {
        self(request).await.into_response()
    }
}

/// Resource bounds apply independently on every worker. Application allocations are separate.
#[derive(Clone, Debug)]
pub struct Limits {
    pub connections: usize,
    pub streams: u32,
    pub stream_window_bytes: u32,
    pub connection_window_bytes: u32,
    pub body_bytes: usize,
    pub header_bytes: usize,
    pub headers: usize,
    pub io_buffer_bytes: usize,
    pub memory_bytes: usize,
    pub requests_per_connection: usize,
    pub header_timeout: std::time::Duration,
    pub request_timeout: std::time::Duration,
    pub idle_timeout: std::time::Duration,
    pub drain_timeout: std::time::Duration,
}
impl Default for Limits {
    fn default() -> Self {
        use std::time::Duration;
        Self {
            connections: 4096,
            streams: 32,
            stream_window_bytes: 65535,
            connection_window_bytes: 1024 * 1024,
            body_bytes: 1024 * 1024,
            header_bytes: 16 * 1024,
            headers: 64,
            io_buffer_bytes: 16 * 1024,
            memory_bytes: 256 * 1024 * 1024,
            requests_per_connection: 10000,
            header_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(60),
            drain_timeout: Duration::from_secs(5),
        }
    }
}
impl Limits {
    pub fn validate(&self) -> std::io::Result<()> {
        if self.streams == 0
            || self.stream_window_bytes > 0x7fff_ffff
            || self.connection_window_bytes > 0x7fff_ffff
            || self.connections == 0
            || self.headers == 0
            || self.header_bytes < 8192
            || self.io_buffer_bytes < 1024
            || self.requests_per_connection == 0
            || self.header_timeout.is_zero()
            || self.request_timeout.is_zero()
            || self.idle_timeout.is_zero()
            || self.connection_capacity() == 0
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid resource limits",
            ));
        }
        Ok(())
    }
    /// Admission estimate for plaintext HTTP/1. This is not an allocator or OS RSS limit.
    pub fn connection_capacity(&self) -> usize {
        self.protocol_capacity(false, false)
    }
    /// Reserves worst-case retained request bodies and headers for each allowed stream,
    /// flow-control windows, I/O buffers and TLS records. Application-owned allocations,
    /// allocator overhead and kernel socket buffers must be budgeted separately.
    pub fn protocol_capacity(&self, multiplexed: bool, tls: bool) -> usize {
        let request = self
            .body_bytes
            .saturating_add(self.header_bytes)
            .saturating_add(8192);
        let per_connection = self
            .io_buffer_bytes
            .saturating_mul(2)
            .saturating_add(request.saturating_mul(if multiplexed {
                self.streams as usize
            } else {
                1
            }))
            .saturating_add(if multiplexed {
                (self.connection_window_bytes as usize).saturating_mul(2)
            } else {
                0
            })
            .saturating_add(if tls { 64 * 1024 } else { 0 });
        self.connections
            .min(self.memory_bytes / per_connection.max(1))
    }
}

pub use http_body::Frame;

#[cfg(any(feature = "json", feature = "multipart"))]
mod features;
#[cfg(feature = "json")]
pub use features::Json;
#[cfg(feature = "sse")]
mod sse;
#[cfg(feature = "sse")]
pub use sse::{Event, Sse};
mod pipeline;
pub use pipeline::{State, guard, middleware, state};

pub use http_body::Body as HttpBody;

mod policy;
pub use policy::{content_length, dispatch_request, suppress_body, validate_path};

#[cfg(test)]
mod tests {
    use super::Limits;

    #[test]
    fn quic_admission_preserves_the_generic_tls_budget() {
        let limits = Limits {
            streams: 2,
            body_bytes: 0,
            header_bytes: 8192,
            io_buffer_bytes: 16384,
            connection_window_bytes: 65536,
            memory_bytes: 262144,
            ..Limits::default()
        };
        limits.validate().unwrap();
        assert_eq!(limits.protocol_capacity(true, true), 1);
    }

    #[test]
    fn admission_buffer_accounting_saturates_without_wrapping() {
        let mut limits = Limits {
            io_buffer_bytes: usize::MAX,
            memory_bytes: usize::MAX - 1,
            ..Limits::default()
        };
        for tls in [false, true] {
            assert_eq!(limits.protocol_capacity(false, tls), 0);
        }
        limits.memory_bytes = usize::MAX;
        for tls in [false, true] {
            assert_eq!(limits.protocol_capacity(false, tls), 1);
        }
    }
}
