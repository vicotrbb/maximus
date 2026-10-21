#![cfg(any(feature = "cors", feature = "compression"))]
use maximus::{Body, Handler, Request};
#[cfg(feature = "cors")]
#[test]
fn cors_validates_origin_and_preflight() {
    use maximus::{Cors, Method, StatusCode};
    let cors = Cors::new(["https://example.com"], [Method::GET], ["x-token"], true).unwrap();
    assert!(Cors::new(["*"], [Method::GET], ["x-token"], true).is_err());
    let app = cors.wrap(|_: Request| async { "ok" });
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let request = |origin, method| {
            Request::new(
                maximus::http::Request::builder()
                    .method("OPTIONS")
                    .uri("/")
                    .header("origin", origin)
                    .header("access-control-request-method", method)
                    .header("access-control-request-headers", "X-Token")
                    .body(Body::empty())
                    .unwrap(),
            )
        };
        let response = app.handle(request("https://example.com", "GET")).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "https://example.com"
        );
        assert_eq!(
            response.headers()["access-control-allow-credentials"],
            "true"
        );
        assert_eq!(
            app.handle(request("https://evil.test", "GET"))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            app.handle(request("https://example.com", "DELETE"))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    });
}
#[cfg(feature = "compression")]
#[test]
fn gzip_negotiation_respects_q_zero_and_preserves_content() {
    use http_body_util::BodyExt;
    use std::io::Read;
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let app = maximus::Compression::default().wrap(|_: Request| async { "hello".repeat(300) });
        let request = |value| {
            Request::new(
                maximus::http::Request::builder()
                    .header("accept-encoding", value)
                    .body(Body::empty())
                    .unwrap(),
            )
        };
        let response = app.handle(request("gzip")).await;
        assert_eq!(response.headers()["content-encoding"], "gzip");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let mut decoded = String::new();
        flate2::read::GzDecoder::new(bytes.as_ref())
            .read_to_string(&mut decoded)
            .unwrap();
        assert_eq!(decoded, "hello".repeat(300));
        for value in ["gzip;q=0, *;q=1", "gzip;Q=0", "*;q=1, gzip;Q=0"] {
            let response = app.handle(request(value)).await;
            assert!(
                !response.headers().contains_key("content-encoding"),
                "{value}"
            );
        }
    });
}

#[cfg(feature = "cors")]
#[test]
fn cors_varies_cached_responses_even_without_an_allowed_origin() {
    use maximus::{Cors, Method};
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let app = Cors::new(["https://example.com"], [Method::GET], ["x-token"], false)
            .unwrap()
            .wrap(|_: Request| async {
                maximus::http::Response::builder()
                    .header("vary", "Accept-Encoding")
                    .body(Body::from("ok"))
                    .unwrap()
            });
        for origin in [None, Some("https://evil.test"), Some("https://example.com")] {
            let mut request = maximus::http::Request::builder();
            if let Some(origin) = origin {
                request = request.header("origin", origin);
            }
            let response = app
                .handle(Request::new(request.body(Body::empty()).unwrap()))
                .await;
            let vary: Vec<_> = response
                .headers()
                .get_all("vary")
                .iter()
                .map(|v| v.to_str().unwrap())
                .collect();
            assert!(vary.contains(&"Origin"));
            if response.status() == 200 {
                assert!(vary.contains(&"Accept-Encoding"));
            }
        }
        let response = app
            .handle(Request::new(
                maximus::http::Request::builder()
                    .method("OPTIONS")
                    .header("origin", "https://evil.test")
                    .header("access-control-request-method", "DELETE")
                    .body(Body::empty())
                    .unwrap(),
            ))
            .await;
        assert!(response.headers().get_all("vary").iter().any(|v| {
            v.to_str()
                .unwrap()
                .contains("Access-Control-Request-Method")
        }));
    });
}
