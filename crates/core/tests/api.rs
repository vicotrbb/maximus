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

#[test]
fn neutral_incoming_body_is_bounded_and_preserves_trailers() {
    use maximus_core::Frame;
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let mut trailers = http::HeaderMap::new();
        trailers.insert("x-check", "yes".parse().unwrap());
        let mut body = Body::incoming(
            futures_util::stream::iter([
                Ok(Frame::data("abc".into())),
                Ok(Frame::trailers(trailers)),
            ]),
            3,
        );
        assert_eq!(
            body.frame().await.unwrap().unwrap().into_data().unwrap(),
            "abc"
        );
        assert_eq!(
            body.frame()
                .await
                .unwrap()
                .unwrap()
                .into_trailers()
                .unwrap()["x-check"],
            "yes"
        );
        assert!(body.frame().await.is_none());
        let stream = futures_util::stream::iter([Ok(Frame::data("four".into()))]);
        assert_eq!(
            Body::incoming(stream, 3)
                .collect()
                .await
                .unwrap_err()
                .status,
            StatusCode::PAYLOAD_TOO_LARGE
        );
    });
}

#[test]
fn buffered_body_take_and_terminal_states_preserve_limits() {
    use maximus_core::{Bytes, HttpBody};
    compio::runtime::Runtime::new().unwrap().block_on(async {
        for mut body in [
            Body::empty(),
            Body::from(Bytes::new()),
            Body::from("").limited(0),
        ] {
            assert!(body.is_buffered());
            assert!(body.is_end_stream());
            assert_eq!(body.size_hint().exact(), Some(0));
            assert_eq!(body.take_bytes(), Some(Bytes::new()));
            assert!(body.frame().await.is_none());
        }
        let mut body = Body::from("abc").limited(3);
        assert_eq!(body.take_bytes().unwrap(), "abc");
        assert!(body.is_end_stream());
        assert_eq!(body.take_bytes(), Some(Bytes::new()));
        let mut body = Body::from("abc").limited(2);
        assert!(body.take_bytes().is_none());
        assert_eq!(
            body.frame().await.unwrap().unwrap_err().status,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert!(body.is_end_stream());
        assert_eq!(body.take_bytes(), Some(Bytes::new()));
        let mut body = Body::frames(futures_util::stream::iter([
            Err(maximus_core::Error::bad_request("broken")),
            Ok(maximus_core::Frame::data(Bytes::from_static(b"ignored"))),
        ]));
        assert!(body.take_bytes().is_none());
        assert!(body.frame().await.unwrap().is_err());
        assert!(body.is_buffered());
        assert!(body.frame().await.is_none());
    });
}
