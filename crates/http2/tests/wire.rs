use maximus_core::{Limits, Request};
use std::{
    io::{Read, Write},
    net::TcpStream,
    rc::Rc,
    time::Duration,
};
fn frame(kind: u8, flags: u8, id: u32, data: &[u8]) -> Vec<u8> {
    let mut v = vec![
        (data.len() >> 16) as u8,
        (data.len() >> 8) as u8,
        data.len() as u8,
        kind,
        flags,
    ];
    v.extend(id.to_be_bytes());
    v.extend(data);
    v
}
fn receive(s: &mut TcpStream) -> (u8, u8, u32, Vec<u8>) {
    let mut h = [0; 9];
    s.read_exact(&mut h).unwrap();
    let n = ((h[0] as usize) << 16) | ((h[1] as usize) << 8) | h[2] as usize;
    let mut b = vec![0; n];
    s.read_exact(&mut b).unwrap();
    (
        h[3],
        h[4],
        u32::from_be_bytes(h[5..].try_into().unwrap()),
        b,
    )
}
fn server() -> TcpStream {
    server_with(|| |_: Request| async { "hello" }, Limits::default())
}
fn server_with<H: maximus_core::Handler>(
    make: impl FnOnce() -> H + Send + 'static,
    limits: Limits,
) -> TcpStream {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        compio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let listener = compio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                tx.send(listener.local_addr().unwrap()).unwrap();
                let (stream, _) = listener.accept().await.unwrap();
                let _ = maximus_http2::serve(
                    stream,
                    Rc::new(make()),
                    Rc::new(limits),
                    Default::default(),
                )
                .await;
            });
    });
    let stream = TcpStream::connect(rx.recv().unwrap()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
}
fn handshake(s: &mut TcpStream) {
    s.write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();
    s.write_all(&frame(4, 0, 0, &[])).unwrap();
    loop {
        let (k, f, _, _) = receive(s);
        if k == 4 && f == 0 {
            s.write_all(&frame(4, 1, 0, &[])).unwrap();
        }
        if k == 4 && f == 1 {
            break;
        }
    }
}
fn request() -> Vec<u8> {
    let mut b = vec![0x82, 0x86, 0x84, 0x01, 9];
    b.extend(b"localhost");
    b
}
#[test]
fn native_preface_and_two_streams() {
    let mut s = server();
    handshake(&mut s);
    for id in [1, 3] {
        s.write_all(&frame(1, 5, id, &request())).unwrap();
    }
    let mut done = vec![];
    while done.len() < 2 {
        let (k, f, id, b) = receive(&mut s);
        if k == 0 && !b.is_empty() {
            assert_eq!(b, b"hello");
        }
        if (k == 0 || k == 1) && f & 1 != 0 {
            done.push(id)
        }
    }
    done.sort();
    assert_eq!(done, [1, 3]);
}
#[test]
fn first_frame_must_be_settings() {
    let mut s = server();
    s.write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();
    s.write_all(&frame(6, 0, 0, b"12345678")).unwrap();
    loop {
        let (k, _, _, b) = receive(&mut s);
        if k == 7 {
            assert_eq!(&b[4..8], &1u32.to_be_bytes());
            break;
        }
    }
}

