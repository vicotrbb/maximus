//! Native HTTP/2 framing, HPACK, multiplexing and bounded flow control.
#![forbid(unsafe_code)]
mod frame;
mod hpack;
use frame::Header;
use futures_util::{FutureExt, Stream, future::LocalBoxFuture};
use maximus_core::{
    Body, Bytes, Error, Frame, Handler, HttpBody, Limits, Request, Response, content_length,
    dispatch_request, http,
};
use maximus_runtime::Shutdown;
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};
const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
const PROTOCOL: u32 = 1;
const INTERNAL: u32 = 2;
const FLOW: u32 = 3;
const CLOSED: u32 = 5;
const REFUSED: u32 = 7;
const CANCEL: u32 = 8;
const COMPRESSION: u32 = 9;
const MAX_FRAME: usize = 16384;

trait ReadWire {
    async fn exact(&mut self, size: usize) -> io::Result<Vec<u8>>;
}
trait WriteWire {
    async fn all(&mut self, bytes: Vec<u8>) -> io::Result<Vec<u8>>;
}
struct Native<T>(T);
impl<T: compio::io::AsyncRead> ReadWire for Native<T> {
    async fn exact(&mut self, size: usize) -> io::Result<Vec<u8>> {
        use compio::io::AsyncReadExt;
        let (r, b) = self
            .0
            .read_exact(Vec::with_capacity(size))
            .await
            .into_parts();
        r?;
        Ok(b)
    }
}
impl<T: compio::io::AsyncWrite> WriteWire for Native<T> {
    async fn all(&mut self, bytes: Vec<u8>) -> io::Result<Vec<u8>> {
        use compio::io::AsyncWriteExt;
        let (result, bytes) = self.0.write_all(bytes).await.into_parts();
        result?;
        self.0.flush().await?;
        Ok(bytes)
    }
}
struct Compat<T>(T);
impl<T: futures_util::AsyncRead + Unpin> ReadWire for Compat<T> {
    async fn exact(&mut self, size: usize) -> io::Result<Vec<u8>> {
        use futures_util::AsyncReadExt;
        let mut b = vec![0; size];
        self.0.read_exact(&mut b).await?;
        Ok(b)
    }
}
impl<T: futures_util::AsyncWrite + Unpin> WriteWire for Compat<T> {
    async fn all(&mut self, bytes: Vec<u8>) -> io::Result<Vec<u8>> {
        use futures_util::AsyncWriteExt;
        self.0.write_all(&bytes).await?;
        self.0.flush().await?;
        Ok(bytes)
    }
}
/// Serve native completion I/O, retaining each submitted read until completion.
pub async fn serve<S, H>(
    stream: S,
    handler: Rc<H>,
    limits: Rc<Limits>,
    shutdown: Shutdown,
) -> io::Result<()>
where
    S: compio::io::AsyncRead + compio::io::AsyncWrite + compio::io::util::Splittable + 'static,
    S::ReadHalf: compio::io::AsyncRead + Unpin + 'static,
    S::WriteHalf: compio::io::AsyncWrite + Unpin + 'static,
    H: Handler,
{
    let (r, w) = compio::io::util::Splittable::split(stream);
    run(Native(r), Native(w), handler, limits, shutdown).await
}
/// TLS uses readiness I/O splitting so a pending read cannot lock out writes.
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
    use futures_util::AsyncReadExt;
    let (r, w) = Box::pin(stream).split();
    run(Compat(r), Compat(w), handler, limits, shutdown).await
}
fn emit(out: &mut Vec<u8>, kind: u8, flags: u8, stream: u32, payload: &[u8]) {
    out.extend(
        Header {
            len: payload.len(),
            kind,
            flags,
            stream,
        }
        .encode(),
    );
    out.extend(payload);
}
fn update(out: &mut Vec<u8>, id: u32, n: usize) {
    if n > 0 {
        emit(out, 8, 0, id, &(n as u32).to_be_bytes());
    }
}
fn goaway(out: &mut Vec<u8>, last: u32, code: u32) {
    let mut p = last.to_be_bytes().to_vec();
    p.extend(code.to_be_bytes());
    emit(out, 7, 0, 0, &p);
}
#[derive(Debug)]
enum Fault {
    Connection(u32),
    Stream(u32, u32),
}
type WireResult<T> = Result<T, Fault>;
struct ReceivedFrame {
    header: Header,
    payload: Vec<u8>,
    header_deadline: Option<Instant>,
}
async fn read_frame<R: ReadWire>(
    mut reader: R,
    header_timeout: Duration,
    mut header_deadline: Option<Instant>,
) -> (R, Result<ReceivedFrame, ReadError>) {
    let result = async {
        let h = read_before(&mut reader, 9, header_deadline).await?;
        let header =
            Header::decode(h.try_into().unwrap(), MAX_FRAME).map_err(ReadError::Protocol)?;
        // The deadline starts at the first HEADERS envelope, not after its payload.
        // A continuation retains the same absolute deadline, including its envelope read.
        if header.kind == 1 && header_deadline.is_none() {
            header_deadline = Some(Instant::now() + header_timeout);
        }
        let payload = read_before(&mut reader, header.len, header_deadline).await?;
        Ok(ReceivedFrame {
            header,
            payload,
            header_deadline,
        })
    }
    .await;
    (reader, result)
}
async fn read_before<R: ReadWire>(
    reader: &mut R,
    size: usize,
    deadline: Option<Instant>,
) -> Result<Vec<u8>, ReadError> {
    if let Some(deadline) = deadline {
        if Instant::now() >= deadline {
            return Err(ReadError::Protocol(PROTOCOL));
        }
        compio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            reader.exact(size),
        )
        .await
        .map_err(|_| ReadError::Protocol(PROTOCOL))?
        .map_err(ReadError::Io)
    } else {
        reader.exact(size).await.map_err(ReadError::Io)
    }
}
enum ReadError {
    Io(io::Error),
    Protocol(u32),
}
#[derive(Default)]
struct IncomingState {
    queue: VecDeque<Frame<Bytes>>,
    tail: bytes::BytesMut,
    credit: usize,
    ended: bool,
    error: bool,
    reader: Option<Waker>,
    connection: Option<Waker>,
}
struct Incoming(Rc<RefCell<IncomingState>>);
impl Stream for Incoming {
    type Item = Result<Frame<Bytes>, Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut s = self.0.borrow_mut();
        let frame = s
            .queue
            .pop_front()
            .or_else(|| (!s.tail.is_empty()).then(|| Frame::data(s.tail.split().freeze())));
        if let Some(frame) = frame {
            s.credit += frame.data_ref().map_or(0, Bytes::len);
            if let Some(w) = s.connection.take() {
                w.wake();
            }
            return Poll::Ready(Some(Ok(frame)));
        }
        if s.error {
            s.error = false;
            return Poll::Ready(Some(Err(Error::bad_request("HTTP/2 stream reset"))));
        }
        if s.ended {
            return Poll::Ready(None);
        }
        s.reader = Some(cx.waker().clone());
        Poll::Pending
    }
}
impl Drop for Incoming {
    fn drop(&mut self) {
        let mut s = self.0.borrow_mut();
        if let Some(w) = s.connection.take() {
            w.wake();
        }
    }
}
struct StreamState {
    method: http::Method,
    incoming: Rc<RefCell<IncomingState>>,
    handler: Option<LocalBoxFuture<'static, Response>>,
    body: Option<Body>,
    pending: Bytes,
    send: i64,
    recv: i64,
    received: u64,
    expected: Option<u64>,
    sent: u64,
    response_length: Option<u64>,
    remote_end: bool,
    local_end: bool,
    deadline: Instant,
}
struct HeaderBlock {
    id: u32,
    end: bool,
    bytes: Vec<u8>,
    deadline: Instant,
    reject: Option<u32>,
    discard: bool,
}
struct Connection<H> {
    handler: Rc<H>,
    limits: Rc<Limits>,
    decoder: hpack::Decoder,
    streams: BTreeMap<u32, StreamState>,
    header: Option<HeaderBlock>,
    last: u32,
    accepted: u32,
    count: usize,
    send: i64,
    recv: i64,
    initial_send: i64,
    max_frame: usize,
    first: bool,
    settings_ack: bool,
    settings_deadline: Instant,
    draining: Option<Instant>,
    last_activity: Instant,
    cursor: u32,
    controls: usize,
}
impl<H: Handler> Connection<H> {
    fn new(handler: Rc<H>, limits: Rc<Limits>) -> Self {
        let now = Instant::now();
        Self {
            handler,
            decoder: hpack::Decoder::new(4096, limits.header_bytes, limits.headers),
            recv: i64::from(limits.connection_window_bytes.max(65535)),
            settings_deadline: now + limits.header_timeout,
            limits,
            streams: BTreeMap::new(),
            header: None,
            last: 0,
            accepted: 0,
            count: 0,
            send: 65535,
            initial_send: 65535,
            max_frame: 16384,
            first: true,
            settings_ack: false,
            draining: None,
            last_activity: now,
            cursor: 0,
            controls: 0,
        }
    }
    fn start(&self, out: &mut Vec<u8>) {
        let mut p = vec![];
        for (k, v) in [
            (3, self.limits.streams),
            (4, self.limits.stream_window_bytes),
            (6, self.limits.header_bytes.min(u32::MAX as usize) as u32),
        ] {
            p.extend((k as u16).to_be_bytes());
            p.extend(v.to_be_bytes());
        }
        emit(out, 4, 0, 0, &p);
        if self.recv > 65535 {
            update(out, 0, (self.recv - 65535) as usize);
        }
    }
    fn drain(&mut self, out: &mut Vec<u8>) {
        if self.draining.is_none() {
            goaway(out, self.accepted, 0);
            self.draining = Some(Instant::now() + self.limits.drain_timeout);
        }
    }
    fn remove_stream(&mut self, id: u32) -> Option<StreamState> {
        // A trailer can be awaiting CONTINUATION when its stream closes. Preserve
        // compression/deadline processing, but never reinterpret it as a request.
        if let Some(block) = self.header.as_mut()
            && block.id == id
        {
            block.discard = true;
        }
        self.streams.remove(&id)
    }
    fn reset(&mut self, id: u32, code: u32, out: &mut Vec<u8>) {
        if let Some(s) = self.remove_stream(id) {
            let mut input = s.incoming.borrow_mut();
            let credit = input.credit
                + input.tail.len()
                + input
                    .queue
                    .iter()
                    .map(|f| f.data_ref().map_or(0, Bytes::len))
                    .sum::<usize>();
            self.recv += credit as i64;
            update(out, 0, credit);
            input.queue.clear();
            input.tail.clear();
            input.error = true;
            input.ended = true;
            if let Some(w) = input.reader.take() {
                w.wake();
            }
        }
        emit(out, 3, 0, id, &code.to_be_bytes());
    }
    fn process(&mut self, frame: ReceivedFrame, out: &mut Vec<u8>) -> WireResult<()> {
        let ReceivedFrame {
            header: h,
            payload: p,
            header_deadline,
        } = frame;
        self.last_activity = Instant::now();
        // Bound control-only churn independently from flow-controlled application bytes.
        if h.kind != 0 || h.len == 0 || !self.streams.contains_key(&h.stream) {
            self.controls = self.controls.saturating_add(1);
        }
        if self.controls > 1024usize.saturating_add(self.count.saturating_mul(64)) {
            return Err(Fault::Connection(11));
        }
        if self.first {
            self.first = false;
            if h.kind != 4 || h.flags & 1 != 0 {
                return Err(Fault::Connection(PROTOCOL));
            }
        }
        if self
            .header
            .as_ref()
            .is_some_and(|b| h.kind != 9 || h.stream != b.id)
        {
            return Err(Fault::Connection(PROTOCOL));
        }
        match h.kind {
            4 => {
                if h.flags & 1 != 0 {
                    if self.settings_ack {
                        return Err(Fault::Connection(PROTOCOL));
                    }
                    self.settings_ack = true;
                    return Ok(());
                }
                for setting in p.chunks_exact(6) {
                    let key = u16::from_be_bytes(setting[..2].try_into().unwrap());
                    let value = u32::from_be_bytes(setting[2..].try_into().unwrap());
                    match key {
                        2 if value > 1 => return Err(Fault::Connection(PROTOCOL)),
                        4 => {
                            if value > 0x7fff_ffff {
                                return Err(Fault::Connection(FLOW));
                            }
                            let delta = i64::from(value) - self.initial_send;
                            for s in self.streams.values_mut() {
                                s.send += delta;
                                if s.send > 0x7fff_ffff {
                                    return Err(Fault::Connection(FLOW));
                                }
                            }
                            self.initial_send = value.into();
                        }
                        5 => {
                            if !(16384..=16777215).contains(&value) {
                                return Err(Fault::Connection(PROTOCOL));
                            }
                            self.max_frame = value as usize;
                        }
                        _ => {}
                    }
                }
                emit(out, 4, 1, 0, &[]);
            }
            6 => {
                if h.flags & 1 == 0 {
                    emit(out, 6, 1, 0, &p);
                }
            }
            7 => self.drain(out),
            8 => {
                let increment = u32::from_be_bytes(p[..].try_into().unwrap()) & 0x7fff_ffff;
                if h.stream == 0 {
                    if increment == 0 {
                        return Err(Fault::Connection(PROTOCOL));
                    }
                    self.send += i64::from(increment);
                    if self.send > 0x7fff_ffff {
                        return Err(Fault::Connection(FLOW));
                    }
                } else if let Some(s) = self.streams.get_mut(&h.stream) {
                    if increment == 0 {
                        return Err(Fault::Stream(h.stream, PROTOCOL));
                    }
                    s.send += i64::from(increment);
                    if s.send > 0x7fff_ffff {
                        return Err(Fault::Stream(h.stream, FLOW));
                    }
                } else if h.stream.is_multiple_of(2) || h.stream > self.last {
                    return Err(Fault::Connection(PROTOCOL));
                }
            }
            3 => {
                if h.stream.is_multiple_of(2) || h.stream > self.last {
                    return Err(Fault::Connection(PROTOCOL));
                }
                self.reset(h.stream, CANCEL, out);
                out.truncate(out.len() - 13);
            }
            2 => {
                if u32::from_be_bytes(p[..4].try_into().unwrap()) & 0x7fff_ffff == h.stream {
                    return Err(Fault::Stream(h.stream, PROTOCOL));
                }
            }
            1 => {
                let known = self.streams.contains_key(&h.stream);
                if h.stream.is_multiple_of(2) {
                    return Err(Fault::Connection(PROTOCOL));
                }
                // RFC 9113 §5.1 permits minimal processing for every closed stream.
                // The high-water mark plus active map avoids per-closed-stream tombstones.
                let discard = !known && h.stream <= self.last;
                if !known && !discard {
                    self.last = h.stream;
                }
                let mut payload = &p[..];
                let mut priority_error = false;
                if h.flags & 8 != 0 {
                    let pad = payload[0] as usize;
                    payload = &payload[1..];
                    if pad > payload.len() {
                        return Err(Fault::Connection(PROTOCOL));
                    }
                    payload = &payload[..payload.len() - pad];
                }
                if h.flags & 0x20 != 0 {
                    if payload.len() < 5 {
                        return Err(Fault::Connection(frame::FRAME_SIZE));
                    }
                    if u32::from_be_bytes(payload[..4].try_into().unwrap()) & 0x7fff_ffff
                        == h.stream
                    {
                        priority_error = true;
                    }
                    payload = &payload[5..];
                }
                let refuse = !known
                    && (self.streams.len() >= self.limits.streams as usize
                        || self.draining.is_some()
                        || self.count >= self.limits.requests_per_connection);
                self.header = Some(HeaderBlock {
                    id: h.stream,
                    end: h.flags & 1 != 0,
                    bytes: payload.to_vec(),
                    deadline: header_deadline.expect("HEADERS envelope establishes its deadline"),
                    discard,
                    reject: if priority_error {
                        Some(PROTOCOL)
                    } else if refuse {
                        Some(REFUSED)
                    } else {
                        None
                    },
                });
                self.finish_headers(h.flags & 4 != 0, out)?;
            }
            9 => {
                let Some(block) = self.header.as_mut() else {
                    return Err(Fault::Connection(PROTOCOL));
                };
                block.bytes.extend(p);
                self.finish_headers(h.flags & 4 != 0, out)?;
            }
            0 => {
                if h.flags & 8 != 0 && p[0] as usize >= p.len() {
                    return Err(Fault::Connection(PROTOCOL));
                }
                self.recv -= h.len as i64;
                if self.recv < 0 {
                    return Err(Fault::Connection(FLOW));
                }
                let Some(s) = self.streams.get_mut(&h.stream) else {
                    self.recv += h.len as i64;
                    update(out, 0, h.len);
                    return if h.stream.is_multiple_of(2) || h.stream > self.last {
                        Err(Fault::Connection(PROTOCOL))
                    } else {
                        Ok(())
                    };
                };
                if s.remote_end {
                    self.recv += h.len as i64;
                    update(out, 0, h.len);
                    return Err(Fault::Stream(h.stream, CLOSED));
                }
                s.recv -= h.len as i64;
                if s.recv < 0 {
                    self.recv += h.len as i64;
                    update(out, 0, h.len);
                    return Err(Fault::Stream(h.stream, FLOW));
                }
                let mut payload = &p[..];
                if h.flags & 8 != 0 {
                    let pad = payload[0] as usize;
                    payload = &payload[1..];
                    if pad > payload.len() {
                        return Err(Fault::Connection(PROTOCOL));
                    }
                    payload = &payload[..payload.len() - pad];
                }
                let padding = h.len - payload.len();
                self.recv += padding as i64;
                s.recv += padding as i64;
                update(out, 0, padding);
                update(out, h.stream, padding);
                s.received += payload.len() as u64;
                if s.received > self.limits.body_bytes as u64
                    || s.expected.is_some_and(|n| s.received > n)
                {
                    self.recv += payload.len() as i64;
                    update(out, 0, payload.len());
                    return Err(Fault::Stream(h.stream, PROTOCOL));
                }
                let mut input = s.incoming.borrow_mut();
                if !payload.is_empty() {
                    input.tail.extend_from_slice(payload);
                    if input.tail.len() >= MAX_FRAME {
                        let data = input.tail.split().freeze();
                        input.queue.push_back(Frame::data(data));
                    }
                }
                s.deadline = Instant::now() + self.limits.request_timeout;
                if h.flags & 1 != 0 {
                    s.remote_end = true;
                    input.ended = true;
                    if s.expected.is_some_and(|n| s.received != n) {
                        return Err(Fault::Stream(h.stream, PROTOCOL));
                    }
                }
                if let Some(w) = input.reader.take() {
                    w.wake();
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn finish_headers(&mut self, end: bool, out: &mut Vec<u8>) -> WireResult<()> {
        if self.header.as_ref().unwrap().bytes.len()
            > self
                .limits
                .header_bytes
                .saturating_mul(4)
                .saturating_add(1024)
        {
            return Err(Fault::Connection(COMPRESSION));
        }
        if !end {
            return Ok(());
        }
        let block = self.header.take().unwrap();
        let fields = self
            .decoder
            .decode(&block.bytes)
            .map_err(|_| Fault::Connection(COMPRESSION))?;
        if block.discard {
            return Ok(());
        }
        if let Some(code) = block.reject {
            return Err(Fault::Stream(block.id, code));
        }
        if let Some(s) = self.streams.get_mut(&block.id) {
            if s.remote_end {
                return Err(Fault::Stream(block.id, CLOSED));
            }
            if !block.end {
                return Err(Fault::Stream(block.id, PROTOCOL));
            }
            let headers =
                regular_headers(&fields, true).map_err(|_| Fault::Stream(block.id, PROTOCOL))?;
            if s.expected.is_some_and(|n| s.received != n) {
                return Err(Fault::Stream(block.id, PROTOCOL));
            }
            s.remote_end = true;
            let mut input = s.incoming.borrow_mut();
            if !input.tail.is_empty() {
                let data = input.tail.split().freeze();
                input.queue.push_back(Frame::data(data));
            }
            input.queue.push_back(Frame::trailers(headers));
            input.ended = true;
            if let Some(w) = input.reader.take() {
                w.wake();
            }
            return Ok(());
        }
        let mut req = request_headers(&fields).map_err(|_| Fault::Stream(block.id, PROTOCOL))?;
        let expected =
            content_length(req.headers()).map_err(|_| Fault::Stream(block.id, PROTOCOL))?;
        if block.end && expected.is_some_and(|n| n != 0) {
            return Err(Fault::Stream(block.id, PROTOCOL));
        }
        let method = req.method().clone();
        let incoming = Rc::new(RefCell::new(IncomingState {
            ended: block.end,
            ..Default::default()
        }));
        *req.body_mut() = Body::incoming(Incoming(incoming.clone()), self.limits.body_bytes);
        let handler = self.handler.clone();
        let limits = self.limits.clone();
        self.streams.insert(
            block.id,
            StreamState {
                method,
                incoming,
                handler: Some(
                    async move { dispatch_request(Request::new(req), handler, limits).await }
                        .boxed_local(),
                ),
                body: None,
                pending: Bytes::new(),
                send: self.initial_send,
                recv: self.limits.stream_window_bytes.into(),
                received: 0,
                expected,
                sent: 0,
                response_length: None,
                remote_end: block.end,
                local_end: false,
                deadline: Instant::now() + self.limits.request_timeout,
            },
        );
        self.accepted = block.id;
        self.count += 1;
        if self.count >= self.limits.requests_per_connection {
            self.drain(out);
        }
        Ok(())
    }
    fn poll(&mut self, cx: &mut Context<'_>, out: &mut Vec<u8>) -> Poll<()> {
        let mut reset = vec![];
        let ids: Vec<_> = self
            .streams
            .range((
                std::ops::Bound::Excluded(self.cursor),
                std::ops::Bound::Unbounded,
            ))
            .chain(self.streams.range(..=self.cursor))
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            let s = self.streams.get_mut(&id).unwrap();
            {
                let mut input = s.incoming.borrow_mut();
                input.connection = Some(cx.waker().clone());
                let credit = std::mem::take(&mut input.credit);
                self.recv += credit as i64;
                s.recv += credit as i64;
                update(out, 0, credit);
                if !s.remote_end {
                    update(out, id, credit);
                }
            }
            if let Some(future) = s.handler.as_mut()
                && let Poll::Ready(response) = future.as_mut().poll(cx)
            {
                s.handler = None;
                let mut response = response;
                if s.method == http::Method::CONNECT && response.status().is_success() {
                    *response.status_mut() = http::StatusCode::NOT_IMPLEMENTED;
                    *response.body_mut() = Body::empty();
                    response.headers_mut().remove("content-length");
                }
                let (parts, body) = response.into_parts();
                let mut map = parts.headers;
                map.remove("connection");
                map.remove("transfer-encoding");
                map.remove("keep-alive");
                map.remove("proxy-connection");
                map.remove("upgrade");
                if !map.contains_key("date") {
                    map.insert("date", maximus_core::response_date());
                }
                if !map.contains_key("content-length")
                    && parts.status != http::StatusCode::NO_CONTENT
                    && parts.status != http::StatusCode::NOT_MODIFIED
                    && !parts.status.is_informational()
                    && !(s.method == http::Method::CONNECT && parts.status.is_success())
                    && let Some(n) = body.size_hint().exact()
                {
                    map.insert("content-length", n.into());
                }
                s.response_length = match content_length(&map) {
                    Ok(n) => n,
                    Err(_) => {
                        reset.push((id, INTERNAL));
                        continue;
                    }
                };
                if parts.status.is_informational()
                    || s.method == http::Method::CONNECT
                        && parts.status.is_success()
                        && map.contains_key("content-length")
                {
                    reset.push((id, INTERNAL));
                    continue;
                }
                if !valid_fields(&map, false, &self.limits) {
                    reset.push((id, INTERNAL));
                    continue;
                }
                let empty = body.is_end_stream();
                if empty
                    && s.response_length.is_some_and(|n| n > 0)
                    && s.method != http::Method::HEAD
                    && parts.status != http::StatusCode::NOT_MODIFIED
                {
                    reset.push((id, INTERNAL));
                    continue;
                }
                emit_headers(
                    out,
                    id,
                    std::iter::once((b":status".as_slice(), parts.status.as_str().as_bytes()))
                        .chain(
                            map.iter()
                                .map(|(n, v)| (n.as_str().as_bytes(), v.as_bytes())),
                        ),
                    empty,
                    self.max_frame,
                );
                s.local_end = empty;
                if !empty {
                    s.body = Some(body);
                }
                s.deadline = Instant::now() + self.limits.request_timeout;
            }
            if !s.local_end && s.handler.is_none() {
                if s.pending.is_empty()
                    && let Some(body) = s.body.as_mut()
                {
                    match Pin::new(body).poll_frame(cx) {
                        Poll::Ready(Some(Ok(frame))) => {
                            if let Some(data) = frame.data_ref() {
                                s.pending = data.clone();
                                if data.is_empty() {
                                    cx.waker().wake_by_ref();
                                }
                            } else if let Ok(trailers) = frame.into_trailers() {
                                if !valid_fields(&trailers, true, &self.limits)
                                    || s.response_length.is_some_and(|n| n != s.sent)
                                {
                                    reset.push((id, INTERNAL));
                                } else {
                                    emit_headers(
                                        out,
                                        id,
                                        trailers
                                            .iter()
                                            .map(|(n, v)| (n.as_str().as_bytes(), v.as_bytes())),
                                        true,
                                        self.max_frame,
                                    );
                                    s.local_end = true;
                                    s.body = None;
                                }
                            }
                        }
                        Poll::Ready(None) => {
                            if s.response_length.is_some_and(|n| n != s.sent) {
                                reset.push((id, INTERNAL));
                            } else {
                                emit(out, 0, 1, id, &[]);
                                s.local_end = true;
                                s.body = None;
                            }
                        }
                        Poll::Ready(Some(Err(_))) => reset.push((id, INTERNAL)),
                        Poll::Pending => {}
                    }
                }
                if !s.pending.is_empty() && s.send > 0 && self.send > 0 {
                    let n = s
                        .pending
                        .len()
                        .min(self.max_frame)
                        .min(self.limits.io_buffer_bytes)
                        .min(s.send as usize)
                        .min(self.send as usize);
                    let end = s.pending.len() == n
                        && s.body.as_ref().is_some_and(HttpBody::is_end_stream);
                    if s.response_length.is_some_and(|total| {
                        s.sent + n as u64 > total || end && s.sent + n as u64 != total
                    }) {
                        reset.push((id, INTERNAL));
                    } else {
                        let data = s.pending.split_to(n);
                        s.sent += n as u64;
                        s.send -= n as i64;
                        self.send -= n as i64;
                        emit(out, 0, u8::from(end), id, &data);
                        if end {
                            s.local_end = true;
                            s.body = None;
                        }
                        s.deadline = Instant::now() + self.limits.request_timeout;
                    }
                }
            }
            if s.local_end {
                if !s.remote_end {
                    // RFC 9113 §8.1: the complete response remains valid while the
                    // server declines the rest of the upload, even for an error status.
                    reset.push((id, 0));
                } else {
                    let input = s.incoming.borrow();
                    let credit = input.credit
                        + input.tail.len()
                        + input
                            .queue
                            .iter()
                            .map(|f| f.data_ref().map_or(0, Bytes::len))
                            .sum::<usize>();
                    drop(input);
                    self.recv += credit as i64;
                    update(out, 0, credit);
                    self.remove_stream(id);
                }
            }
            self.cursor = id;
            if out.len() >= self.limits.io_buffer_bytes {
                break;
            }
        }
        for (id, code) in reset {
            self.reset(id, code, out);
        }
        if out.is_empty() {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
    fn deadline(&self) -> Instant {
        let mut t = self.last_activity + self.limits.idle_timeout;
        if !self.settings_ack {
            t = t.min(self.settings_deadline);
        }
        if let Some(h) = &self.header {
            t = t.min(h.deadline);
        }
        if let Some(d) = self.draining {
            t = t.min(d);
        }
        for s in self.streams.values() {
            if !s.remote_end || s.handler.is_none() {
                t = t.min(s.deadline);
            }
        }
        t
    }
    fn expired(&mut self, out: &mut Vec<u8>) -> bool {
        let now = Instant::now();
        if self.draining.is_some_and(|d| now >= d)
            || now >= self.last_activity + self.limits.idle_timeout
        {
            return true;
        }
        if !self.settings_ack && now >= self.settings_deadline {
            goaway(out, self.accepted, 4);
            return true;
        }
        if self.header.as_ref().is_some_and(|h| now >= h.deadline) {
            goaway(out, self.accepted, PROTOCOL);
            return true;
        }
        let ids: Vec<_> = self
            .streams
            .iter()
            .filter(|(_, s)| (!s.remote_end || s.handler.is_none()) && now >= s.deadline)
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            self.reset(id, CANCEL, out);
        }
        false
    }
}
fn emit_headers<'a>(
    out: &mut Vec<u8>,
    id: u32,
    fields: impl IntoIterator<Item = (&'a [u8], &'a [u8])>,
    end: bool,
    max: usize,
) {
    let mut block = vec![];
    hpack::encode(fields, &mut block);
    let chunks = block.chunks(max);
    let count = chunks.len();
    for (i, chunk) in chunks.enumerate() {
        emit(
            out,
            if i == 0 { 1 } else { 9 },
            if i + 1 == count { 4 } else { 0 } | if i == 0 && end { 1 } else { 0 },
            id,
            chunk,
        );
    }
}
fn valid_fields(fields: &http::HeaderMap, trailers: bool, limits: &Limits) -> bool {
    fields.len() <= limits.headers
        && fields
            .iter()
            .try_fold(0usize, |total, (n, v)| {
                total.checked_add(n.as_str().len() + v.len() + 32)
            })
            .is_some_and(|n| n <= limits.header_bytes)
        && fields
            .iter()
            .all(|(n, v)| valid_field(n.as_str().as_bytes(), v.as_bytes(), trailers))
}
fn valid_field(n: &[u8], v: &[u8], trailers: bool) -> bool {
    !n.is_empty()
        && n[0] != b':'
        && !n.iter().any(u8::is_ascii_uppercase)
        && !v.first().is_some_and(|b| b.is_ascii_whitespace())
        && !v.last().is_some_and(|b| b.is_ascii_whitespace())
        && !v.iter().any(|b| matches!(b, 0 | 10 | 13))
        && !matches!(
            n,
            b"connection" | b"proxy-connection" | b"keep-alive" | b"transfer-encoding" | b"upgrade"
        )
        && !(n == b"te" && v != b"trailers")
        && !(trailers && matches!(n, b"content-length" | b"host" | b"te"))
}
fn regular_headers(fields: &[(Vec<u8>, Vec<u8>)], trailers: bool) -> Result<http::HeaderMap, ()> {
    let mut map = http::HeaderMap::new();
    for (n, v) in fields {
        if !valid_field(n, v, trailers) {
            return Err(());
        }
        let name = http::header::HeaderName::from_bytes(n).map_err(|_| ())?;
        let value = http::HeaderValue::from_bytes(v).map_err(|_| ())?;
        map.append(name, value);
    }
    Ok(map)
}
fn request_headers(fields: &[(Vec<u8>, Vec<u8>)]) -> Result<http::Request<Body>, ()> {
    let (mut method, mut scheme, mut authority, mut path) = (None, None, None, None);
    let mut regular = 0;
    for (i, (n, v)) in fields.iter().enumerate() {
        if !n.starts_with(b":") {
            regular = i;
            break;
        }
        regular = i + 1;
        let slot = match n.as_slice() {
            b":method" => &mut method,
            b":scheme" => &mut scheme,
            b":authority" => &mut authority,
            b":path" => &mut path,
            _ => return Err(()),
        };
        if slot
            .replace(std::str::from_utf8(v).map_err(|_| ())?)
            .is_some()
        {
            return Err(());
        }
    }
    let method = method.ok_or(())?.parse::<http::Method>().map_err(|_| ())?;
    let headers = regular_headers(&fields[regular..], false)?;
    let uri = if method == http::Method::CONNECT {
        if scheme.is_some() || path.is_some() {
            return Err(());
        }
        let a = authority.ok_or(())?;
        let parsed = a.parse::<http::uri::Authority>().map_err(|_| ())?;
        if parsed.port_u16().is_none() {
            return Err(());
        }
        http::Uri::builder()
            .authority(parsed)
            .build()
            .map_err(|_| ())?
    } else {
        let p = path.ok_or(())?;
        if p.is_empty()
            || !(p.starts_with('/') || p == "*" && method == http::Method::OPTIONS)
            || p.contains('#')
        {
            return Err(());
        }
        let scheme = scheme.ok_or(())?;
        let a = authority
            .or_else(|| headers.get("host").and_then(|v| v.to_str().ok()))
            .ok_or(())?;
        if a.is_empty() {
            return Err(());
        }
        http::Uri::builder()
            .scheme(scheme)
            .authority(a)
            .path_and_query(p)
            .build()
            .map_err(|_| ())?
    };
    if let (Some(authority), Some(host)) = (authority, headers.get("host"))
        && host.as_bytes() != authority.as_bytes()
    {
        return Err(());
    }
    let mut request = http::Request::builder()
        .version(http::Version::HTTP_2)
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .map_err(|_| ())?;
    *request.headers_mut() = headers;
    Ok(request)
}
async fn run<R: ReadWire + 'static, W: WriteWire, H: Handler>(
    mut reader: R,
    mut writer: W,
    handler: Rc<H>,
    limits: Rc<Limits>,
    shutdown: Shutdown,
) -> io::Result<()> {
    let preface = {
        let read = compio::time::timeout(limits.header_timeout, reader.exact(24)).fuse();
        let stop = shutdown.cancelled().fuse();
        futures_util::pin_mut!(read, stop);
        futures_util::select_biased! {_=stop=>return Ok(()), result=read=>result.map_err(|_|io::Error::new(io::ErrorKind::TimedOut,"HTTP/2 preface timeout"))??}
    };
    if preface != PREFACE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid HTTP/2 preface",
        ));
    }
    let mut connection = Connection::new(handler, limits.clone());
    let mut output = vec![];
    connection.start(&mut output);
    let mut reading = Box::pin(read_frame(reader, limits.header_timeout, None));
    let mut stopped = false;
    let mut read_turn = true;
    loop {
        if !output.is_empty() {
            let bytes = std::mem::take(&mut output);
            let budget = connection
                .draining
                .map_or(limits.request_timeout, |deadline| {
                    deadline
                        .saturating_duration_since(Instant::now())
                        .min(limits.request_timeout)
                });
            let writing = compio::time::timeout(budget, writer.all(bytes)).fuse();
            let draining = connection.draining.is_some();
            let stop = async {
                if draining {
                    futures_util::future::pending::<()>().await;
                }
                shutdown.cancelled().await
            }
            .fuse();
            futures_util::pin_mut!(writing, stop);
            let result = futures_util::select_biased! {
                result=writing=>Some(result),
                _=stop=>None,
            };
            let mut bytes = match result {
                Some(result) => result.map_err(|_| {
                    io::Error::new(io::ErrorKind::TimedOut, "HTTP/2 write timeout")
                })??,
                None => {
                    connection.drain(&mut output);
                    compio::time::timeout(limits.drain_timeout, writing)
                        .await
                        .map_err(|_| {
                            io::Error::new(io::ErrorKind::TimedOut, "HTTP/2 drain timeout")
                        })?
                        .map_err(|_| {
                            io::Error::new(io::ErrorKind::TimedOut, "HTTP/2 write timeout")
                        })??
                }
            };
            // Reuse normal frame batches without retaining unusually large header batches.
            if bytes.capacity() <= limits.io_buffer_bytes.saturating_mul(2) {
                bytes.clear();
                bytes.extend_from_slice(&output);
                output = bytes;
            }
            connection.last_activity = Instant::now();
            if !output.is_empty() {
                continue;
            }
        }
        if stopped || connection.draining.is_some() && connection.streams.is_empty() {
            return Ok(());
        }
        let deadline = connection.deadline();
        enum Event<T> {
            Read(T),
            Work,
            Stop,
            Timer,
        }
        let draining = connection.draining.is_some();
        let event = {
            let work = futures_util::future::poll_fn(|cx| connection.poll(cx, &mut output)).fuse();
            let timer =
                compio::time::sleep(deadline.saturating_duration_since(Instant::now())).fuse();
            let stop = async {
                if draining {
                    futures_util::future::pending::<()>().await;
                }
                shutdown.cancelled().await
            }
            .fuse();
            futures_util::pin_mut!(work, timer, stop);
            if read_turn {
                futures_util::select_biased! {_=stop=>Event::Stop,_=timer=>Event::Timer,r=reading.as_mut().fuse()=>Event::Read(r),_=work=>Event::Work}
            } else {
                futures_util::select_biased! {_=stop=>Event::Stop,_=timer=>Event::Timer,_=work=>Event::Work,r=reading.as_mut().fuse()=>Event::Read(r)}
            }
        };
        match event {
            Event::Read((r, result)) => {
                read_turn = false;
                match result {
                    Ok(frame) => match connection.process(frame, &mut output) {
                        Ok(()) => {}
                        Err(Fault::Stream(id, code)) => connection.reset(id, code, &mut output),
                        Err(Fault::Connection(code)) => {
                            goaway(&mut output, connection.accepted, code);
                            stopped = true;
                        }
                    },
                    Err(ReadError::Protocol(code)) => {
                        goaway(&mut output, connection.accepted, code);
                        stopped = true;
                    }
                    Err(ReadError::Io(e)) => {
                        if e.kind() == io::ErrorKind::UnexpectedEof {
                            return Ok(());
                        }
                        return Err(e);
                    }
                }
                reading.as_mut().set(read_frame(
                    r,
                    limits.header_timeout,
                    connection.header.as_ref().map(|h| h.deadline),
                ));
            }
            Event::Work => {
                read_turn = true;
            }
            Event::Stop => connection.drain(&mut output),
            Event::Timer => stopped = connection.expired(&mut output),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PartialWriter {
        bytes: Vec<u8>,
        fail_after: usize,
        fail_flush: bool,
    }
    impl PartialWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.bytes.len() >= self.fail_after {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            let n = bytes.len().min(2).min(self.fail_after - self.bytes.len());
            self.bytes.extend_from_slice(&bytes[..n]);
            Ok(n)
        }
        fn flush(&self) -> io::Result<()> {
            if self.fail_flush {
                Err(io::ErrorKind::Other.into())
            } else {
                Ok(())
            }
        }
    }
    impl compio::io::AsyncWrite for PartialWriter {
        async fn write<T: compio::buf::IoBuf>(&mut self, bytes: T) -> compio::BufResult<usize, T> {
            compio::BufResult(self.write(bytes.as_init()), bytes)
        }
        async fn flush(&mut self) -> io::Result<()> {
            PartialWriter::flush(self)
        }
        async fn shutdown(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl futures_util::AsyncWrite for PartialWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(self.write(bytes))
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(PartialWriter::flush(&self))
        }
        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[test]
    fn wire_writers_return_original_buffer_after_partial_writes() {
        let runtime = compio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            for native in [true, false] {
                let writer = PartialWriter {
                    bytes: vec![],
                    fail_after: usize::MAX,
                    fail_flush: false,
                };
                let mut bytes = Vec::with_capacity(128);
                bytes.extend_from_slice(b"hello");
                let pointer = bytes.as_ptr();
                let returned = if native {
                    let mut writer = Native(writer);
                    let returned = writer.all(bytes).await.unwrap();
                    assert_eq!(writer.0.bytes, b"hello");
                    returned
                } else {
                    let mut writer = Compat(writer);
                    let returned = writer.all(bytes).await.unwrap();
                    assert_eq!(writer.0.bytes, b"hello");
                    returned
                };
                assert_eq!(returned, b"hello");
                assert_eq!(returned.as_ptr(), pointer);
                assert_eq!(returned.capacity(), 128);
            }
        });
    }

    #[test]
    fn wire_writers_propagate_partial_write_and_flush_errors() {
        let runtime = compio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            for native in [true, false] {
                for (fail_after, fail_flush, expected, written) in [
                    (3, false, io::ErrorKind::BrokenPipe, b"hel".as_slice()),
                    (usize::MAX, true, io::ErrorKind::Other, b"hello".as_slice()),
                ] {
                    let writer = PartialWriter {
                        bytes: vec![],
                        fail_after,
                        fail_flush,
                    };
                    let bytes = b"hello".to_vec();
                    if native {
                        let mut writer = Native(writer);
                        assert_eq!(writer.all(bytes).await.unwrap_err().kind(), expected);
                        assert_eq!(writer.0.bytes, written);
                    } else {
                        let mut writer = Compat(writer);
                        assert_eq!(writer.all(bytes).await.unwrap_err().kind(), expected);
                        assert_eq!(writer.0.bytes, written);
                    }
                }
            }
        });
    }

    #[test]
    fn outbound_fields_preserve_h2_validation_and_duplicate_limits() {
        for (name, value, trailers, valid) in [
            ("x-test", "good", false, true),
            ("x-test", " good", false, false),
            ("x-test", "good ", false, false),
            ("x-test", "\tgood", false, false),
            ("x-test", "good\t", false, false),
            ("te", "trailers", false, true),
            ("te", "gzip", false, false),
            ("connection", "close", false, false),
            ("host", "localhost", true, false),
            ("content-length", "0", true, false),
            ("te", "trailers", true, false),
            ("x-test", "good", true, true),
        ] {
            let mut headers = http::HeaderMap::new();
            headers.insert(name, http::HeaderValue::from_static(value));
            assert_eq!(
                valid_fields(&headers, trailers, &Limits::default()),
                valid,
                "{name}: {value:?}"
            );
        }

        let mut headers = http::HeaderMap::new();
        headers.append("x", http::HeaderValue::from_static("a"));
        headers.append("x", http::HeaderValue::from_static("b"));
        let mut limits = Limits {
            headers: 2,
            header_bytes: 68,
            ..Limits::default()
        };
        assert!(valid_fields(&headers, false, &limits));
        limits.headers = 1;
        assert!(!valid_fields(&headers, false, &limits));
        limits.headers = 2;
        limits.header_bytes = 67;
        assert!(!valid_fields(&headers, false, &limits));
    }
}
