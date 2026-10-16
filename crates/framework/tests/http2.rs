#![cfg(feature = "http2")]
use http_body_util::BodyExt;
use maximus::{Body, Request, Server};
#[test]
fn multiplexed_http2_streams_and_flow_control() {
    let server = Server::new("127.0.0.1:0".parse().unwrap())
        .http2_prior_knowledge(true)
        .start(|| {
            |req: Request| async move {
                if req.uri().path() == "/slow" {
                    compio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                "x".repeat(256 * 1024)
            }
        })
        .unwrap();
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let stream = compio::net::TcpStream::connect(server.local_addr())
            .await
            .unwrap();
        let io = maximus_http1::adapt(compio::io::compat::AsyncStream::new(stream));
        let (mut client, connection) =
            hyper::client::conn::http2::handshake(maximus_http1::Executor, io)
                .await
                .unwrap();
        let connection = compio::runtime::spawn(connection);
        let request = |path| {
            hyper::Request::builder()
                .uri(format!("http://localhost{path}"))
                .body(Body::empty())
                .unwrap()
        };
        let slow = client.send_request(request("/slow"));
        let fast = client.send_request(request("/fast"));
        let fast = fast.await.unwrap();
        assert_eq!(fast.version(), hyper::Version::HTTP_2);
        assert_eq!(
            fast.into_body().collect().await.unwrap().to_bytes().len(),
            256 * 1024
        );
        assert_eq!(
            slow.await
                .unwrap()
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .len(),
            256 * 1024
        );
        let head = client
            .send_request(
                hyper::Request::builder()
                    .method("HEAD")
                    .uri("http://localhost/fast")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(head.headers()["content-length"], "262144");
        assert!(
            head.into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .is_empty()
        );
        drop(client);
        connection.cancel().await;
    });
    server.shutdown().unwrap();
}