fn until(s: &mut TcpStream, kind: u8, id: u32) -> (u8, Vec<u8>) {
    loop {
        let (k, f, i, p) = receive(s);
        if k == kind && i == id {
            return (f, p);
        }
    }
}
#[test]
fn continuation_cannot_be_interleaved() {
    let mut s = server();
    handshake(&mut s);
    s.write_all(&frame(1, 1, 1, &request()[..2])).unwrap();
    s.write_all(&frame(6, 0, 0, b"12345678")).unwrap();
    assert_eq!(&until(&mut s, 7, 0).1[4..8], &1u32.to_be_bytes());
}
#[test]
fn malformed_pseudo_headers_reset_only_one_stream() {
    let mut s = server();
    handshake(&mut s);
    let mut bad = request();
    bad.push(0x82);
    s.write_all(&frame(1, 5, 1, &bad)).unwrap();
    s.write_all(&frame(1, 5, 3, &request())).unwrap();
    assert_eq!(until(&mut s, 3, 1).1, 1u32.to_be_bytes());
    assert_eq!(until(&mut s, 0, 3).1, b"hello");
}
#[test]
fn body_ending_at_exact_send_window_completes_without_more_credit() {
    let mut s = server_with(
        || |_: Request| async { "x".repeat(65535) },
        Limits::default(),
    );
    handshake(&mut s);
    s.write_all(&frame(1, 5, 1, &request())).unwrap();
    let mut count = 0;
    loop {
        let (k, f, id, p) = receive(&mut s);
        if k == 0 && id == 1 {
            count += p.len();
            if f & 1 != 0 {
                assert!(!p.is_empty(), "final payload must carry END_STREAM");
                break;
            }
        }
    }
    assert_eq!(count, 65535);
}
#[test]
fn buffered_response_ends_on_last_payload_after_frame_splitting() {
    for size in [5, 16385] {
        let mut s = server_with(
            move || move |_: Request| async move { "x".repeat(size) },
            Limits::default(),
        );
        handshake(&mut s);
        s.write_all(&frame(1, 5, 1, &request())).unwrap();
        let mut count = 0;
        loop {
            let (kind, flags, id, payload) = receive(&mut s);
            if kind == 0 && id == 1 {
                assert!(!payload.is_empty(), "final payload must carry END_STREAM");
                assert!(payload.iter().all(|&b| b == b'x'));
                count += payload.len();
                assert_eq!(flags & 1 != 0, count == size);
                if flags & 1 != 0 {
                    break;
                }
            }
        }
        assert_eq!(count, size);
    }
}
#[test]
fn zero_stream_window_does_not_block_headers_or_other_streams() {
    let mut s = server();
    handshake(&mut s);
    let mut setting = 4u16.to_be_bytes().to_vec();
    setting.extend(0u32.to_be_bytes());
    s.write_all(&frame(4, 0, 0, &setting)).unwrap();
    until(&mut s, 4, 0);
    s.write_all(&frame(1, 5, 1, &request())).unwrap();
    until(&mut s, 1, 1);
    s.write_all(&frame(1, 5, 3, &request())).unwrap();
    until(&mut s, 1, 3);
    s.write_all(&frame(8, 0, 3, &5u32.to_be_bytes())).unwrap();
    let mut payload = vec![];
    loop {
        let (k, f, id, p) = receive(&mut s);
        assert_ne!((k, id), (0, 1));
        if k == 0 && id == 3 {
            payload.extend(p);
            if f & 1 != 0 {
                break;
            }
        }
    }
    assert_eq!(payload, b"hello");
}
#[test]
fn receive_credit_tracks_consumption_and_padding() {
    use http_body_util::BodyExt;
    let limits = Limits {
        stream_window_bytes: 16,
        ..Limits::default()
    };
    let mut s = server_with(
        || {
            |r: Request| async move {
                let bytes = r.into_body().collect().await.unwrap().to_bytes();
                String::from_utf8(bytes.to_vec()).unwrap()
            }
        },
        limits,
    );
    handshake(&mut s);
    s.write_all(&frame(1, 4, 1, &request())).unwrap();
    s.write_all(&frame(0, 8, 1, b"\x03abcdefghijkl\0\0\0"))
        .unwrap();
    let mut credit = 0;
    while credit < 16 {
        let (_, p) = until(&mut s, 8, 1);
        credit += u32::from_be_bytes(p.try_into().unwrap());
    }
    s.write_all(&frame(0, 1, 1, b"mnop")).unwrap();
    assert_eq!(until(&mut s, 0, 1).1, b"abcdefghijklmnop");
}
#[test]
fn fragmented_headers_and_trailers_reach_handler() {
    use http_body_util::BodyExt;
    let mut s = server_with(
        || {
            |r: Request| async move {
                let body = r.into_body().collect().await.unwrap();
                assert_eq!(body.trailers().unwrap()["x-end"], "yes");
                body.to_bytes().len().to_string()
            }
        },
        Limits::default(),
    );
    handshake(&mut s);
    let b = request();
    s.write_all(&frame(1, 0, 1, &b[..2])).unwrap();
    s.write_all(&frame(9, 4, 1, &b[2..])).unwrap();
    s.write_all(&frame(0, 0, 1, b"abc")).unwrap();
    s.write_all(&frame(1, 5, 1, b"\x00\x05x-end\x03yes"))
        .unwrap();
    assert_eq!(until(&mut s, 0, 1).1, b"3");
}
#[test]
fn reset_cancels_handler_and_preserves_connection() {
    let mut s = server_with(
        || {
            |r: Request| async move {
                if r.uri().path() == "/" {
                    futures_util::future::pending::<()>().await;
                }
                "hello"
            }
        },
        Limits::default(),
    );
    handshake(&mut s);
    s.write_all(&frame(1, 5, 1, &request())).unwrap();
    s.write_all(&frame(3, 0, 1, &8u32.to_be_bytes())).unwrap();
    let mut b = vec![0x82, 0x86, 0x04, 2, b'/', b'x', 0x01, 9];
    b.extend(b"localhost");
    s.write_all(&frame(1, 5, 3, &b)).unwrap();
    assert_eq!(until(&mut s, 0, 3).1, b"hello");
}
#[test]
fn invalid_response_length_resets_stream() {
    use maximus_core::{HeaderValue, IntoResponse};
    let mut s = server_with(
        || {
            |_: Request| async {
                let mut r = "hello".into_response();
                r.headers_mut()
                    .insert("content-length", HeaderValue::from_static("bogus"));
                r
            }
        },
        Limits::default(),
    );
    handshake(&mut s);
    s.write_all(&frame(1, 5, 1, &request())).unwrap();
    assert_eq!(until(&mut s, 3, 1).1, 2u32.to_be_bytes());
}
#[test]
fn mismatched_buffered_response_length_never_completes_successfully() {
    use maximus_core::{HeaderValue, IntoResponse};
    for length in ["4", "6"] {
        let mut s = server_with(
            move || {
                move |_: Request| async move {
                    let mut response = "hello".into_response();
                    response
                        .headers_mut()
                        .insert("content-length", HeaderValue::from_static(length));
                    response
                }
            },
            Limits::default(),
        );
        handshake(&mut s);
        s.write_all(&frame(1, 5, 1, &request())).unwrap();
        loop {
            let (kind, flags, id, payload) = receive(&mut s);
            if id == 1 {
                if kind == 3 {
                    assert_eq!(payload, 2u32.to_be_bytes());
                    break;
                }
                assert_eq!(flags & 1, 0, "mismatched length must not END_STREAM");
            }
        }
    }
}
#[test]
fn streaming_data_keeps_stream_open_for_response_trailers() {
    use maximus_core::{Body, Bytes, Frame, HeaderMap, HeaderValue};
    let mut s = server_with(
        || {
            |_: Request| async {
                let mut trailers = HeaderMap::new();
                trailers.insert("x-end", HeaderValue::from_static("yes"));
                Body::frames(futures_util::stream::iter([
                    Ok(Frame::data(Bytes::from_static(b"hello"))),
                    Ok(Frame::trailers(trailers)),
                ]))
            }
        },
        Limits::default(),
    );
    handshake(&mut s);
    s.write_all(&frame(1, 5, 1, &request())).unwrap();
    let (flags, payload) = until(&mut s, 0, 1);
    assert_eq!(flags & 1, 0);
    assert_eq!(payload, b"hello");
    assert_eq!(until(&mut s, 1, 1).0, 5);
}
#[test]
fn forbidden_response_trailers_reset_stream() {
    use maximus_core::{Body, Frame, HeaderMap, HeaderValue};
    let mut s = server_with(
        || {
            |_: Request| async {
                let mut trailers = HeaderMap::new();
                trailers.insert("content-length", HeaderValue::from_static("0"));
                Body::frames(futures_util::stream::iter([Ok(Frame::trailers(trailers))]))
            }
        },
        Limits::default(),
    );
    handshake(&mut s);
    s.write_all(&frame(1, 5, 1, &request())).unwrap();
    assert_eq!(until(&mut s, 3, 1).1, 2u32.to_be_bytes());
}
#[test]
fn refused_headers_still_update_hpack_dynamic_table() {
    let limits = Limits {
        streams: 1,
        ..Limits::default()
    };
    let mut s = server_with(
        || {
            |r: Request| async move {
                if r.uri().path() == "/wait" {
                    futures_util::future::pending::<()>().await;
                }
                assert_eq!(r.headers()["x-test"], "yes");
                "hello"
            }
        },
        limits,
    );
    handshake(&mut s);
    let mut waiting = vec![0x82, 0x86, 0x04, 5];
    waiting.extend(b"/wait");
    waiting.extend([0x01, 9]);
    waiting.extend(b"localhost");
    s.write_all(&frame(1, 5, 1, &waiting)).unwrap();
    let mut refused = request();
    refused.extend(b"\x40\x06x-test\x03yes");
    s.write_all(&frame(1, 5, 3, &refused)).unwrap();
    assert_eq!(until(&mut s, 3, 3).1, 7u32.to_be_bytes());
    s.write_all(&frame(3, 0, 1, &8u32.to_be_bytes())).unwrap();
    let mut next = request();
    next.push(0xbe);
    s.write_all(&frame(1, 5, 5, &next)).unwrap();
    assert_eq!(until(&mut s, 0, 5).1, b"hello");
}
#[test]
fn settings_decrease_can_make_send_window_negative() {
    let mut s = server_with(|| |_: Request| async { "x".repeat(100) }, Limits::default());
    handshake(&mut s);
    let setting = |n: u32| {
        let mut b = 4u16.to_be_bytes().to_vec();
        b.extend(n.to_be_bytes());
        b
    };
    s.write_all(&frame(4, 0, 0, &setting(5))).unwrap();
    until(&mut s, 4, 0);
    s.write_all(&frame(1, 5, 1, &request())).unwrap();
    assert_eq!(until(&mut s, 0, 1).1.len(), 5);
    s.write_all(&frame(4, 0, 0, &setting(0))).unwrap();
    until(&mut s, 4, 0);
    s.write_all(&frame(8, 0, 1, &5u32.to_be_bytes())).unwrap();
    s.write_all(&frame(6, 0, 0, b"barrier!")).unwrap();
    loop {
        let (k, _, _, _) = receive(&mut s);
        assert_ne!(k, 0);
        if k == 6 {
            break;
        }
    }
    s.write_all(&frame(8, 0, 1, &3u32.to_be_bytes())).unwrap();
    assert_eq!(until(&mut s, 0, 1).1.len(), 3);
}
#[test]
fn unread_request_does_not_receive_credit_and_stream_overflow_is_scoped() {
    let limits = Limits {
        stream_window_bytes: 16,
        ..Limits::default()
    };
    let mut s = server_with(
        || {
            |r: Request| async move {
                if r.uri().path() == "/" {
                    futures_util::future::pending::<()>().await;
                }
                "hello"
            }
        },
        limits,
    );
    handshake(&mut s);
    s.write_all(&frame(1, 4, 1, &request())).unwrap();
    s.write_all(&frame(0, 0, 1, b"12345678901234567")).unwrap();
    assert_eq!(until(&mut s, 3, 1).1, 3u32.to_be_bytes());
    s.write_all(&frame(6, 0, 0, b"alive!!!")).unwrap();
    assert_eq!(until(&mut s, 6, 0).1, b"alive!!!");
}
#[test]
fn incomplete_body_length_is_rejected() {
    use http_body_util::BodyExt;
    let mut s = server_with(
        || {
            |r: Request| async move {
                let _ = r.into_body().collect().await;
                "hello"
            }
        },
        Limits::default(),
    );
    handshake(&mut s);
    let mut b = request();
    b.extend(b"\x0f\x0d\x01");
    b.push(b'4');
    s.write_all(&frame(1, 4, 1, &b)).unwrap();
    s.write_all(&frame(0, 1, 1, b"abc")).unwrap();
    assert_eq!(until(&mut s, 3, 1).1, 1u32.to_be_bytes());
}
#[test]
fn empty_response_chunks_do_not_stall_scheduler() {
    use maximus_core::{Body, Bytes};
    let mut s = server_with(
        || {
            |_: Request| async {
                Body::stream(futures_util::stream::iter([
                    Ok(Bytes::new()),
                    Ok(Bytes::from_static(b"hello")),
                ]))
            }
        },
        Limits::default(),
    );
    handshake(&mut s);
    s.write_all(&frame(1, 5, 1, &request())).unwrap();
    assert_eq!(until(&mut s, 0, 1).1, b"hello");
}
#[test]
fn handler_deadline_produces_a_response() {
    let limits = Limits {
        request_timeout: Duration::from_millis(20),
        ..Limits::default()
    };
    let mut s = server_with(
        || {
            |_: Request| async {
                futures_util::future::pending::<()>().await;
                "hello"
            }
        },
        limits,
    );
    handshake(&mut s);
    s.write_all(&frame(1, 5, 1, &request())).unwrap();
    let (kind, _, id, _) = receive(&mut s);
    assert_eq!((kind, id), (1, 1));
}
#[test]
fn control_only_churn_is_bounded() {
    let mut s = server();
    handshake(&mut s);
    for _ in 0..1022 {
        s.write_all(&frame(6, 0, 0, b"12345678")).unwrap();
        until(&mut s, 6, 0);
    }
    s.write_all(&frame(6, 0, 0, b"12345678")).unwrap();
    assert_eq!(&until(&mut s, 7, 0).1[4..8], &11u32.to_be_bytes());
}
#[test]
fn shutdown_bounds_a_blocked_response_write() {
    use maximus_core::{Body, Bytes};
    let shutdown = maximus_runtime::Shutdown::default();
    let signal = shutdown.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let (done_tx, done) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        compio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let listener = compio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                tx.send(listener.local_addr().unwrap()).unwrap();
                let (stream, _) = listener.accept().await.unwrap();
                let limits = Limits {
                    drain_timeout: Duration::from_millis(40),
                    ..Limits::default()
                };
                let _ = maximus_http2::serve(
                    stream,
                    Rc::new(|_: Request| async {
                        Body::stream(futures_util::stream::repeat_with(|| {
                            Ok(Bytes::from_static(&[b'x'; 16384]))
                        }))
                    }),
                    Rc::new(limits),
                    signal,
                )
                .await;
                done_tx.send(()).unwrap();
            });
    });
    let mut s = TcpStream::connect(rx.recv().unwrap()).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    handshake(&mut s);
    let mut settings = 4u16.to_be_bytes().to_vec();
    settings.extend(0x7fff_ffffu32.to_be_bytes());
    s.write_all(&frame(4, 0, 0, &settings)).unwrap();
    until(&mut s, 4, 0);
    s.write_all(&frame(8, 0, 0, &(0x7fff_ffffu32 - 65535).to_be_bytes()))
        .unwrap();
    s.write_all(&frame(1, 5, 1, &request())).unwrap();
    std::thread::sleep(Duration::from_millis(30));
    let start = std::time::Instant::now();
    shutdown.cancel();
    done.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(start.elapsed() < Duration::from_millis(500));
}

