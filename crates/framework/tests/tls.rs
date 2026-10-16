#![cfg(feature = "tls")]
use maximus::{Request, Server, TlsConfig};
use maximus_tls::rustls;
use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::Arc,
};
#[test]
fn tls13_alpn_and_session_resumption_over_tcp() {
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let tls = TlsConfig::from_der(
        vec![cert.der().clone()],
        signing_key.serialize_der().try_into().unwrap(),
        false,
    )
    .unwrap();
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .tls(tls)
        .start(|| |_req: Request| async { "secure" })
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
    for expected in [rustls::HandshakeKind::Full, rustls::HandshakeKind::Resumed] {
        let socket = TcpStream::connect(server.local_addr()).unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let connection =
            rustls::ClientConnection::new(config.clone(), "localhost".try_into().unwrap()).unwrap();
        let mut stream = rustls::StreamOwned::new(connection, socket);
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.contains("secure"), "{response}");
        assert_eq!(
            stream.conn.protocol_version(),
            Some(rustls::ProtocolVersion::TLSv1_3)
        );
        assert_eq!(stream.conn.alpn_protocol(), Some(b"http/1.1".as_slice()));
        assert_eq!(stream.conn.handshake_kind(), Some(expected));
    }
    server.shutdown().unwrap();
}
