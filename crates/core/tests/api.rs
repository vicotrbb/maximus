use http_body_util::BodyExt;
use maximus_core::{Body, IntoResponse, Request, StatusCode};

fn request(uri: &str) -> Request {
    Request::new(
        http::Request::builder()
            .uri(uri)
            .body(Body::empty())
            .unwrap(),
    )
}
#[test]
fn typed_query_and_paths_are_strict() {
    let mut req = request("/users/42?q=hello+world&n=7");
    req.set_route("/users/{id}");
    assert_eq!(req.path::<u64>("id").unwrap(), 42);
    assert_eq!(req.query::<String>("q").unwrap(), "hello world");
    assert_eq!(req.query::<u8>("n").unwrap(), 7);
    assert!(req.query::<u8>("missing").is_err());
    assert!(request("/?x=1&x=2").query::<u64>("x").is_err());
    assert!(request("/?x=%ff").query::<String>("x").is_err());
    assert!(request("/?x=%xz").query::<String>("x").is_err());
}
#[test]
fn typed_headers_reject_duplicates_and_invalid_values() {
    let mut req = request("/");
    req.headers_mut().append("x-id", "1".parse().unwrap());
    assert_eq!(req.header::<u8>("x-id").unwrap(), 1);
    req.headers_mut().append("x-id", "2".parse().unwrap());
    assert!(req.header::<u8>("x-id").is_err());
}
#[test]
fn static_response_preserves_bytes() {
    let response = "hello".into_response();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(http_body::Body::size_hint(response.body()).exact(), Some(5));
}
#[test]
fn bounded_stream_rejects_overflow() {
    let rt = compio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let stream = futures_util::stream::iter([
            Ok(bytes::Bytes::from_static(b"123")),
            Ok(bytes::Bytes::from_static(b"45")),
        ]);
        let mut body = Body::stream(stream).limited(4);
        assert_eq!(
            body.frame().await.unwrap().unwrap().into_data().unwrap(),
            "123"
        );
        assert_eq!(
            body.frame().await.unwrap().unwrap_err().status,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert!(body.frame().await.is_none());
    });
}
#[test]
fn body_collect_and_error_conversion() {
    let rt = compio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        assert_eq!(Body::from("ok").collect().await.unwrap().to_bytes(), "ok");
        let response = maximus_core::Error::bad_request("bad input").into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    });
}

#[test]
fn admission_reserves_multiplexed_bodies_and_tls_records() {
    let limits = maximus_core::Limits::default();
    assert!(limits.protocol_capacity(true, true) < limits.protocol_capacity(false, true));
    assert!(limits.protocol_capacity(false, true) < limits.connection_capacity());
    let limits = maximus_core::Limits {
        memory_bytes: 65536,
        ..limits
    };
    assert_eq!(limits.protocol_capacity(true, true), 0);
    assert!(limits.validate().is_err());
}