#[test]
fn in_flight_headers_after_reset_preserve_hpack_and_other_streams() {
    let mut s = server_with(
        || {
            |r: Request| async move {
                r.headers()
                    .get("x")
                    .map_or("initial", |v| v.to_str().unwrap())
                    .to_owned()
            }
        },
        Limits::default(),
    );
    handshake(&mut s);
    s.write_all(&frame(1, 4, 1, &request())).unwrap();
    until(&mut s, 3, 1);
    // Both DATA and fragmented trailers can already be in flight at the reset.
    s.write_all(&frame(0, 8, 1, b"\x02abc\0\0")).unwrap();
    s.write_all(&frame(1, 1, 1, b"\x40\x01x")).unwrap();
    s.write_all(&frame(9, 4, 1, b"\x01y")).unwrap();
    let mut next = request();
    next.push(0xbe); // Dynamic index 62 inserted by discarded trailers.
    s.write_all(&frame(1, 5, 3, &next)).unwrap();
    let mut credit = 0;
    loop {
        let (kind, _, id, payload) = receive(&mut s);
        assert_ne!(kind, 7, "in-flight frames closed unrelated streams");
        assert_ne!(kind, 3, "closed stream must not be reset again");
        if kind == 8 && id == 0 {
            credit += u32::from_be_bytes(payload.clone().try_into().unwrap());
        }
        if kind == 0 && id == 3 {
            assert_eq!(payload, b"y");
            break;
        }
    }
    assert_eq!(
        credit, 6,
        "discarded DATA including padding restores connection credit"
    );
}

