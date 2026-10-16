//! Independent UDP HTTP/3 server with bounded QUIC flow control and request streams.
#![forbid(unsafe_code)]
use bytes::{Buf, Bytes};
use compio::quic::{self, Endpoint, h3};
use futures_util::{
    FutureExt, StreamExt,
    future::{Either, pending},
    stream::FuturesUnordered,
};
use http_body_util::BodyExt;
use maximus_core::{Body, Error, Handler, Limits, Request, http};
use maximus_runtime::Shutdown;
use maximus_tls::TlsConfig;
use std::{cell::Cell, io, rc::Rc, sync::Arc};

pub fn endpoint(
    socket: std::net::UdpSocket,
    tls: &TlsConfig,
    limits: &Limits,
) -> io::Result<Endpoint> {
    limits.validate()?;
    if limits.protocol_capacity(true, true) == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "memory budget cannot admit one QUIC connection",
        ));
    }
    let mut crypto = (**tls.rustls()).clone();
    crypto.alpn_protocols = vec![b"h3".to_vec()];
    crypto.max_early_data_size = 0;
    let crypto =
        quic::crypto::rustls::QuicServerConfig::try_from(crypto).map_err(io::Error::other)?;
    let mut transport = quic::TransportConfig::default();
    transport
        .max_concurrent_bidi_streams(limits.streams.into())
        .max_concurrent_uni_streams(3u32.into())
        .stream_receive_window(limits.stream_window_bytes.into())
        .receive_window(limits.connection_window_bytes.into())
        .send_window(limits.connection_window_bytes as u64)
        .max_idle_timeout(Some(
            limits.idle_timeout.try_into().map_err(io::Error::other)?,
        ));
    let mut server = quic::ServerConfig::with_crypto(Arc::new(crypto));
    server
        .transport_config(Arc::new(transport))
        .migration(false)
        .max_incoming(limits.protocol_capacity(true, true))
        .incoming_buffer_size(limits.io_buffer_bytes as u64)
        .incoming_buffer_size_total(limits.memory_bytes as u64);
    Endpoint::new(
        compio::net::UdpSocket::from_std(socket)?,
        quic::EndpointConfig::default(),
        Some(server),
        None,
    )
}

pub async fn serve<H: Handler>(
    endpoint: Endpoint,
    handler: Rc<H>,
    limits: Rc<Limits>,
    shutdown: Shutdown,
) -> io::Result<()> {
    let mut connections = FuturesUnordered::new();
    loop {
        let incoming = endpoint.wait_incoming().fuse();
        let done = if connections.is_empty() {
            Either::Left(pending())
        } else {
            Either::Right(connections.next())
        }
        .fuse();
        let stop = shutdown.cancelled().fuse();
        futures_util::pin_mut!(incoming, done, stop);
        futures_util::select_biased! {
            _ = stop => break,
            _ = done => {},
            incoming = incoming => {
                let Some(incoming) = incoming else { break; };
                if connections.len() >= limits.protocol_capacity(true, true) { incoming.refuse(); continue; }
                if !incoming.remote_address_validated() && incoming.may_retry() { let _ = incoming.retry(); continue; }
                let handler = handler.clone(); let limits = limits.clone(); let shutdown = shutdown.clone();
                connections.push(async move {
                    if let Ok(Ok(connection)) = compio::time::timeout(limits.header_timeout, async { incoming.await }).await {
                        let _ = serve_connection(connection, handler, limits, shutdown).await;
                    }
                });
            },
        }
    }
    endpoint.set_server_config(None);
    let _ = compio::time::timeout(limits.drain_timeout, async {
        while connections.next().await.is_some() {}
    })
    .await;
    drop(connections);
    endpoint.close(0u32.into(), b"server shutdown");
    let _ = compio::time::timeout(limits.drain_timeout, endpoint.shutdown()).await;
    Ok(())
}

