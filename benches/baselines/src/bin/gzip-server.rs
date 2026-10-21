//! Dedicated gzip fixture: other comparison servers keep their original body types.
use std::convert::Infallible;
use tower_http::compression::{Compression, CompressionLayer, CompressionLevel};
fn layer() -> CompressionLayer {
    CompressionLayer::new().quality(CompressionLevel::Precise(1))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = std::env::args().nth(1).unwrap_or("maximus".into());
    let address = "0.0.0.0:8080".parse()?;
    match engine.as_str() {
        "maximus" => {
            let server = maximus::Server::new(address)
                .workers(maximus::WorkerConfig {
                    workers: 1,
                    ..Default::default()
                })
                .start(|| {
                    maximus::Compression::default()
                        .wrap(|_: maximus::Request| async { "Hello, compression!".repeat(100) })
                })?;
            eprintln!("ready {engine}");
            server.wait()?;
        }
        "hyper" | "axum" => {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()?
                .block_on(async {
                    let listener = tokio::net::TcpListener::bind(address).await?;
                    eprintln!("ready {engine}");
                    if engine == "axum" {
                        let app = axum::Router::new()
                            .route(
                                "/compress",
                                axum::routing::get(|| async { "Hello, compression!".repeat(100) }),
                            )
                            .layer(layer());
                        axum::serve(listener, app).await?;
                    } else {
                        loop {
                            let (stream, _) = listener.accept().await?;
                            stream.set_nodelay(true)?;
                            tokio::spawn(async move {
                                let service = tower::service_fn(
                                    |_: hyper::Request<hyper::body::Incoming>| async {
                                        Ok::<_, Infallible>(
                                            hyper::Response::builder()
                                                .header("content-type", "text/plain; charset=utf-8")
                                                .body(http_body_util::Full::new(
                                                    bytes::Bytes::from(
                                                        "Hello, compression!".repeat(100),
                                                    ),
                                                ))
                                                .unwrap(),
                                        )
                                    },
                                );
                                let service =
                                    Compression::new(service).quality(CompressionLevel::Precise(1));
                                let _ = hyper::server::conn::http1::Builder::new()
                                    .serve_connection(
                                        hyper_util::rt::TokioIo::new(stream),
                                        hyper_util::service::TowerToHyperService::new(service),
                                    )
                                    .await;
                            });
                        }
                    }
                    Ok::<_, std::io::Error>(())
                })?;
        }
        _ => return Err("expected maximus, hyper or axum".into()),
    }
    Ok(())
}