#[test]
fn first_header_payload_deadline_survives_other_stream_progress() {
    use maximus_core::{Body, Bytes};
    let mut s = server_with(
        || {
            |_: Request| async {
                Body::stream(futures_util::stream::unfold((), |()| async {
                    compio::time::sleep(Duration::from_millis(15)).await;
                    Some((Ok(Bytes::from_static(b"x")), ()))
                }))
            }
        },
        Limits {
            header_timeout: Duration::from_millis(80),
            idle_timeout: Duration::from_millis(50),
            ..Limits::default()
        },
    );
    handshake(&mut s);
    s.write_all(&frame(1, 5, 1, &request())).unwrap();
    until(&mut s, 1, 1);
    let start = std::time::Instant::now();
    s.write_all(&[0, 0, 5, 1, 5, 0, 0, 0, 3]).unwrap();
    let mut progress = 0;
    loop {
        let (kind, _, id, payload) = receive(&mut s);
        assert!(
            start.elapsed() < Duration::from_millis(300),
            "active responses bypassed the first header payload deadline"
        );
        if kind == 0 && id == 1 {
            progress += payload.len();
        }
        if kind == 7 {
            assert_eq!(&payload[4..8], &1u32.to_be_bytes());
            break;
        }
    }
    assert!(
        progress >= 2,
        "other stream must continue while the header read is pending"
    );
}