async fn serve_connection<H: Handler>(
    connection: quic::Connection,
    handler: Rc<H>,
    limits: Rc<Limits>,
    shutdown: Shutdown,
) -> io::Result<()> {
    let mut builder = h3::server::builder();
    builder.max_field_section_size(limits.header_bytes as u64);
    let mut connection =
        compio::time::timeout(limits.header_timeout, builder.build::<_, Bytes>(connection))
            .await
            .map_err(io::Error::other)?
            .map_err(io::Error::other)?;
    let mut requests = FuturesUnordered::new();
    let mut count = 0;
    loop {
        let accept = if requests.len() < limits.streams as usize {
            Either::Left(connection.accept())
        } else {
            Either::Right(pending())
        }
        .fuse();
        let done = if requests.is_empty() {
            Either::Left(pending())
        } else {
            Either::Right(requests.next())
        }
        .fuse();
        let stop = shutdown.cancelled().fuse();
        futures_util::pin_mut!(accept, done, stop);
        futures_util::select_biased! {
            _ = stop => break,
            _ = done => {},
            resolver = accept => {
                let Some(resolver) = resolver.map_err(io::Error::other)? else { break; };
                let maximum_requests = limits.requests_per_connection;
                let handler = handler.clone(); let limits = limits.clone();
                requests.push(async move {
                    let (request, stream) = compio::time::timeout(limits.header_timeout, resolver.resolve_request()).await
                        .map_err(io::Error::other)?.map_err(io::Error::other)?;
                    let (send, mut recv) = stream.split();
                    let mut send = ResponseStream { inner: send, finished: false, error: h3::error::Code::H3_INTERNAL_ERROR };
                    let remaining = match maximus_http1::content_length(request.headers()) {
                        Ok(length) => length,
                        Err(error) => {
                            recv.stop_sending(h3::error::Code::H3_MESSAGE_ERROR);
                            send.error = h3::error::Code::H3_MESSAGE_ERROR;
                            return Err(io::Error::other(error));
                        }
                    };
                    let invalid = Rc::new(Cell::new(false));
                    let body_invalid = invalid.clone();
                    let head = request.method() == http::Method::HEAD;
                    let frames = futures_util::stream::unfold(Some((recv, remaining)), move |state| {
                        let invalid = body_invalid.clone();
                        async move {
                            let (mut recv, mut remaining) = state?;
                            let result = match recv.recv_data().await {
                                Ok(Some(mut data)) if remaining.is_none_or(|left| data.remaining() as u64 <= left) => {
                                    let size = data.remaining();
                                    if let Some(left) = &mut remaining { *left -= size as u64; }
                                    return Some((Ok(maximus_core::Frame::data(data.copy_to_bytes(size))), Some((recv, remaining))));
                                },
                                Ok(None) if remaining.is_none_or(|left| left == 0) => match recv.recv_trailers().await {
                                    Ok(Some(trailers)) => return Some((Ok(maximus_core::Frame::trailers(trailers)), None)),
                                    Ok(None) => return None,
                                    Err(_) => Error::bad_request("invalid request trailers"),
                                },
                                _ => Error::bad_request("invalid request body or Content-Length"),
                            };
                            invalid.set(true);
                            recv.stop_sending(h3::error::Code::H3_MESSAGE_ERROR);
                            Some((Err(result), None))
                        }
                    });
                    let request = Request::new(request.map(|()| Body::frames(frames).limited(limits.body_bytes)));
                    let timeout = limits.request_timeout;
                    let response = maximus_http1::dispatch_request(request, handler, limits).await;
                    if invalid.get() {
                        send.error = h3::error::Code::H3_MESSAGE_ERROR;
                        return Err(io::Error::other("malformed request body"));
                    }
                    let result = write_response(&mut send.inner, response, head, timeout, &invalid).await;
                    if invalid.get() { send.error = h3::error::Code::H3_MESSAGE_ERROR; }
                    send.finished = result.is_ok();
                    result
                });
                count += 1;
                if count >= maximum_requests { break; }
            },
        }
    }
    connection.shutdown(0).await.map_err(io::Error::other)?;
    let _ = compio::time::timeout(limits.drain_timeout, async {
        while requests.next().await.is_some() {}
    })
    .await;
    Ok(())
}

// QUIC's sender finishes on drop. An unfinished HTTP response must instead reset,
// including when a deadline or server drain cancels this future.
struct ResponseStream {
    inner: h3::server::RequestStream<h3::SendStream<Bytes>, Bytes>,
    finished: bool,
    error: h3::error::Code,
}
impl Drop for ResponseStream {
    fn drop(&mut self) {
        if !self.finished {
            self.inner.stop_stream(self.error);
        }
    }
}

async fn write_response(
    send: &mut h3::server::RequestStream<h3::SendStream<Bytes>, Bytes>,
    response: maximus_core::Response,
    head: bool,
    timeout: std::time::Duration,
    invalid: &Cell<bool>,
) -> io::Result<()> {
    let (mut parts, mut body) = response.into_parts();
    for name in [
        "connection",
        "transfer-encoding",
        "keep-alive",
        "upgrade",
        "proxy-connection",
        "te",
    ] {
        parts.headers.remove(name);
    }
    // HEAD Content-Length is optional; some H3 clients validate it without tracking the method.
    if head {
        parts.headers.remove("content-length");
    }
    let no_body = head
        || parts.status == http::StatusCode::NO_CONTENT
        || parts.status == http::StatusCode::NOT_MODIFIED;
    let declared = maximus_http1::content_length(&parts.headers).map_err(io::Error::other)?;
    let mut remaining = if no_body { None } else { declared };
    compio::time::timeout(
        timeout,
        send.send_response(http::Response::from_parts(parts, ())),
    )
    .await
    .map_err(io::Error::other)?
    .map_err(io::Error::other)?;
    if !no_body {
        while let Some(frame) = body.frame().await {
            let frame = frame.map_err(io::Error::other)?;
            match frame.into_data() {
                Ok(data) => {
                    if let Some(left) = &mut remaining {
                        *left = left
                            .checked_sub(data.len() as u64)
                            .ok_or_else(|| io::Error::other("response exceeds Content-Length"))?;
                    }
                    compio::time::timeout(timeout, send.send_data(data))
                        .await
                        .map_err(io::Error::other)?
                        .map_err(io::Error::other)?;
                }
                Err(frame) => {
                    if let Ok(trailers) = frame.into_trailers() {
                        compio::time::timeout(timeout, send.send_trailers(trailers))
                            .await
                            .map_err(io::Error::other)?
                            .map_err(io::Error::other)?;
                        break;
                    }
                }
            }
        }
    }
    if invalid.get() {
        return Err(io::Error::other("malformed request body"));
    }
    if remaining.is_some_and(|left| left != 0) {
        return Err(io::Error::other("response shorter than Content-Length"));
    }
    compio::time::timeout(timeout, send.finish())
        .await
        .map_err(io::Error::other)?
        .map_err(io::Error::other)
}
