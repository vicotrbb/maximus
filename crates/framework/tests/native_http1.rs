use maximus::{Request, Server};
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

#[test]
fn ambiguous_transfer_encoding_and_length_never_reaches_handler() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| |_: Request| async { "application" })
        .unwrap();
    let mut stream = TcpStream::connect(server.local_addr()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream.write_all(b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\nGET / HTTP/1.1\r\nHost: a\r\n\r\n").unwrap();
    let mut wire = String::new();
    stream.read_to_string(&mut wire).unwrap();
    assert!(wire.starts_with("HTTP/1.1 400"), "{wire}");
    assert!(!wire.contains("application"), "{wire}");
    server.shutdown().unwrap();
}

fn connect(address: std::net::SocketAddr) -> TcpStream {
    let stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
}
fn read_all(mut stream: TcpStream) -> String {
    let mut wire = String::new();
    stream.read_to_string(&mut wire).unwrap();
    wire
}

#[test]
fn pipelined_header_metadata_survives_fragmentation_and_buffer_reuse() {
    for capacity in [1024, 16384] {
        let server = Server::new("127.0.0.1:0".parse().unwrap())
            .limits(maximus::Limits {
                io_buffer_bytes: capacity,
                ..Default::default()
            })
            .start(|| {
                |req: Request| async move {
                    format!(
                        "{}:{}",
                        req.uri(),
                        req.headers()["x-probe"].to_str().unwrap()
                    )
                }
            })
            .unwrap();
        let first = "a".repeat(1500);
        let second = "b".repeat(1500);
        let mut stream = connect(server.local_addr());
        write!(
            stream,
            "GET /one?pick=first HTTP/1.1\r\nHost: localhost\r\nX-Probe: {first}\r\n\r\nGET /two?pick=second HTTP/1.1\r\nHost: localhost\r\nX-Probe: {second}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let wire = read_all(stream);
        assert_eq!(wire.matches("HTTP/1.1 200 OK").count(), 2, "{capacity}");
        assert!(wire.contains(&format!("/one?pick=first:{first}HTTP/1.1 200 OK")));
        assert!(wire.ends_with(&format!("/two?pick=second:{second}")));
        server.shutdown().unwrap();
    }
}

#[test]
fn fragmented_chunked_request_trailers_and_ordered_pipeline() {
    use http_body_util::BodyExt;
    use maximus::IntoResponse;
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| {
            |req: Request| async move {
                match req.into_body().collect().await {
                    Ok(body) => {
                        let trailer = body
                            .trailers()
                            .and_then(|t| t.get("x-check"))
                            .map(|v| v.to_str().unwrap().to_owned())
                            .unwrap_or_default();
                        format!("{}:{trailer}", String::from_utf8_lossy(&body.to_bytes()))
                            .into_response()
                    }
                    Err(e) => e.into_response(),
                }
            }
        })
        .unwrap();
    let mut stream = connect(server.local_addr());
    for fragment in [
        b"POST / HT".as_slice(),
        b"TP/1.1\r\nHost: a\r\nTransfer-Encoding: chunked\r\n\r",
        b"\n3;name=\"semi;colon\"\r",
        b"\nabc\r",
        b"\n2\r\nde\r\n0\r\nX-Check: yes\r\n\r",
        b"\nGET / HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n",
    ] {
        stream.write_all(fragment).unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    let wire = read_all(stream);
    assert_eq!(wire.matches("200 OK").count(), 2, "{wire}");
    assert!(wire.contains("abcde:yesHTTP/1.1 200"), "{wire}");
    server.shutdown().unwrap();
}

#[test]
fn consumed_request_framing_preserves_body_trailers_and_pipeline_order() {
    use http_body_util::BodyExt;
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| {
            |req: Request| async move {
                if req.uri().path() == "/next" {
                    return "next".to_owned();
                }
                let body = req.into_body().collect().await.unwrap();
                let trailer = body
                    .trailers()
                    .and_then(|trailers| trailers.get("x-check"))
                    .map(|value| value.to_str().unwrap().to_owned())
                    .unwrap_or_else(|| "none".to_owned());
                format!("{}:{trailer}", String::from_utf8_lossy(&body.to_bytes()))
            }
        })
        .unwrap();
    for (name, framing, body, expected) in [
        ("no framing", "", "", ":none"),
        ("zero length", "Content-Length: 0\r\n", "", ":none"),
        ("fixed body", "Content-Length: 5\r\n", "hello", "hello:none"),
        (
            "empty chunked body",
            "Transfer-Encoding: chunked\r\n",
            "0\r\nX-Check: yes\r\n\r\n",
            ":yes",
        ),
        (
            "nonempty chunked body",
            "Transfer-Encoding: chunked\r\n",
            "5\r\nhello\r\n0\r\nX-Check: yes\r\n\r\n",
            "hello:yes",
        ),
    ] {
        let mut stream = connect(server.local_addr());
        stream
            .write_all(
                format!("POST /echo HTTP/1.1\r\nHost: a\r\n{framing}\r\n{body}GET /next HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .unwrap();
        let wire = read_all(stream);
        assert!(wire.starts_with("HTTP/1.1 200 OK\r\n"), "{name}: {wire}");
        assert_eq!(wire.matches("HTTP/1.1 ").count(), 2, "{name}: {wire}");
        let (_, payload) = wire.split_once("\r\n\r\n").unwrap();
        let next = payload
            .strip_prefix(expected)
            .unwrap_or_else(|| panic!("{name}: {wire}"));
        let (headers, body) = next.split_once("\r\n\r\n").unwrap();
        assert!(headers.starts_with("HTTP/1.1 200 OK\r\n"), "{name}: {wire}");
        assert_eq!(body, "next", "{name}: {wire}");
    }
    server.shutdown().unwrap();
}

#[test]
fn expect_continue_and_large_demand_driven_echo() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| |req: Request| async move { req.into_body() })
        .unwrap();
    let mut stream = connect(server.local_addr());
    stream.write_all(b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 50000\r\nExpect: 100-continue\r\nConnection: close\r\n\r\n").unwrap();
    let mut interim = [0; 25];
    stream.read_exact(&mut interim).unwrap();
    assert_eq!(&interim, b"HTTP/1.1 100 Continue\r\n\r\n");
    stream.write_all(&vec![b'x'; 50000]).unwrap();
    let wire = read_all(stream);
    assert!(wire.starts_with("HTTP/1.1 200"), "{wire}");
    assert_eq!(wire.bytes().filter(|b| *b == b'x').count(), 50000);
    assert!(wire.ends_with("\r\n0\r\n\r\n"));
    server.shutdown().unwrap();
}

#[test]
fn bodyless_statuses_and_http10_close_delimited_stream() {
    use maximus::{Body, IntoResponse, StatusCode};
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| {
            |req: Request| async move {
                if req.uri().path() == "/205" {
                    (StatusCode::RESET_CONTENT, "forbidden").into_response()
                } else {
                    Body::stream(futures_util::stream::iter([Ok("a".into()), Ok("b".into())]))
                        .into_response()
                }
            }
        })
        .unwrap();
    let mut stream = connect(server.local_addr());
    stream
        .write_all(b"GET /205 HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n")
        .unwrap();
    let wire = read_all(stream);
    assert!(wire.ends_with("\r\n\r\n"), "{wire}");
    let mut stream = connect(server.local_addr());
    stream
        .write_all(b"GET / HTTP/1.0\r\nConnection: keep-alive\r\n\r\n")
        .unwrap();
    let wire = read_all(stream);
    assert!(wire.starts_with("HTTP/1.0 200"), "{wire}");
    assert!(wire.ends_with("ab"), "{wire}");
    assert!(wire.contains("connection: close"), "{wire}");
    assert!(!wire.contains("transfer-encoding"), "{wire}");
    server.shutdown().unwrap();
}

#[test]
fn bare_newline_and_invalid_extensions_are_rejected() {
    use http_body_util::BodyExt;
    use maximus::IntoResponse;
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| {
            |req: Request| async move {
                match req.into_body().collect().await {
                    Ok(_) => "application".into_response(),
                    Err(e) => e.into_response(),
                }
            }
        })
        .unwrap();
    for request in [
        b"GET / HTTP/1.1\r\nHost: a\r\n\nConnection: close\r\n\r\n".as_slice(),
        b"POST / HTTP/1.1\r\nHost: a\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n1;foo=bar\x0b\r\nx\r\n0\r\n\r\n",
        b"POST / HTTP/1.1\r\nHost: a\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n1;=bad\r\nx\r\n0\r\n\r\n",
    ] {
        let mut stream=connect(server.local_addr());stream.write_all(request).unwrap();
        let wire=read_all(stream);assert!(wire.contains("400 Bad Request"),"{wire}");assert!(!wire.contains("application"));
    }
    server.shutdown().unwrap();
}