#[test]
fn normal_early_responses_use_no_error_for_success_and_rejection() {
    use maximus_core::StatusCode;
    for status in [StatusCode::OK, StatusCode::FORBIDDEN] {
        let mut s = server_with(
            move || move |_: Request| async move { (status, "answer") },
            Limits::default(),
        );
        handshake(&mut s);
        s.write_all(&frame(1, 4, 1, &request())).unwrap();
        let mut response = vec![];
        let mut ended = false;
        loop {
            let (kind, flags, id, payload) = receive(&mut s);
            if id == 1 && kind == 0 {
                response.extend_from_slice(&payload);
                ended |= flags & 1 != 0;
            }
            if id == 1 && kind == 3 {
                assert!(ended, "reset preceded the complete response");
                assert_eq!(payload, 0u32.to_be_bytes());
                break;
            }
        }
        assert_eq!(response, b"answer");
    }
}

#[test]
fn continuation_keeps_deadline_from_first_envelope() {
    let mut s = server_with(
        || |_: Request| async { "hello" },
        Limits {
            header_timeout: Duration::from_millis(300),
            idle_timeout: Duration::from_secs(5),
            ..Limits::default()
        },
    );
    handshake(&mut s);
    let start = std::time::Instant::now();
    s.write_all(&[0, 0, 2, 1, 1, 0, 0, 0, 1]).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    s.write_all(&request()[..2]).unwrap();
    s.write_all(&[0, 0, 3, 9, 4, 0, 0, 0, 1]).unwrap(); // Stalled continuation payload.
    assert_eq!(&until(&mut s, 7, 0).1[4..8], &1u32.to_be_bytes());
    assert!(
        start.elapsed() < Duration::from_millis(440),
        "continuation extended the original deadline"
    );
}

