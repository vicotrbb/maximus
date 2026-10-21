//! Native HTTP/1 framing over completion I/O, with bounded parsing and graceful drain.
#![forbid(unsafe_code)]
mod activity;
#[path = "io.rs"]
mod transport_io;
pub use activity::Activity;
use compio::io::{AsyncRead, AsyncWrite, util::Splittable};
use futures_util::{FutureExt, lock::Mutex, pin_mut, select_biased};
use http_body_util::BodyExt;
use maximus_core::{
    Body, Bytes, Error, Frame, Handler, HttpBody, IntoResponse, Limits, Request, Response,
    StatusCode, http,
};
pub use maximus_core::{content_length, dispatch_request};
use maximus_runtime::Shutdown;
use std::{cell::Cell, io, rc::Rc};
use transport_io::{Futures, Native, ReadIo, WriteIo};
#[cfg(feature = "websocket")]
mod upgrade;
#[cfg(feature = "websocket")]
pub use upgrade::{UpgradeSlot, Upgraded};

pub async fn serve<S, H>(
    stream: S,
    handler: Rc<H>,
    limits: Rc<Limits>,
    shutdown: Shutdown,
) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Splittable + 'static,
    S::ReadHalf: AsyncRead + Unpin + 'static,
    S::WriteHalf: AsyncWrite + Unpin + 'static,
    H: Handler,
{
    let (read, write) = stream.split();
    serve_halves(Native(read), Native(write), handler, limits, shutdown).await
}
/// Explicit TLS/futures transport path. The lock is held for a poll, never across a pending read.
pub async fn serve_futures<S, H>(
    stream: S,
    handler: Rc<H>,
    limits: Rc<Limits>,
    shutdown: Shutdown,
) -> io::Result<()>
where
    S: futures_util::AsyncRead + futures_util::AsyncWrite + 'static,
    H: Handler,
{
    let (read, write) = futures_util::AsyncReadExt::split(Box::pin(stream));
    serve_halves(Futures(read), Futures(write), handler, limits, shutdown).await
}
async fn serve_halves<R: ReadIo, W: WriteIo, H: Handler>(
    read: R,
    write: W,
    handler: Rc<H>,
    limits: Rc<Limits>,
    shutdown: Shutdown,
) -> io::Result<()> {
    limits.validate()?;
    let activity = Activity::new();
    let connection = connection(
        read,
        write,
        handler,
        limits.clone(),
        activity.clone(),
        shutdown.clone(),
    )
    .fuse();
    let idle = activity.idle(limits.idle_timeout).fuse();
    let stop = shutdown.cancelled().fuse();
    pin_mut!(connection, idle, stop);
    select_biased! {
        result = connection => result,
        _ = idle => Ok(()),
        _ = stop => match compio::time::timeout(limits.drain_timeout, connection).await { Ok(result) => result, Err(_) => Ok(()) },
    }
}