#[test]
fn malformed_and_truncated_bodies_close_without_reusing_pipeline_bytes() {
    use http_body_util::BodyExt;
    use maximus::IntoResponse;
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| {
            |req: Request| async move {
                match req.into_body().collect().await {
                    Ok(_) => "application".into_response(),
                    Err(e) => e.into_response(),
                }
            }
        })
        .unwrap();
    for body in [
        "3\r\nab",
        "3\r\nabc",
        "3\r\nabc\r",
        "3\r\nabcX\r\n",
        "0\r\nX: value",
        "0\r\nHost: evil\r\n\r\n",
        "FFFFFFFFFFFFFFFFFFFFFFFF\r\n",
    ] {
        let mut stream = connect(server.local_addr());
        stream
            .write_all(
                format!("POST / HTTP/1.1\r\nHost: a\r\nTransfer-Encoding: chunked\r\n\r\n{body}")
                    .as_bytes(),
            )
            .unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        let wire = read_all(stream);
        assert!(wire.contains("400 Bad Request"), "{body:?}: {wire}");
        assert!(!wire.contains("application"));
    }
    let mut stream = connect(server.local_addr());
    stream
        .write_all(b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 5\r\n\r\n123")
        .unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    assert!(read_all(stream).contains("400 Bad Request"));
    server.shutdown().unwrap();
}

