use http_body_util::BodyExt;
use maximus_core::{Body, Bytes, Error, Frame, IntoResponse, Limits, Request};
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
                let limits = Rc::new(Limits {
                    request_timeout: std::time::Duration::from_millis(200),
                    ..Limits::default()
                });
                let endpoint = maximus_http3::endpoint(socket, &config, &limits).unwrap();
                tx.send(()).unwrap();
                maximus_http3::serve(
                    endpoint,
                    Rc::new(|req: Request| async {
                        match req.uri().path() {
                            "/echo" => req
                                .into_body()
                                .collect()
                                .await
                                .map(|body| body.to_bytes())
                                .into_response(),
                            "/bad-length" => maximus_core::http::Response::builder()
                                .header("content-length", "1")
                                .body(Body::from("abc"))
                                .unwrap(),
                            "/short-length" => maximus_core::http::Response::builder()
                                .header("content-length", "9")
                                .body(Body::from("abc"))
                                .unwrap(),
                            "/invalid-304-length" => maximus_core::http::Response::builder()
                                .status(304)
                                .header("content-length", "invalid")
                                .body(Body::empty())
                                .unwrap(),
                            "/broken" => Body::stream(futures_util::stream::iter([
                                Ok(Bytes::from_static(b"partial")),
                                Err(Error::bad_request("broken producer")),
                            ]))
                            .into_response(),
                            "/stalled" => {
                                Body::stream(futures_util::stream::pending()).into_response()
                            }
                            "/credit" => Body::stream(futures_util::stream::iter(
                                (0..2048).map(|_| Ok(Bytes::from_static(&[42; 65536]))),
                            ))
                            .into_response(),
                            "/trailers" => {
                                let mut trailers = maximus_core::HeaderMap::new();
                                trailers.insert("x-check", "passed".parse().unwrap());
                                Body::frames(futures_util::stream::iter([
                                    Ok(Frame::data(Bytes::from_static(b"data"))),
                                    Ok(Frame::trailers(trailers)),
                                ]))
                                .into_response()
                            }
                            _ => "hello h3".into_response(),
                        }
                    }),
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
        // The peer must observe reset, never clean EOF, on producer error or timeout.
        for path in [
            "/broken",
            "/stalled",
            "/credit",
            "/bad-length",
            "/short-length",
            "/invalid-304-length",
        ] {
            let mut stream = sender
                .send_request(
                    maximus_core::http::Request::get(format!("https://localhost{path}"))
                        .body(())
                        .unwrap(),
                )
                .await
                .unwrap();
            stream.finish().await.unwrap();
            if path == "/credit" {
                compio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            let outcome = compio::time::timeout(std::time::Duration::from_secs(3), async {
                stream.recv_response().await?;
                while stream.recv_data().await?.is_some() {}
                Ok::<_, compio::quic::h3::error::StreamError>(())
            })
            .await
            .expect("response failure must meet its deadline");
            assert!(
                outcome.is_err(),
                "{path} ended as a successful truncated response"
            );
        }
        for lengths in [
            vec!["1"],
            vec!["99"],
            vec!["3", "99"],
            vec!["+3"],
            vec!["3, 3"],
        ] {
            let mut request = maximus_core::http::Request::post("https://localhost/echo");
            for length in lengths {
                request = request.header("content-length", length);
            }
            let mut stream = sender
                .send_request(request.body(()).unwrap())
                .await
                .unwrap();
            let _ = stream.send_data(Bytes::from_static(b"abc")).await;
            let _ = stream.finish().await;
            let outcome = compio::time::timeout(std::time::Duration::from_secs(1), async {
                stream.recv_response().await?;
                while stream.recv_data().await?.is_some() {}
                Ok::<_, compio::quic::h3::error::StreamError>(())
            })
            .await
            .unwrap();
            assert!(
                outcome.is_err(),
                "malformed Content-Length reached clean EOF"
            );
        }
        let mut stream = sender
            .send_request(
                maximus_core::http::Request::get("https://localhost/trailers")
                    .body(())
                    .unwrap(),
            )
            .await
            .unwrap();
        stream.finish().await.unwrap();
        assert_eq!(stream.recv_response().await.unwrap().status(), 200);
        assert_eq!(stream.recv_data().await.unwrap().unwrap().remaining(), 4);
        assert!(stream.recv_data().await.unwrap().is_none());
        assert_eq!(
            stream.recv_trailers().await.unwrap().unwrap()["x-check"],
            "passed"
        );
        drop(stream);
        drop(sender);
        driver.cancel().await;
        endpoint.close(0u32.into(), b"done");
        let _ = compio::time::timeout(std::time::Duration::from_secs(1), endpoint.shutdown()).await;
    });
    shutdown.cancel();
    worker.join().unwrap();
}
