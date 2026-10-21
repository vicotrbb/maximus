//! Uninstrumented component timings. Run frozen baseline/candidate pairs on one CPU.
use maximus::{Body, DynamicRouter, Error, Handler, IntoResponse, Request, StatusCode};
use std::{hint::black_box, time::Instant};

fn response_case(name: &str, status: StatusCode, make: impl Fn() -> maximus::Response) {
    const COUNT: usize = 2_000_000;
    let mut response = make();
    assert_eq!(response.status(), status);
    assert_eq!(
        response.headers()["content-type"],
        "text/plain; charset=utf-8"
    );
    assert_eq!(response.body_mut().take_bytes().unwrap(), "handler error");
    for _ in 0..1000 {
        black_box(make());
    }
    let started = Instant::now();
    for _ in 0..COUNT {
        black_box(make());
    }
    println!(
        "{name},{COUNT},{:.3}",
        started.elapsed().as_nanos() as f64 / COUNT as f64
    );
}

fn main() {
    println!("component,operations,ns_per_operation");
    response_case("static-text", StatusCode::OK, || {
        "handler error".into_response()
    });
    response_case("static-error", StatusCode::BAD_REQUEST, || {
        Error::bad_request("handler error").into_response()
    });
    response_case("owned-error", StatusCode::BAD_REQUEST, || {
        Error::bad_request("handler error".to_owned()).into_response()
    });
    maximus_runtime::WorkerConfig::default()
        .build(0)
        .unwrap()
        .block_on(async {
            let mut router = DynamicRouter::new(2);
            router
                .add("GET", "/plaintext", |_: Request| async { Body::empty() })
                .unwrap();
            router
                .add("GET", "/users/{id}", |req: Request| async move {
                    assert_eq!(req.path::<u64>("id").unwrap(), 42);
                    Body::empty()
                })
                .unwrap();
            for (name, method, path, status) in [
                (
                    "literal",
                    maximus::Method::GET,
                    "/plaintext",
                    StatusCode::OK,
                ),
                ("capture", maximus::Method::GET, "/users/42", StatusCode::OK),
                (
                    "method-not-allowed",
                    maximus::Method::PATCH,
                    "/plaintext",
                    StatusCode::METHOD_NOT_ALLOWED,
                ),
            ] {
                let uri = maximus::Uri::from_static(path);
                let request = || {
                    let mut req = maximus::http::Request::new(Body::empty());
                    *req.method_mut() = method.clone();
                    *req.uri_mut() = uri.clone();
                    Request::new(req)
                };
                let response = router.handle(request()).await;
                assert_eq!(response.status(), status);
                if status == StatusCode::METHOD_NOT_ALLOWED {
                    assert_eq!(response.headers()["allow"], "GET, HEAD");
                }
                for _ in 0..1000 {
                    black_box(router.handle(request()).await);
                }
                const COUNT: usize = 2_000_000;
                let started = Instant::now();
                for _ in 0..COUNT {
                    black_box(router.handle(request()).await);
                }
                println!(
                    "{name},{COUNT},{:.3}",
                    started.elapsed().as_nanos() as f64 / COUNT as f64
                );
            }
            for literal in [false, true] {
                let patterns: Vec<_> = (0..32)
                    .map(|id| {
                        if literal {
                            format!("/items/{id}")
                        } else {
                            format!("/items/{id}/{{value}}")
                        }
                    })
                    .collect();
                const COUNT: usize = 10_000;
                let started = Instant::now();
                for _ in 0..COUNT {
                    let mut router = DynamicRouter::new(64);
                    for pattern in &patterns {
                        for method in ["GET", "POST"] {
                            router
                                .add(method, pattern, |_: Request| async { Body::empty() })
                                .unwrap();
                        }
                    }
                    black_box(router);
                }
                let name = if literal {
                    "register-64-literal"
                } else {
                    "register-64"
                };
                println!(
                    "{name},{COUNT},{:.3}",
                    started.elapsed().as_nanos() as f64 / COUNT as f64
                );
            }
        });
}