#[test]
fn failed_response_stream_is_not_terminated_as_success() {
    use maximus::{Body, Error};
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| {
            |_: Request| async move {
                Body::stream(futures_util::stream::iter([
                    Ok("before".into()),
                    Err(Error::bad_request("stream failed")),
                ]))
            }
        })
        .unwrap();
    let mut stream = connect(server.local_addr());
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n")
        .unwrap();
    let wire = read_all(stream);
    assert!(wire.contains("6\r\nbefore\r\n"), "{wire}");
    assert!(!wire.ends_with("0\r\n\r\n"), "{wire}");
    server.shutdown().unwrap();
}

#[test]
fn unread_request_body_is_closed_and_never_parsed_as_next_request() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| |_: Request| async { "ok" })
        .unwrap();
    let mut stream = connect(server.local_addr());
    stream.write_all(b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 50\r\n\r\nGET / HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n").unwrap();
    let wire = read_all(stream);
    assert_eq!(wire.matches("200 OK").count(), 1, "{wire}");
    assert!(wire.ends_with("ok"));
    server.shutdown().unwrap();
}

#[test]
fn ignored_empty_chunked_body_closes_before_dispatching_pipeline() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let followups = Arc::new(AtomicUsize::new(0));
    let calls = followups.clone();
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(move || {
            let calls = calls.clone();
            move |req: Request| {
                let calls = calls.clone();
                async move {
                    if req.uri().path() == "/next" {
                        calls.fetch_add(1, Ordering::Relaxed);
                        "unexpected next"
                    } else {
                        "ignored"
                    }
                }
            }
        })
        .unwrap();
    let mut stream = connect(server.local_addr());
    stream.write_all(b"POST / HTTP/1.1\r\nHost: a\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nX-Check: yes\r\n\r\nGET /next HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n").unwrap();
    let wire = read_all(stream);
    server.shutdown().unwrap();
    assert!(wire.starts_with("HTTP/1.1 200 OK\r\n"), "{wire}");
    assert_eq!(wire.matches("HTTP/1.1 ").count(), 1, "{wire}");
    assert_eq!(wire.split_once("\r\n\r\n").unwrap().1, "ignored", "{wire}");
    assert_eq!(followups.load(Ordering::Relaxed), 0, "{wire}");
}

