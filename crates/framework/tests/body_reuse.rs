use http_body_util::BodyExt;
use maximus::{Body, Limits, Request, Server};
use std::io::{Read, Write};

fn payload() -> Vec<u8> {
    (0..131072u64)
        .step_by(8)
        .flat_map(u64::to_le_bytes)
        .collect()
}

async fn echo_retaining_first_frame(request: Request) -> Body {
    if request.uri().path() == "/next" {
        return "next".into();
    }
    let mut body = request.into_body();
    let first = body.frame().await.unwrap().unwrap().into_data().unwrap();
    let saved = first.to_vec();
    let mut received = saved.clone();
    let mut later_frames = 0;
    while let Some(frame) = body.frame().await {
        received.extend_from_slice(&frame.unwrap().into_data().unwrap());
        later_frames += 1;
    }
    assert!(later_frames > 0, "exercise more than one receive window");
    assert_eq!(first.as_ref(), saved, "later reads changed a held frame");
    assert_eq!(received, payload(), "receive windows lost their positions");
    received.into()
}

fn response(stream: &mut (impl Read + Write)) -> Vec<u8> {
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        assert!(head.len() < 8192);
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    let head = std::str::from_utf8(&head).unwrap();
    assert!(head.starts_with("HTTP/1.1 200 OK\r\n"), "{head}");
    let length = head
        .split("\r\n")
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .unwrap()
        .1
        .trim()
        .parse()
        .unwrap();
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    body
}

fn exchange(stream: &mut (impl Read + Write)) {
    let body = payload();
    write!(
        stream,
        "POST /echo HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .unwrap();
    for chunk in body.chunks(4093) {
        stream.write_all(chunk).unwrap();
    }
    stream.flush().unwrap();
    assert_eq!(response(stream), body);
    stream
        .write_all(b"GET /next HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    stream.flush().unwrap();
    assert_eq!(response(stream), b"next");
}

fn server(capacity: usize) -> Server {
    Server::new("127.0.0.1:0".parse().unwrap()).limits(Limits {
        io_buffer_bytes: capacity,
        ..Default::default()
    })
}

fn connect(address: std::net::SocketAddr) -> std::net::TcpStream {
    let socket = std::net::TcpStream::connect(address).unwrap();
    let timeout = Some(std::time::Duration::from_secs(5));
    socket.set_read_timeout(timeout).unwrap();
    socket.set_write_timeout(timeout).unwrap();
    socket
}

#[test]
fn native_receive_windows_preserve_payload_ownership_and_connection_reuse() {
    for capacity in [1024, 16384] {
        let server = server(capacity)
            .start(|| echo_retaining_first_frame)
            .unwrap();
        exchange(&mut connect(server.local_addr()));
        server.shutdown().unwrap();
    }
}

#[cfg(feature = "tls")]
#[test]
fn tls_receive_windows_preserve_payload_ownership_and_connection_reuse() {
    use maximus::TlsConfig;
    use maximus_tls::rustls;
    use std::sync::Arc;

    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let tls = TlsConfig::from_der(
        vec![cert.der().clone()],
        signing_key.serialize_der().try_into().unwrap(),
        false,
    )
    .unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert.der().clone()).unwrap();
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    let config = Arc::new(config);
    for capacity in [1024, 16384] {
        let server = server(capacity)
            .tls(tls.clone())
            .start(|| echo_retaining_first_frame)
            .unwrap();
        let connection =
            rustls::ClientConnection::new(config.clone(), "localhost".try_into().unwrap()).unwrap();
        let mut stream = rustls::StreamOwned::new(connection, connect(server.local_addr()));
        exchange(&mut stream);
        assert_eq!(stream.conn.alpn_protocol(), Some(b"http/1.1".as_slice()));
        drop(stream);
        server.shutdown().unwrap();
    }
}