struct Reader<R> {
    io: R,
    bytes: Vec<u8>,
    scratch: Vec<u8>,
    capacity: usize,
    activity: Activity,
}
impl<R: ReadIo> Reader<R> {
    async fn fill(&mut self) -> io::Result<bool> {
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.resize(self.capacity, 0);
        let (result, scratch) = self.io.read(scratch).await;
        self.scratch = scratch;
        let n = result?;
        if n > 0 {
            self.activity.touch();
            self.bytes.extend_from_slice(&self.scratch[..n]);
        }
        Ok(n > 0)
    }
    async fn line(&mut self, max: usize) -> Result<Vec<u8>, Error> {
        loop {
            if let Some(end) = self.bytes.windows(2).position(|s| s == b"\r\n") {
                if end + 2 > max {
                    return Err(Error::bad_request("line limit exceeded"));
                }
                let mut line: Vec<_> = self.bytes.drain(..end + 2).collect();
                line.truncate(end);
                return Ok(line);
            }
            if self.bytes.len() >= max {
                return Err(Error::bad_request("line limit exceeded"));
            }
            if !self
                .fill()
                .await
                .map_err(|_| Error::bad_request("body read failed"))?
            {
                return Err(Error::bad_request("truncated body"));
            }
        }
    }
    async fn head(&mut self, max: usize) -> Result<Option<Vec<u8>>, Error> {
        let mut scanned = 0;
        loop {
            if let Some(end) = self.bytes[scanned..]
                .windows(4)
                .position(|s| s == b"\r\n\r\n")
            {
                let end = scanned + end + 4;
                if end > max {
                    return Err(Error::new(
                        StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
                        "header limit exceeded",
                    ));
                }
                return Ok(Some(self.bytes.drain(..end).collect()));
            }
            if self.bytes.len() >= max {
                return Err(Error::new(
                    StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
                    "header limit exceeded",
                ));
            }
            scanned = self.bytes.len().saturating_sub(3);
            if !self
                .fill()
                .await
                .map_err(|_| Error::bad_request("request read failed"))?
            {
                return if self.bytes.is_empty() {
                    Ok(None)
                } else {
                    Err(Error::bad_request("truncated headers"))
                };
            }
        }
    }
    async fn data(&mut self, remaining: usize) -> Result<Bytes, Error> {
        if self.bytes.is_empty()
            && !self
                .fill()
                .await
                .map_err(|_| Error::bad_request("body read failed"))?
        {
            return Err(Error::bad_request("truncated body"));
        }
        let count = remaining.min(self.capacity).min(self.bytes.len());
        Ok(self.bytes.drain(..count).collect::<Vec<_>>().into())
    }
}
#[derive(Clone, Copy)]
enum Framing {
    Empty,
    Fixed(u64),
    Chunked,
}
struct Head {
    request: http::Request<Body>,
    framing: Framing,
    close: bool,
    expect: bool,
}
fn token(headers: &http::HeaderMap, name: &str, needle: &str) -> bool {
    headers.get_all(name).iter().any(|v| {
        v.to_str().is_ok_and(|v| {
            v.split(',')
                .any(|part| part.trim().eq_ignore_ascii_case(needle))
        })
    })
}
fn field(line: &[u8], headers: &mut http::HeaderMap) -> Result<(), Error> {
    let colon = line
        .iter()
        .position(|b| *b == b':')
        .ok_or_else(|| Error::bad_request("invalid header"))?;
    let name = http::header::HeaderName::from_bytes(&line[..colon])
        .map_err(|_| Error::bad_request("invalid header name"))?;
    let value = line[colon + 1..].trim_ascii();
    // Only SP/HTAB are OWS; reject all other controls rather than trimming them away.
    if line[colon + 1..]
        .iter()
        .any(|b| (*b < 32 && *b != b'\t') || *b == 127)
    {
        return Err(Error::bad_request("invalid header value"));
    }
    let value = http::HeaderValue::from_bytes(value)
        .map_err(|_| Error::bad_request("invalid header value"))?;
    headers.append(name, value);
    Ok(())
}
fn parse_head(bytes: &[u8], limits: &Limits) -> Result<Head, Error> {
    let mut lines = bytes[..bytes.len() - 2].split(|b| *b == b'\n');
    let line = lines
        .next()
        .unwrap()
        .strip_suffix(b"\r")
        .ok_or_else(|| Error::bad_request("invalid request line"))?;
    let mut words = line.split(|b| *b == b' ');
    let method = http::Method::from_bytes(words.next().unwrap_or_default())
        .map_err(|_| Error::bad_request("invalid method"))?;
    let target = words
        .next()
        .ok_or_else(|| Error::bad_request("missing target"))?;
    if target.is_empty() || target.contains(&b'#') || target.iter().any(|b| *b <= 32 || *b == 127) {
        return Err(Error::bad_request("invalid target"));
    }
    let uri = http::Uri::try_from(target).map_err(|_| Error::bad_request("invalid target"))?;
    let version = match words.next() {
        Some(b"HTTP/1.1") => http::Version::HTTP_11,
        Some(b"HTTP/1.0") => http::Version::HTTP_10,
        _ => {
            return Err(Error::new(
                StatusCode::HTTP_VERSION_NOT_SUPPORTED,
                "unsupported HTTP version",
            ));
        }
    };
    if words.next().is_some() {
        return Err(Error::bad_request("invalid request line"));
    }
    let target_valid = if method == http::Method::CONNECT {
        uri.authority().is_some_and(|a| a.port_u16().is_some())
            && uri.scheme().is_none()
            && !target.contains(&b'/')
    } else {
        target.starts_with(b"/")
            || uri.scheme().is_some() && uri.authority().is_some()
            || method == http::Method::OPTIONS && target == b"*"
    };
    if !target_valid {
        return Err(Error::bad_request("invalid request target form"));
    }
    let mut request = http::Request::new(Body::empty());
    *request.method_mut() = method;
    *request.uri_mut() = uri;
    *request.version_mut() = version;
    let mut lines = lines.peekable();
    while let Some(line) = lines.next() {
        if line.is_empty() && lines.peek().is_none() {
            break;
        }
        let line = line
            .strip_suffix(b"\r")
            .ok_or_else(|| Error::bad_request("invalid header newline"))?;
        field(line, request.headers_mut())?;
        if request.headers().len() > limits.headers {
            return Err(Error::new(
                StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
                "too many headers",
            ));
        }
    }
    let length = content_length(request.headers())?;
    let encodings: Vec<_> = request
        .headers()
        .get_all("transfer-encoding")
        .iter()
        .collect();
    if !encodings.is_empty()
        && (length.is_some()
            || version == http::Version::HTTP_10
            || encodings.len() != 1
            || !encodings[0].as_bytes().eq_ignore_ascii_case(b"chunked"))
    {
        return Err(Error::bad_request(
            "ambiguous or unsupported transfer framing",
        ));
    }
    if length.is_some_and(|n| n > limits.body_bytes as u64) {
        return Err(Error::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "body limit exceeded",
        ));
    }
    let framing = if !encodings.is_empty() {
        Framing::Chunked
    } else {
        length
            .filter(|n| *n > 0)
            .map_or(Framing::Empty, Framing::Fixed)
    };
    let expects: Vec<_> = request.headers().get_all("expect").iter().collect();
    if !expects.is_empty()
        && (expects.len() != 1
            || !expects[0].as_bytes().eq_ignore_ascii_case(b"100-continue")
            || version != http::Version::HTTP_11)
    {
        return Err(Error::new(
            StatusCode::EXPECTATION_FAILED,
            "unsupported expectation",
        ));
    }
    let expect = !expects.is_empty() && !matches!(framing, Framing::Empty);
    let close = token(request.headers(), "connection", "close")
        || version == http::Version::HTTP_10
            && !token(request.headers(), "connection", "keep-alive");
    Ok(Head {
        request,
        framing,
        close,
        expect,
    })
}