#[test]
fn stalled_request_body_times_out_and_releases_admission() {
    use http_body_util::BodyExt;
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .limits(maximus::Limits {
            connections: 1,
            request_timeout: Duration::from_millis(40),
            ..Default::default()
        })
        .start(|| {
            |req: Request| async move {
                let _ = req.into_body().collect().await;
                "ok"
            }
        })
        .unwrap();
    let mut stream = connect(server.local_addr());
    stream
        .write_all(b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 5\r\n\r\n1")
        .unwrap();
    assert!(read_all(stream).contains("408 Request Timeout"));
    let mut next = connect(server.local_addr());
    next.write_all(b"GET / HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n")
        .unwrap();
    assert!(read_all(next).ends_with("ok"));
    server.shutdown().unwrap();
}

#[test]
fn response_lengths_and_trailers_have_unambiguous_end_markers() {
    use maximus::{Body, Frame, http};
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| {
            |req: Request| async move {
                if req.uri().path() == "/trailers" {
                    let mut trailers = http::HeaderMap::new();
                    trailers.insert("x-check", "yes".parse().unwrap());
                    http::Response::new(Body::frames(futures_util::stream::iter([
                        Ok(Frame::data("ok".into())),
                        Ok(Frame::trailers(trailers)),
                    ])))
                } else {
                    http::Response::builder()
                        .header(
                            "content-length",
                            if req.uri().path() == "/short" {
                                "5"
                            } else {
                                "1"
                            },
                        )
                        .body(Body::from("abc"))
                        .unwrap()
                }
            }
        })
        .unwrap();
    for (path, length) in [("/short", "5"), ("/long", "1")] {
        let mut stream = connect(server.local_addr());
        stream.write_all(format!("GET {path} HTTP/1.1\r\nHost: a\r\n\r\nGET /trailers HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n").as_bytes()).unwrap();
        let wire = read_all(stream);
        assert_eq!(wire.matches("200 OK").count(), 1, "{wire}");
        assert!(wire.contains(&format!("content-length: {length}")));
        assert!(!wire.contains("x-check"));
    }
    let mut stream = connect(server.local_addr());
    stream
        .write_all(b"GET /trailers HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n")
        .unwrap();
    let wire = read_all(stream);
    assert!(
        wire.ends_with("2\r\nok\r\n0\r\nx-check: yes\r\n\r\n"),
        "{wire}"
    );
    server.shutdown().unwrap();
}

#[test]
fn request_target_forms_preserve_method_and_uri() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| |req: Request| async move { format!("{} {}", req.method(), req.uri()) })
        .unwrap();
    for (request, expected) in [
        (
            "OPTIONS * HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n",
            "OPTIONS *",
        ),
        (
            "GET http://a/path?q=1 HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n",
            "GET http://a/path?q=1",
        ),
    ] {
        let mut stream = connect(server.local_addr());
        stream.write_all(request.as_bytes()).unwrap();
        let wire = read_all(stream);
        assert!(wire.ends_with(expected), "{wire}");
    }
    server.shutdown().unwrap();
}

#[test]
fn invalid_target_forms_do_not_reach_handler() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| |_: Request| async { "application" })
        .unwrap();
    for target in ["GET /path#fragment", "CONNECT /path", "GET *"] {
        let mut stream = connect(server.local_addr());
        stream
            .write_all(
                format!("{target} HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n").as_bytes(),
            )
            .unwrap();
        let wire = read_all(stream);
        assert!(wire.contains("400 Bad Request"), "{target}: {wire}");
        assert!(!wire.contains("application"));
    }
    server.shutdown().unwrap();
}

