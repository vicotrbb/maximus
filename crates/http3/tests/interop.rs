use maximus_core::{Limits, Request};
use maximus_runtime::{Shutdown, WorkerConfig};
use maximus_tls::TlsConfig;
use std::{rc::Rc, sync::mpsc};
#[test]
fn real_quic_http3_request_response() {
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let certificate = cert.der().clone();
    let config = TlsConfig::from_der(
        vec![cert.der().clone()],
        signing_key.serialize_der().try_into().unwrap(),
        false,
    )
    .unwrap();
    let socket =
        maximus_net::bind_udp("127.0.0.1:0".parse().unwrap(), &Default::default()).unwrap();
    let address = socket.local_addr().unwrap();
    let shutdown = Shutdown::default();
    let signal = shutdown.clone();
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        WorkerConfig::default()
            .build(0)
            .unwrap()
            .block_on(async move {
                let limits = Rc::new(Limits::default());
                let endpoint = maximus_http3::endpoint(socket, &config, &limits).unwrap();
                tx.send(()).unwrap();
                maximus_http3::serve(
                    endpoint,
                    Rc::new(|_req: Request| async { "hello h3" }),
                    limits,
                    signal,
                )
                .await
                .unwrap();
            });
    });
    rx.recv().unwrap();
    WorkerConfig::default().build(0).unwrap().block_on(async {
        let mut roots = maximus_tls::rustls::RootCertStore::empty();
        roots.add(certificate).unwrap();
        let config = maximus_tls::rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
            maximus_tls::rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&maximus_tls::rustls::version::TLS13])
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
        let endpoint = compio::quic::ClientBuilder::new_with_rustls_client_config(config)
            .with_alpn_protocols(&["h3"])
            .bind("127.0.0.1:0")
            .await
            .unwrap();
        let connection = endpoint
            .connect(address, "localhost", None)
            .unwrap()
            .await
            .unwrap();
        let (mut connection, mut sender) = compio::quic::h3::client::new(connection).await.unwrap();
        let driver = compio::runtime::spawn(async move { connection.wait_idle().await });
        let mut stream = sender
            .send_request(
                maximus_core::http::Request::get("https://localhost/")
                    .body(())
                    .unwrap(),
            )
            .await
            .unwrap();
        stream.finish().await.unwrap();
        let response = stream.recv_response().await.unwrap();
        assert_eq!(response.status(), 200);
        use bytes::Buf;
        let mut output = Vec::new();
        while let Some(mut chunk) = stream.recv_data().await.unwrap() {
            output.extend_from_slice(&chunk.copy_to_bytes(chunk.remaining()));
        }
        assert_eq!(output, b"hello h3");
        drop(stream);
        drop(sender);
        driver.cancel().await;
        endpoint.close(0u32.into(), b"done");
        let _ = compio::time::timeout(std::time::Duration::from_secs(1), endpoint.shutdown()).await;
    });
    shutdown.cancel();
    worker.join().unwrap();
}