#[test]
fn idle_streams_remain_protocol_errors_after_closed_stream_processing() {
    for id in [2, 5] {
        for (kind, payload) in [
            (0, b"x".as_slice()),
            (3, &8u32.to_be_bytes()),
            (8, &1u32.to_be_bytes()),
        ] {
            let mut s = server();
            handshake(&mut s);
            s.write_all(&frame(1, 5, 3, &request())).unwrap();
            loop {
                let (k, f, i, _) = receive(&mut s);
                if matches!(k, 0 | 1) && i == 3 && f & 1 != 0 {
                    break;
                }
            }
            s.write_all(&frame(kind, 0, id, payload)).unwrap();
            assert_eq!(&until(&mut s, 7, 0).1[4..8], &1u32.to_be_bytes());
        }
    }
}

#[test]
fn response_progress_refreshes_idle_deadline() {
    use maximus_core::{Body, Bytes};
    let mut s = server_with(
        || {
            |_: Request| async {
                Body::stream(futures_util::stream::unfold(0, |i| async move {
                    if i == 12 {
                        return None;
                    }
                    compio::time::sleep(Duration::from_millis(25)).await;
                    Some((Ok(Bytes::from_static(b"x")), i + 1))
                }))
            }
        },
        Limits {
            idle_timeout: Duration::from_millis(120),
            ..Limits::default()
        },
    );
    handshake(&mut s);
    s.write_all(&frame(1, 5, 1, &request())).unwrap();
    let mut count = 0;
    loop {
        let (k, f, id, p) = receive(&mut s);
        assert_ne!(k, 7);
        if k == 0 && id == 1 {
            count += p.len();
            if f & 1 != 0 {
                break;
            }
        }
    }
    assert_eq!(count, 12);
}

