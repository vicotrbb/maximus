use maximus::{Request, Server, WorkerConfig};
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

fn exchange(address: std::net::SocketAddr, request: &[u8]) -> String {
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream.write_all(request).unwrap();
    let mut output = String::new();
    stream.read_to_string(&mut output).unwrap();
    output
}
#[test]
fn real_http1_pipelining_and_shutdown() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .workers(WorkerConfig {
            workers: 2,
            ..Default::default()
        })
        .start(|| |_req: Request| async { "hello" })
        .unwrap();
    let address = server.local_addr();
    let response = exchange(address, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\nGET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    assert_eq!(response.matches("200 OK").count(), 2, "{response}");
    assert_eq!(response.matches("hello").count(), 2);
    let malformed = exchange(address, b"GET / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\nContent-Length: 3\r\nConnection: close\r\n\r\nxxx");
    assert!(malformed.contains("400 Bad Request"), "{malformed}");
    server.shutdown().unwrap();
    assert!(TcpStream::connect(address).is_err());
}
#[test]
fn body_limit_and_request_timeout() {
    let limits = maximus::Limits {
        body_bytes: 4,
        request_timeout: Duration::from_millis(30),
        ..Default::default()
    };
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .limits(limits)
        .start(|| {
            |req: Request| async move {
                if req.uri().path() == "/slow" {
                    compio::time::sleep(Duration::from_secs(1)).await;
                }
                use http_body_util::BodyExt;
                use maximus::IntoResponse;
                match req.into_body().collect().await {
                    Ok(body) => body.to_bytes().into_response(),
                    Err(error) => error.into_response(),
                }
            }
        })
        .unwrap();
    let too_large = exchange(server.local_addr(), b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\nConnection: close\r\n\r\n12345");
    assert!(too_large.contains("413 Payload Too Large"), "{too_large}");
    let slow = exchange(
        server.local_addr(),
        b"GET /slow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(slow.contains("408 Request Timeout"), "{slow}");
    server.shutdown().unwrap();
}

#[test]
fn chunked_streaming_head_and_header_deadline() {
    use maximus::{Body, IntoResponse};
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .limits(maximus::Limits {
            header_timeout: Duration::from_millis(50),
            ..Default::default()
        })
        .start(|| {
            |_req: Request| async {
                Body::stream(futures_util::stream::iter([
                    Ok("one".into()),
                    Ok("two".into()),
                ]))
                .into_response()
            }
        })
        .unwrap();
    let response = exchange(
        server.local_addr(),
        b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(
        response.contains("transfer-encoding: chunked"),
        "{response}"
    );
    assert!(response.contains("one"));
    assert!(response.contains("two"));
    let head = exchange(
        server.local_addr(),
        b"HEAD / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    );
    assert!(!head.contains("one"));
    let mut slow = TcpStream::connect(server.local_addr()).unwrap();
    slow.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    slow.write_all(b"GET / HTTP/1.1\r\nHo").unwrap();
    let mut result = Vec::new();
    slow.read_to_end(&mut result).unwrap();
    assert!(result.is_empty() || String::from_utf8_lossy(&result).contains("408"));
    server.shutdown().unwrap();
}

#[test]
fn failed_worker_stops_siblings() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .workers(WorkerConfig {
            workers: 2,
            ..Default::default()
        })
        .start(|| {
            |_req: Request| async {
                panic!("test handler panic");
                #[allow(unreachable_code)]
                "unreachable"
            }
        })
        .unwrap();
    let mut stream = TcpStream::connect(server.local_addr()).unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let shutdown = server.shutdown_handle();
    let waiter = std::thread::spawn(move || {
        tx.send(server.wait()).unwrap();
    });
    let result = rx.recv_timeout(Duration::from_millis(300));
    shutdown.cancel();
    waiter.join().unwrap();
    assert!(
        result
            .expect("sibling workers must stop when one fails")
            .is_err()
    );
}

#[test]
fn sequential_connections_do_not_cancel_pending_accept() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| |_req: Request| async { "ok" })
        .unwrap();
    for index in 0..100 {
        let result = exchange(
            server.local_addr(),
            b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        );
        assert!(result.ends_with("ok"), "connection {index}: {result}");
    }
    server.shutdown().unwrap();
}

#[test]
fn stalled_response_releases_connection_admission() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .limits(maximus::Limits {
            connections: 1,
            request_timeout: Duration::from_millis(40),
            ..Default::default()
        })
        .start(|| {
            |req: Request| async move {
                if req.uri().path() == "/stall" {
                    maximus::Body::stream(futures_util::stream::pending())
                } else {
                    "ok".into()
                }
            }
        })
        .unwrap();
    let mut first = TcpStream::connect(server.local_addr()).unwrap();
    first
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    first
        .write_all(b"GET /stall HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let mut wire = Vec::new();
    first
        .read_to_end(&mut wire)
        .expect("stalled streaming body must be cancelled");
    assert!(
        exchange(
            server.local_addr(),
            b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .ends_with("ok")
    );
    server.shutdown().unwrap();
}

#[test]
fn rejects_ambiguous_framing_and_hosts_before_application() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .start(|| |_req: Request| async { "application" })
        .unwrap();
    for request in [
        "GET / HTTP/1.1\r\nConnection: close\r\n\r\n",
        "GET / HTTP/1.1\r\nHost: a\r\nHost: b\r\nConnection: close\r\n\r\n",
        "GET / HTTP/1.1\r\nHost: \r\nConnection: close\r\n\r\n",
        "GET / HTTP/1.1\r\nHost: invalid host\r\nConnection: close\r\n\r\n",
        "GET / HTTP/1.1\r\nHost: localhost:bad\r\nConnection: close\r\n\r\n",
        "GET / HTTP/1.1\r\nHost: [bad::ip]\r\nConnection: close\r\n\r\n",
        "GET / HTTP/1.1\r\nHost: [::1]suffix\r\nConnection: close\r\n\r\n",
        "GET / HTTP/1.1\r\nHost: [v1.address]suffix\r\nConnection: close\r\n\r\n",
        "GET /%2e%2e/secret HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n",
    ] {
        let response = exchange(server.local_addr(), request.as_bytes());
        assert!(!response.contains("application"), "{request:?}: {response}");
        assert!(response.contains("400 Bad Request"), "{response}");
    }
    for host in ["localhost:", "[::1]", "[::1]:8080", "[v1.address]"] {
        let response = exchange(
            server.local_addr(),
            format!("GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes(),
        );
        assert!(response.contains("200 OK"), "{host}: {response}");
    }
    // Reject ambiguous TE/CL framing before dispatch and never process a pipeline behind it.
    let response = exchange(server.local_addr(), b"POST / HTTP/1.1\r\nHost: a\r\nContent-Length: 1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\nGET / HTTP/1.1\r\nHost: a\r\n\r\n");
    assert!(response.contains("400 Bad Request"));
    assert!(!response.contains("application"));
    server.shutdown().unwrap();
}

#[test]
fn idle_connections_release_capacity_and_partial_startup_cleans_up() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .limits(maximus::Limits {
            connections: 1,
            idle_timeout: Duration::from_millis(40),
            ..Default::default()
        })
        .start(|| |_req: Request| async { "ok" })
        .unwrap();
    let mut idle = TcpStream::connect(server.local_addr()).unwrap();
    idle.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    idle.write_all(b"GET / HTTP/1.1\r\nHost: a\r\n\r\n")
        .unwrap();
    let mut output = String::new();
    idle.read_to_string(&mut output).unwrap();
    assert!(output.ends_with("ok"));
    assert!(
        exchange(
            server.local_addr(),
            b"GET / HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n"
        )
        .ends_with("ok")
    );
    server.shutdown().unwrap();
    let factory =
        || -> fn(Request) -> std::future::Ready<&'static str> { panic!("startup failure") };
    assert!(
        Server::new("127.0.0.1:0".parse().unwrap())
            .workers(WorkerConfig {
                workers: 2,
                ..Default::default()
            })
            .start(factory)
            .is_err()
    );
}