struct Incoming<R> {
    reader: Rc<Mutex<Reader<R>>>,
    framing: Framing,
    chunk: usize,
    total: usize,
    done: Rc<Cell<bool>>,
    limits: Rc<Limits>,
}
impl<R: ReadIo> Incoming<R> {
    async fn next(&mut self) -> Result<Option<Frame<Bytes>>, Error> {
        let mut reader = self.reader.lock().await;
        match self.framing {
            Framing::Empty => {
                self.done.set(true);
                Ok(None)
            }
            Framing::Fixed(remaining) => {
                let bytes = reader
                    .data(remaining.min(usize::MAX as u64) as usize)
                    .await?;
                let left = remaining - bytes.len() as u64;
                self.framing = if left == 0 {
                    self.done.set(true);
                    Framing::Empty
                } else {
                    Framing::Fixed(left)
                };
                Ok(Some(Frame::data(bytes)))
            }
            Framing::Chunked => {
                if self.chunk == 0 {
                    let line = reader.line(self.limits.header_bytes).await?;
                    if line.iter().any(|b| (*b < 32 && *b != 9) || *b == 127) {
                        return Err(Error::bad_request("invalid chunk extension"));
                    }
                    let (digits, extension) = match line.iter().position(|b| *b == b';') {
                        Some(i) => (&line[..i], Some(&line[i + 1..])),
                        None => (line.as_slice(), None),
                    };
                    let digits = digits.trim_ascii_end();
                    if digits.is_empty()
                        || !digits.iter().all(u8::is_ascii_hexdigit)
                        || extension.is_some_and(|s| !valid_extensions(s))
                    {
                        return Err(Error::bad_request("invalid chunk size or extension"));
                    }
                    self.chunk = usize::from_str_radix(std::str::from_utf8(digits).unwrap(), 16)
                        .map_err(|_| Error::bad_request("chunk size overflow"))?;
                    self.total = self
                        .total
                        .checked_add(self.chunk)
                        .filter(|n| *n <= self.limits.body_bytes)
                        .ok_or_else(|| {
                            Error::new(StatusCode::PAYLOAD_TOO_LARGE, "body limit exceeded")
                        })?;
                    if self.chunk == 0 {
                        let mut trailers = http::HeaderMap::new();
                        let mut size = 0usize;
                        loop {
                            let line = reader
                                .line(self.limits.header_bytes.saturating_sub(size))
                                .await?;
                            size += line.len() + 2;
                            if line.is_empty() {
                                break;
                            }
                            field(&line, &mut trailers)?;
                            if trailers.len() > self.limits.headers {
                                return Err(Error::bad_request("too many trailers"));
                            }
                        }
                        if trailers.keys().any(|name| {
                            matches!(
                                name.as_str(),
                                "content-length"
                                    | "transfer-encoding"
                                    | "host"
                                    | "connection"
                                    | "trailer"
                                    | "upgrade"
                                    | "expect"
                            )
                        }) {
                            return Err(Error::bad_request("forbidden trailer field"));
                        }
                        self.framing = Framing::Empty;
                        self.done.set(true);
                        return Ok(if trailers.is_empty() {
                            None
                        } else {
                            Some(Frame::trailers(trailers))
                        });
                    }
                }
                let bytes = reader.data(self.chunk).await?;
                self.chunk -= bytes.len();
                if self.chunk == 0 && !reader.line(2).await?.is_empty() {
                    return Err(Error::bad_request("invalid chunk terminator"));
                }
                Ok(Some(Frame::data(bytes)))
            }
        }
    }
}
fn valid_extensions(mut s: &[u8]) -> bool {
    if s.iter().any(|b| (*b < 32 && *b != 9) || *b == 127) {
        return false;
    }
    fn tchar(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
    }
    loop {
        s = s.trim_ascii_start();
        let n = s.iter().take_while(|b| tchar(**b)).count();
        if n == 0 {
            return false;
        }
        s = s[n..].trim_ascii_start();
        if s.first() == Some(&b'=') {
            s = s[1..].trim_ascii_start();
            if s.first() == Some(&b'"') {
                s = &s[1..];
                let mut closed = false;
                while let Some((&b, rest)) = s.split_first() {
                    s = rest;
                    if b == b'"' {
                        closed = true;
                        break;
                    }
                    if b == b'\\' {
                        if s.is_empty() || (s[0] < 32 && s[0] != 9) || s[0] == 127 {
                            return false;
                        }
                        s = &s[1..];
                    } else if (b < 32 && b != 9) || b == 127 {
                        return false;
                    }
                }
                if !closed {
                    return false;
                }
            } else {
                let n = s.iter().take_while(|b| tchar(**b)).count();
                if n == 0 {
                    return false;
                }
                s = &s[n..];
            }
        }
        s = s.trim_ascii_start();
        if s.is_empty() {
            return true;
        }
        if s[0] != b';' {
            return false;
        }
        s = &s[1..];
    }
}