#[test]
fn options_asterisk_is_preserved() {
    let mut s = server_with(
        || {
            |r: Request| async move {
                assert_eq!(r.uri().path(), "*");
                "hello"
            }
        },
        Limits::default(),
    );
    handshake(&mut s);
    s.write_all(&frame(
        1,
        5,
        1,
        b"\x02\x07OPTIONS\x86\x04\x01*\x01\x09localhost",
    ))
    .unwrap();
    assert_eq!(until(&mut s, 0, 1).1, b"hello");
}

#[test]
fn independent_hyper_client_receives_headers_and_complete_early_responses() {
    use http_body_util::BodyExt;
    use maximus_core::{Body, HeaderValue, IntoResponse, StatusCode};
    use tokio_util::compat::FuturesAsyncReadCompatExt;
    #[derive(Clone)]
    struct Executor;
    impl<F: std::future::Future<Output = ()> + 'static> hyper::rt::Executor<F> for Executor {
        fn execute(&self, future: F) {
            compio::runtime::spawn(future).detach();
        }
    }
    // Reuse the native wire server, transferring its fresh TCP connection to the client runtime.
    let stream = server_with(
        || {
            |r: Request| async move {
                let status = if r.uri().path() == "/deny" {
                    StatusCode::FORBIDDEN
                } else {
                    StatusCode::OK
                };
                let mut response = (status, "answer").into_response();
                response.headers_mut().insert(
                    "date",
                    HeaderValue::from_static("Thu, 01 Jan 1970 00:00:00 GMT"),
                );
                response
                    .headers_mut()
                    .append("set-cookie", HeaderValue::from_static("a=1"));
                response
                    .headers_mut()
                    .append("set-cookie", HeaderValue::from_static("b=2"));
                response
                    .headers_mut()
                    .insert("x-binary", HeaderValue::from_bytes(&[0x80, 0xff]).unwrap());
                response
            }
        },
        Limits::default(),
    );
    compio::runtime::Runtime::new()
        .unwrap()
        .block_on(async move {
            let stream = compio::net::TcpStream::from_std(stream).unwrap();
            let io = hyper_util::rt::TokioIo::new(
                Box::pin(compio::io::compat::AsyncStream::new(stream)).compat(),
            );
            let (mut client, connection) = hyper::client::conn::http2::handshake(Executor, io)
                .await
                .unwrap();
            let task = compio::runtime::spawn(connection);
            for (path, status) in [("/", StatusCode::OK), ("/deny", StatusCode::FORBIDDEN)] {
                let response = client
                    .send_request(
                        hyper::Request::builder()
                            .method("POST")
                            .uri(format!("http://localhost{path}"))
                            .body(Body::stream(futures_util::stream::pending()))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), status);
                assert_eq!(response.headers()["date"], "Thu, 01 Jan 1970 00:00:00 GMT");
                assert_eq!(response.headers().get_all("date").iter().count(), 1);
                assert_eq!(
                    response
                        .headers()
                        .get_all("set-cookie")
                        .iter()
                        .map(|v| v.as_bytes())
                        .collect::<Vec<_>>(),
                    [b"a=1".as_slice(), b"b=2".as_slice()]
                );
                assert_eq!(response.headers()["x-binary"].as_bytes(), &[0x80, 0xff]);
                assert_eq!(
                    response.into_body().collect().await.unwrap().to_bytes(),
                    "answer"
                );
            }
            drop(client);
            task.cancel().await;
        });
}