#[test]
fn unsupported_tunnel_and_informational_final_responses_are_refused() {
    use maximus::{IntoResponse, StatusCode};
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| {
            |req: Request| async move {
                if req.method() == maximus::Method::CONNECT {
                    "not a tunnel".into_response()
                } else {
                    StatusCode::CONTINUE.into_response()
                }
            }
        })
        .unwrap();
    for (request, status) in [
        ("CONNECT example.com:443", "501 Not Implemented"),
        ("GET /", "500 Internal Server Error"),
    ] {
        let mut stream = connect(server.local_addr());
        stream
            .write_all(
                format!("{request} HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n").as_bytes(),
            )
            .unwrap();
        let wire = read_all(stream);
        assert!(wire.starts_with(&format!("HTTP/1.1 {status}")), "{wire}");
    }
    server.shutdown().unwrap();
}

#[test]
fn rejected_connect_never_dispatches_optimistic_tunnel_bytes() {
    use maximus::{IntoResponse, StatusCode};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    for status in [
        StatusCode::OK,
        StatusCode::FOUND,
        StatusCode::FORBIDDEN,
        StatusCode::BAD_GATEWAY,
    ] {
        let followups = Arc::new(AtomicUsize::new(0));
        let calls = followups.clone();
        let server = Server::new("127.0.0.1:0".parse().unwrap())
            .start(move || {
                let calls = calls.clone();
                move |req: Request| {
                    let calls = calls.clone();
                    async move {
                        if req.method() == maximus::Method::CONNECT {
                            status.into_response()
                        } else {
                            calls.fetch_add(1, Ordering::Relaxed);
                            "must not dispatch".into_response()
                        }
                    }
                }
            })
            .unwrap();
        let mut stream = connect(server.local_addr());
        stream.write_all(b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\nGET / HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n").unwrap();
        let wire = read_all(stream);
        server.shutdown().unwrap();
        let expected = if status.is_success() {
            StatusCode::NOT_IMPLEMENTED
        } else {
            status
        };
        assert!(
            wire.starts_with(&format!("HTTP/1.1 {}", expected.as_u16())),
            "{wire}"
        );
        assert_eq!(followups.load(Ordering::Relaxed), 0, "{status}: {wire}");
        assert_eq!(wire.matches("HTTP/1.1 ").count(), 1, "{wire}");
        assert!(wire.contains("connection: close\r\n"), "{wire}");
    }
}

#[test]
fn buffered_batching_preserves_body_limit_and_large_payload() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| {
            |req: Request| async move {
                if req.uri().path() == "/limited" {
                    maximus::Body::from("forbidden").limited(2)
                } else {
                    maximus::Body::from(vec![b'x'; 100_000])
                }
            }
        })
        .unwrap();
    let mut stream = connect(server.local_addr());
    stream
        .write_all(b"GET /limited HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n")
        .unwrap();
    let wire = read_all(stream);
    assert!(wire.starts_with("HTTP/1.1 200"));
    assert!(!wire.contains("forbidden"));
    let mut stream = connect(server.local_addr());
    stream
        .write_all(b"GET /large HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n")
        .unwrap();
    let wire = read_all(stream);
    let (headers, body) = wire.split_once("\r\n\r\n").unwrap();
    assert!(headers.contains("content-length: 100000"));
    assert_eq!(body.len(), 100_000);
    assert!(body.bytes().all(|b| b == b'x'));
    server.shutdown().unwrap();
}

#[test]
fn application_date_header_is_preserved_after_generated_date() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| {
            |req: Request| async move {
                let mut response = maximus::http::Response::new(maximus::Body::from("ok"));
                if req.uri().path() == "/explicit" {
                    response
                        .headers_mut()
                        .insert("date", "Thu, 01 Jan 1970 00:00:00 GMT".parse().unwrap());
                }
                response
            }
        })
        .unwrap();
    let mut stream = connect(server.local_addr());
    stream.write_all(b"GET /generated HTTP/1.1\r\nHost: a\r\n\r\nGET /explicit HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n").unwrap();
    let wire = read_all(stream);
    let responses: Vec<_> = wire.split("HTTP/1.1 200 OK").skip(1).collect();
    assert_eq!(responses.len(), 2, "{wire}");
    assert_eq!(responses[0].matches("date: ").count(), 1);
    assert_eq!(responses[1].matches("date: ").count(), 1);
    assert!(
        responses[1].contains("date: Thu, 01 Jan 1970 00:00:00 GMT\r\n"),
        "{wire}"
    );
    server.shutdown().unwrap();
}