async fn connection<R: ReadIo, W: WriteIo, H: Handler>(
    read: R,
    mut write: W,
    handler: Rc<H>,
    limits: Rc<Limits>,
    activity: Activity,
    shutdown: Shutdown,
) -> io::Result<()> {
    let reader = Rc::new(Mutex::new(Reader {
        io: read,
        bytes: Vec::new(),
        scratch: Vec::new(),
        capacity: limits.io_buffer_bytes,
        activity: activity.clone(),
    }));
    let mut output = Vec::with_capacity(limits.io_buffer_bytes + 32);
    for count in 1..=limits.requests_per_connection {
        if shutdown.is_cancelled() {
            return write.close().await;
        }
        let parsed = compio::time::timeout(limits.header_timeout, async {
            let mut reader = reader.lock().await;
            reader
                .head(limits.header_bytes)
                .await?
                .map(|bytes| parse_head(&bytes, &limits))
                .transpose()
        })
        .await;
        let mut head = match parsed {
            Ok(Ok(Some(head))) => head,
            Ok(Ok(None)) => return write.close().await,
            other => {
                let error = match other {
                    Ok(Err(error)) => error,
                    _ => Error::new(StatusCode::REQUEST_TIMEOUT, "header deadline exceeded"),
                };
                write_response(
                    &mut write,
                    &mut output,
                    error.into_response(),
                    false,
                    http::Version::HTTP_11,
                    true,
                    &limits,
                    &activity,
                )
                .await?;
                return write.close().await;
            }
        };
        let version = head.request.version();
        let is_head = head.request.method() == http::Method::HEAD;
        let connect = head.request.method() == http::Method::CONNECT;
        let done = Rc::new(Cell::new(matches!(head.framing, Framing::Empty)));
        if !done.get() {
            let incoming = Incoming {
                reader: reader.clone(),
                framing: head.framing,
                chunk: 0,
                total: 0,
                done: done.clone(),
                limits: limits.clone(),
            };
            *head.request.body_mut() = Body::incoming(
                futures_util::stream::unfold(Some(incoming), |state| async move {
                    let mut incoming = state?;
                    match incoming.next().await {
                        Ok(Some(frame)) => Some((Ok(frame), Some(incoming))),
                        Ok(None) => None,
                        Err(error) => Some((Err(error), None)),
                    }
                }),
                limits.body_bytes,
            );
        }
        #[cfg(feature = "websocket")]
        let upgrades = {
            let slot = UpgradeSlot::default();
            head.request.extensions_mut().insert(slot.clone());
            slot
        };
        if head.expect {
            output.extend_from_slice(b"HTTP/1.1 100 Continue\r\n\r\n");
            send_output(&mut write, &mut output, &activity).await?;
            write.flush().await?;
            activity.touch();
        }
        let mut response =
            dispatch_request(Request::new(head.request), handler.clone(), limits.clone()).await;
        #[cfg(feature = "websocket")]
        let callback = upgrades.take();
        #[cfg(feature = "websocket")]
        let can_upgrade = callback.is_some();
        #[cfg(not(feature = "websocket"))]
        let can_upgrade = false;
        if connect && response.status().is_success() && !can_upgrade {
            response = (
                StatusCode::NOT_IMPLEMENTED,
                "CONNECT requires a registered tunnel upgrade",
            )
                .into_response();
        }
        if response.status().is_informational()
            && !(response.status() == StatusCode::SWITCHING_PROTOCOLS && can_upgrade)
        {
            response = (
                StatusCode::INTERNAL_SERVER_ERROR,
                "handler returned an unsupported informational final response",
            )
                .into_response();
        }
        let tunnel = connect && response.status().is_success();
        if tunnel {
            response.headers_mut().remove("content-length");
            response.headers_mut().remove("transfer-encoding");
            *response.body_mut() = Body::empty();
        }
        let switching = response.status() == StatusCode::SWITCHING_PROTOCOLS || tunnel;
        let close = (version == http::Version::HTTP_10
            && !is_head
            && response.body().size_hint().exact().is_none()
            && !response.headers().contains_key("content-length"))
            || head.close
            // RFC 9931 §8: optimistic tunnel bytes must not become HTTP requests.
            || connect && !switching
            || count == limits.requests_per_connection
            || response.status().is_client_error()
            || token(response.headers(), "connection", "close")
            || shutdown.is_cancelled();
        write_response(
            &mut write,
            &mut output,
            response,
            is_head || tunnel,
            version,
            close,
            &limits,
            &activity,
        )
        .await?;
        #[cfg(feature = "websocket")]
        if switching {
            if let Some(callback) = callback {
                if !done.get() {
                    return Err(io::Error::other("upgrade with unread request body"));
                }
                let reader = Rc::try_unwrap(reader)
                    .map_err(|_| io::Error::other("request reader retained during upgrade"))?
                    .into_inner();
                callback(Upgraded {
                    read: reader.io.into_futures(limits.io_buffer_bytes),
                    write: write.into_futures(limits.io_buffer_bytes),
                    pending: reader.bytes,
                    offset: 0,
                    activity,
                })
                .await;
            }
            return Ok(());
        }
        if close || switching || !done.get() {
            return write.close().await;
        }
    }
    write.close().await
}
// The connection-owned output buffer stays explicit alongside the existing wire policy.
#[allow(clippy::too_many_arguments)]
async fn write_response<W: WriteIo>(
    write: &mut W,
    output: &mut Vec<u8>,
    mut response: Response,
    head: bool,
    version: http::Version,
    mut close: bool,
    limits: &Limits,
    activity: &Activity,
) -> io::Result<()> {
    let bodyless = head
        || response.status().is_informational()
        || matches!(response.status().as_u16(), 204 | 205 | 304);
    let mut length = content_length(response.headers()).map_err(io::Error::other)?;
    if response.headers().contains_key("transfer-encoding")
        && (length.is_some()
            || response
                .headers()
                .get_all("transfer-encoding")
                .iter()
                .count()
                != 1
            || !response.headers()["transfer-encoding"]
                .as_bytes()
                .eq_ignore_ascii_case(b"chunked"))
    {
        return Err(io::Error::other("invalid response framing"));
    }
    response.headers_mut().remove("transfer-encoding");
    if !bodyless && length.is_none() {
        length = response.body().size_hint().exact();
        if let Some(n) = length {
            response.headers_mut().insert("content-length", n.into());
        }
    }
    let chunked = !bodyless && length.is_none() && version == http::Version::HTTP_11;
    if chunked {
        response.headers_mut().insert(
            "transfer-encoding",
            http::HeaderValue::from_static("chunked"),
        );
    }
    if !bodyless && length.is_none() && !chunked {
        close = true;
    }
    if close {
        response
            .headers_mut()
            .insert("connection", http::HeaderValue::from_static("close"));
    } else if version == http::Version::HTTP_10 {
        response
            .headers_mut()
            .insert("connection", http::HeaderValue::from_static("keep-alive"));
    }
    if !response.headers().contains_key("date") {
        response
            .headers_mut()
            .insert("date", maximus_core::response_date());
    }
    let protocol = if version == http::Version::HTTP_10 {
        "HTTP/1.0"
    } else {
        "HTTP/1.1"
    };
    use std::io::Write;
    write!(
        output,
        "{protocol} {} {}\r\n",
        response.status().as_u16(),
        response.status().canonical_reason().unwrap_or("")
    )?;
    for (name, value) in response.headers() {
        for part in [name.as_str().as_bytes(), b": ", value.as_bytes(), b"\r\n"] {
            append_output(write, output, part, limits.io_buffer_bytes, activity).await?;
        }
    }
    append_output(write, output, b"\r\n", limits.io_buffer_bytes, activity).await?;
    // A buffered body cannot suspend, so coalesce it with the headers. Streaming
    // bodies must still flush headers before polling potentially pending input.
    if !bodyless
        && response.body().is_buffered()
        && response.body().size_hint().exact() == length
        && let Some(bytes) = response.body_mut().take_bytes()
    {
        append_output(write, output, &bytes, limits.io_buffer_bytes, activity).await?;
        send_output(write, output, activity).await?;
        return write.flush().await;
    }
    send_output(write, output, activity).await?;
    write.flush().await?;
    activity.touch();
    let mut sent = 0u64;
    let mut trailers = None;
    if !bodyless {
        while let Some(frame) = response.body_mut().frame().await {
            let frame = frame.map_err(io::Error::other)?;
            match frame.into_data() {
                Ok(data) => {
                    if trailers.is_some() {
                        return Err(io::Error::other("data after trailers"));
                    }
                    sent = sent
                        .checked_add(data.len() as u64)
                        .ok_or_else(|| io::Error::other("response length overflow"))?;
                    if length.is_some_and(|n| sent > n) {
                        return Err(io::Error::other("response exceeds Content-Length"));
                    }
                    for bytes in data.chunks(limits.io_buffer_bytes) {
                        if chunked {
                            write!(output, "{:x}\r\n", bytes.len())?;
                        }
                        output.extend_from_slice(bytes);
                        if chunked {
                            output.extend_from_slice(b"\r\n");
                        }
                        send_output(write, output, activity).await?;
                        write.flush().await?;
                        activity.touch();
                    }
                }
                Err(frame) => {
                    if let Ok(fields) = frame.into_trailers() {
                        if trailers.is_some() {
                            return Err(io::Error::other("duplicate trailers"));
                        }
                        if !chunked {
                            return Err(io::Error::other(
                                "trailers require chunked response framing",
                            ));
                        }
                        trailers = Some(fields);
                    }
                }
            }
        }
        if length.is_some_and(|n| n != sent) {
            return Err(io::Error::other("response shorter than Content-Length"));
        }
        if chunked && trailers.is_none() {
            output.extend_from_slice(b"0\r\n\r\n");
            send_output(write, output, activity).await?;
        } else if chunked {
            let mut final_chunk = b"0\r\n".to_vec();
            if let Some(trailers) = trailers {
                for (name, value) in &trailers {
                    if matches!(
                        name.as_str(),
                        "content-length"
                            | "transfer-encoding"
                            | "host"
                            | "connection"
                            | "trailer"
                            | "upgrade"
                    ) {
                        return Err(io::Error::other("forbidden response trailer"));
                    }
                    if final_chunk
                        .len()
                        .saturating_add(name.as_str().len())
                        .saturating_add(value.len())
                        .saturating_add(4)
                        > limits.header_bytes
                    {
                        return Err(io::Error::other("response trailers too large"));
                    }
                    final_chunk.extend_from_slice(name.as_str().as_bytes());
                    final_chunk.extend_from_slice(b": ");
                    final_chunk.extend_from_slice(value.as_bytes());
                    final_chunk.extend_from_slice(b"\r\n");
                    if final_chunk.len() > limits.header_bytes {
                        return Err(io::Error::other("response trailers too large"));
                    }
                }
            }
            final_chunk.extend_from_slice(b"\r\n");
            write.write(final_chunk).await.0?;
            activity.touch();
        }
    }
    write.flush().await
}