#[test]
fn discarded_data_still_validates_padding() {
    let mut s = server();
    handshake(&mut s);
    s.write_all(&frame(1, 4, 1, &request())).unwrap();
    until(&mut s, 3, 1);
    s.write_all(&frame(0, 8, 1, b"\x05ab")).unwrap();
    assert_eq!(&until(&mut s, 7, 0).1[4..8], &1u32.to_be_bytes());
}

fn pending_trailer_closed_by_response(request_shaped: bool) {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let mut s = server_with(
        move || {
            move |r: Request| {
                observed.fetch_add(1, Ordering::SeqCst);
                async move {
                    compio::time::sleep(Duration::from_millis(50)).await;
                    r.headers()
                        .get("x")
                        .map_or("initial", |v| v.to_str().unwrap())
                        .to_owned()
                }
            }
        },
        Limits::default(),
    );
    handshake(&mut s);
    let mut trailer = if request_shaped {
        request()
    } else {
        Vec::new()
    };
    trailer.extend(b"\x40\x01x\x01y");
    let split = trailer.len() - 2;
    let mut frames = frame(1, 4, 1, &request());
    frames.extend(frame(1, 1, 1, &trailer[..split]));
    s.write_all(&frames).unwrap();
    assert_eq!(until(&mut s, 3, 1).1, 0u32.to_be_bytes());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    s.write_all(&frame(9, 4, 1, &trailer[split..])).unwrap();
    let mut next = request();
    next.push(0xbe); // The discarded block must still insert x: y.
    s.write_all(&frame(1, 5, 3, &next)).unwrap();
    loop {
        let (kind, _, id, payload) = receive(&mut s);
        assert_ne!(kind, 7, "pending trailer killed the connection");
        assert!(
            !(id == 1 && matches!(kind, 0 | 1 | 3)),
            "pending trailer reopened or reset stream 1"
        );
        if kind == 0 && id == 3 {
            assert_eq!(payload, b"y");
            break;
        }
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "one invocation for stream 1 and one for stream 3"
    );
}

#[test]
fn legal_pending_trailer_is_discarded_when_early_response_closes_stream() {
    pending_trailer_closed_by_response(false);
}

#[test]
fn request_shaped_pending_trailer_cannot_reopen_closed_stream() {
    pending_trailer_closed_by_response(true);
}
