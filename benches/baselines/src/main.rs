use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use std::{convert::Infallible, net::SocketAddr};
const TEXT: &str = "Hello, World!";
const SSE_DATA: &str = "one\ntwo\nthree";
const SSE_WIRE: &str = "data:one\ndata:two\ndata:three\n\n";
const JSON: &str = r#"{"message":"Hello, World!"}"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let engine = args.get(1).map(String::as_str).unwrap_or("maximus");
    let address: SocketAddr = std::env::var("ADDRESS")
        .unwrap_or("0.0.0.0:8080".into())
        .parse()?;
    let workers: usize = std::env::var("WORKERS").unwrap_or("1".into()).parse()?;
    let h2 = std::env::var("PROTOCOL").as_deref() == Ok("h2");
    let tls = if let Ok(cert) = std::env::var("MAXIMUS_CERT") {
        Some(maximus::TlsConfig::from_pem(
            &std::fs::read(cert)?,
            &std::fs::read(std::env::var("MAXIMUS_KEY")?)?,
            h2,
        )?)
    } else {
        None
    };
    match engine {
        "maximus" | "epoll" | "dynamic" => {
            let backend = if engine == "epoll" {
                maximus::Backend::Epoll
            } else {
                maximus::Backend::IoUring
            };
            let mut limits = maximus::Limits {
                requests_per_connection: usize::MAX,
                memory_bytes: 4 * 1024 * 1024 * 1024,
                ..Default::default()
            };
            if let Ok(connections) = std::env::var("IDLE_CONNECTIONS") {
                limits.connections = connections.parse()?;
                limits.body_bytes = 0;
                limits.header_bytes = 8192;
                limits.io_buffer_bytes = 1024;
                limits.memory_bytes = 1024 * 1024 * 1024;
                limits.header_timeout = std::time::Duration::from_secs(300);
                limits.idle_timeout = std::time::Duration::from_secs(300);
            }
            let mut server = maximus::Server::new(address)
                .workers(maximus::WorkerConfig {
                    workers,
                    backend,
                    ..Default::default()
                })
                .limits(limits);
            if let Some(tls) = tls {
                server = server.tls(tls);
            }
            let server = if engine == "dynamic" {
                server.start(|| {
                    let mut router = maximus::DynamicRouter::new(6);
                    router.add("GET", "/plaintext", plain).unwrap();
                    router.add("GET", "/json", json).unwrap();
                    router.add("GET", "/sse", events).unwrap();
                    router.add("POST", "/echo", echo).unwrap();
                    router.add("GET", "/error", error).unwrap();
                    router
                        .add("GET", "/users/{id}", |req: maximus::Request| async move {
                            req.path::<u64>("id").map(|id| id.to_string())
                        })
                        .unwrap();
                    router
                })?
            } else {
                server.start(|| {
                    maximus::routes! {
                        GET "/plaintext" => plain,
                        GET "/json" => json,
                        GET "/sse" => events,
                        POST "/echo" => echo,
                        GET "/error" => error,
                        GET "/users/{id:u64}" => user,
                    }
                })?
            };
            eprintln!("ready {engine} {} workers={workers}", server.local_addr());
            server.wait()?;
        }
        "axum" | "hyper" => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(workers)
                .enable_all()
                .build()?;
            runtime.block_on(async {
                let listener = tokio::net::TcpListener::bind(address).await?;
                eprintln!("ready {engine} {address} workers={workers}");
                let router = axum::Router::new()
                    .route("/plaintext", axum::routing::get(|| async { TEXT }))
                    .route(
                        "/sse",
                        axum::routing::get(|| async {
                            axum::response::Sse::new(futures_util::stream::iter([Ok::<
                                _,
                                Infallible,
                            >(
                                axum::response::sse::Event::default().data(SSE_DATA),
                            )]))
                        }),
                    )
                    .route(
                        "/error",
                        axum::routing::get(|| async {
                            (
                                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                                "handler error",
                            )
                        }),
                    )
                    .route(
                        "/json",
                        axum::routing::get(|| async {
                            ([("content-type", "application/json")], JSON)
                        }),
                    )
                    .route("/echo", axum::routing::post(|bytes: Bytes| async { bytes }))
                    .route(
                        "/users/{id}",
                        axum::routing::get(
                            |axum::extract::Path(id): axum::extract::Path<u64>| async move {
                                id.to_string()
                            },
                        ),
                    );
                if let Some(tls) = tls {
                    let acceptor = tokio_rustls::TlsAcceptor::from(tls.rustls().clone());
                    let axum = engine == "axum";
                    loop {
                        let (stream, _) = listener.accept().await?;
                        stream.set_nodelay(true)?;
                        let acceptor = acceptor.clone();
                        let router = router.clone();
                        tokio::spawn(async move {
                            let Ok(stream) = acceptor.accept(stream).await else {
                                return;
                            };
                            let service = hyper::service::service_fn(move |request| {
                                let router = router.clone();
                                async move {
                                    if axum {
                                        use hyper::service::Service;
                                        hyper_util::service::TowerToHyperService::new(router)
                                            .call(request)
                                            .await
                                    } else {
                                        Ok(hyper_handler(request)
                                            .await
                                            .unwrap()
                                            .map(axum::body::Body::new))
                                    }
                                }
                            });
                            let io = hyper_util::rt::TokioIo::new(stream);
                            if h2 {
                                let _ = hyper::server::conn::http2::Builder::new(
                                    hyper_util::rt::TokioExecutor::new(),
                                )
                                .max_concurrent_streams(32)
                                .initial_stream_window_size(65535)
                                .initial_connection_window_size(1024 * 1024)
                                .max_send_buf_size(16384)
                                .serve_connection(io, service)
                                .await;
                            } else {
                                let _ = hyper::server::conn::http1::Builder::new()
                                    .serve_connection(io, service)
                                    .await;
                            }
                        });
                    }
                } else if engine == "axum" {
                    axum::serve(listener, router).await?;
                } else {
                    loop {
                        let (stream, _) = listener.accept().await?;
                        stream.set_nodelay(true)?;
                        tokio::spawn(async move {
                            let _ = hyper::server::conn::http1::Builder::new()
                                .serve_connection(
                                    hyper_util::rt::TokioIo::new(stream),
                                    hyper::service::service_fn(hyper_handler),
                                )
                                .await;
                        });
                    }
                }
                Ok::<(), std::io::Error>(())
            })?;
        }
        "actix" => {
            actix_web::rt::System::new().block_on(async move {
                use actix_web::{App, HttpResponse, HttpServer, web};
                let server = HttpServer::new(|| {
                    App::new()
                        .route(
                            "/error",
                            web::get().to(|| async {
                                HttpResponse::InternalServerError()
                                    .content_type("text/plain; charset=utf-8")
                                    .body("handler error")
                            }),
                        )
                        .route(
                            "/plaintext",
                            web::get().to(|| async {
                                HttpResponse::Ok()
                                    .content_type("text/plain; charset=utf-8")
                                    .body(TEXT)
                            }),
                        )
                        .route(
                            "/json",
                            web::get().to(|| async {
                                HttpResponse::Ok()
                                    .content_type("application/json")
                                    .body(JSON)
                            }),
                        )
                        .route(
                            "/echo",
                            web::post()
                                .to(|body: web::Bytes| async { HttpResponse::Ok().body(body) }),
                        )
                        .route(
                            "/users/{id}",
                            web::get().to(|id: web::Path<u64>| async move { id.to_string() }),
                        )
                })
                .workers(workers)
                .bind(address)?;
                eprintln!("ready {engine} {address} workers={workers}");
                server.run().await
            })?;
        }
        _ => return Err("engine must be maximus, dynamic, epoll, axum, hyper or actix".into()),
    }
    Ok(())
}
async fn events(_: maximus::Request) -> impl maximus::IntoResponse {
    maximus::Sse::new(futures_util::stream::iter([Ok(maximus::Event::new(
        SSE_DATA,
    ))]))
}
async fn plain(_: maximus::Request) -> &'static str {
    TEXT
}
async fn error(_: maximus::Request) -> maximus::Error {
    maximus::Error::new(maximus::StatusCode::INTERNAL_SERVER_ERROR, "handler error")
}
async fn json(_: maximus::Request) -> maximus::Response {
    maximus::http::Response::builder()
        .header("content-type", "application/json")
        .body(JSON.into())
        .unwrap()
}
async fn user(_: maximus::Request, id: u64) -> String {
    id.to_string()
}
async fn echo(req: maximus::Request) -> Result<Bytes, maximus::Error> {
    Ok(req.into_body().collect().await?.to_bytes())
}
async fn hyper_handler(
    req: hyper::Request<hyper::body::Incoming>,
) -> Result<hyper::Response<Full<Bytes>>, Infallible> {
    let (status, content, body) = match (req.method().as_str(), req.uri().path()) {
        ("GET", "/error") => (
            500,
            "text/plain; charset=utf-8",
            Bytes::from_static(b"handler error"),
        ),
        ("GET", "/plaintext") => (
            200,
            "text/plain; charset=utf-8",
            Bytes::from_static(TEXT.as_bytes()),
        ),
        ("GET", "/json") => (200, "application/json", Bytes::from_static(JSON.as_bytes())),
        ("GET", "/sse") => (
            200,
            "text/event-stream",
            Bytes::from_static(SSE_WIRE.as_bytes()),
        ),
        ("POST", "/echo") => match http_body_util::Limited::new(req.into_body(), 1024 * 1024)
            .collect()
            .await
        {
            Ok(body) => (200, "application/octet-stream", body.to_bytes()),
            Err(_) => (413, "text/plain", Bytes::new()),
        },
        ("GET", path) if path.starts_with("/users/") => match path[7..].parse::<u64>() {
            Ok(id) => (
                200,
                "text/plain; charset=utf-8",
                Bytes::from(id.to_string()),
            ),
            Err(_) => (400, "text/plain", Bytes::new()),
        },
        _ => (404, "text/plain", Bytes::new()),
    };
    Ok(hyper::Response::builder()
        .status(status)
        .header("content-type", content)
        .body(Full::new(body))
        .unwrap())
}