async fn append_output<W: WriteIo>(
    write: &mut W,
    output: &mut Vec<u8>,
    mut bytes: &[u8],
    capacity: usize,
    activity: &Activity,
) -> io::Result<()> {
    while !bytes.is_empty() {
        let n = (capacity - output.len()).min(bytes.len());
        output.extend_from_slice(&bytes[..n]);
        bytes = &bytes[n..];
        if output.len() == capacity {
            send_output(write, output, activity).await?;
        }
    }
    Ok(())
}

async fn send_output<W: WriteIo>(
    write: &mut W,
    output: &mut Vec<u8>,
    activity: &Activity,
) -> io::Result<()> {
    if !output.is_empty() {
        let (result, buffer) = write.write(std::mem::take(output)).await;
        *output = buffer;
        output.clear();
        result?;
        activity.touch();
    }
    Ok(())
}

#[cfg(test)]
mod line_tests {
    use super::*;

    #[test]
    fn line_boundaries_preserve_limits_and_following_bytes() {
        compio::runtime::Runtime::new().unwrap().block_on(async {
            for capacity in [1, 2, 3, 8, 1024] {
                for (wire, limit, expected) in [
                    (b"\r\nnext\r\n".as_slice(), 2, Some(b"".as_slice())),
                    (b"abc\r\nnext\r\n", 5, Some(b"abc".as_slice())),
                    (b"abc\r\nnext\r\n", 4, None),
                    (b"abc\r", 5, None),
                    (b"", 2, None),
                    (b"abc\rx\r\nnext\r\n", 8, Some(b"abc\rx".as_slice())),
                ] {
                    let mut reader = Reader {
                        io: Futures(futures_util::io::Cursor::new(wire.to_vec())),
                        bytes: vec![],
                        scratch: vec![],
                        capacity,
                        activity: Activity::new(),
                    };
                    let line = reader.line(limit).await;
                    match expected {
                        Some(expected) => {
                            assert_eq!(line.unwrap(), expected);
                            assert_eq!(reader.line(6).await.unwrap(), b"next");
                        }
                        None => assert!(line.is_err()),
                    }
                }
            }
        });
    }
}
