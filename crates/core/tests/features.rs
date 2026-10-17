use http_body_util::BodyExt;
use maximus_core::{Body, IntoResponse, Request};
#[cfg(feature = "json")]
#[test]
fn json_extraction_and_serialization_validate_input() {
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Input {
        value: u64,
    }
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let req = Request::new(
            http::Request::builder()
                .header("content-type", "application/json")
                .body(Body::from(r#"{"value":42}"#))
                .unwrap(),
        );
        assert_eq!(req.json::<Input>().await.unwrap(), Input { value: 42 });
        let req = Request::new(
            http::Request::builder()
                .header("content-type", "application/json")
                .body(Body::from("invalid"))
                .unwrap(),
        );
        assert!(req.json::<Input>().await.is_err());
        let response = maximus_core::Json(Input { value: 42 }).into_response();
        assert_eq!(response.headers()["content-type"], "application/json");
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            r#"{"value":42}"#
        );
    });
}
#[cfg(feature = "multipart")]
#[test]
fn multipart_streams_fields_with_limits() {
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let body = "--boundary\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\nmaximus\r\n--boundary--\r\n";
        let req = Request::new(http::Request::builder().header("content-type", "multipart/form-data; boundary=boundary").body(Body::from(body)).unwrap());
        let mut multipart = req.multipart(128, 16).unwrap();
        let field = multipart.next_field().await.unwrap().unwrap();
        assert_eq!(field.name(), Some("name"));
        assert_eq!(field.text().await.unwrap(), "maximus");
        assert!(multipart.next_field().await.unwrap().is_none());
        let req = Request::new(http::Request::builder().header("content-type", "multipart/form-data; boundary=boundary").body(Body::from(body)).unwrap());
        let mut multipart = req.multipart(128, 3).unwrap();
        assert!(multipart.next_field().await.unwrap().unwrap().bytes().await.is_err());
    });
}
#[cfg(feature = "sse")]
#[test]
fn sse_escapes_multiline_data_and_rejects_metadata_injection() {
    let event = maximus_core::Event::new("one\ntwo")
        .id("42")
        .unwrap()
        .event("update")
        .unwrap();
    assert!(maximus_core::Event::new("x").id("1\ndata:evil").is_err());
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let response =
            maximus_core::Sse::new(futures_util::stream::iter([Ok(event)])).into_response();
        assert_eq!(response.headers()["content-type"], "text/event-stream");
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            "id:42\nevent:update\ndata:one\ndata:two\n\n"
        );
    });
}

#[cfg(any(feature = "json", feature = "multipart"))]
#[test]
fn content_type_extraction_preserves_header_validation() {
    compio::runtime::Runtime::new().unwrap().block_on(async {
        for (values, message) in [
            (vec![], "missing header"),
            (vec![b"\xff".as_slice()], "invalid header"),
            (
                vec![b"application/json".as_slice(), b"application/json"],
                "duplicate header",
            ),
            (
                vec![b"\xff".as_slice(), b"application/json"],
                "duplicate header",
            ),
        ] {
            let make = || {
                let mut req = http::Request::new(Body::from("true"));
                for value in &values {
                    req.headers_mut().append(
                        "content-type",
                        http::HeaderValue::from_bytes(value).unwrap(),
                    );
                }
                Request::new(req)
            };
            let expected = make().header::<String>("content-type").unwrap_err();
            assert_eq!(expected.status, http::StatusCode::BAD_REQUEST);
            assert_eq!(expected.message, message);
            #[cfg(feature = "json")]
            {
                let actual = make().json::<bool>().await.unwrap_err();
                assert_eq!(actual.status, expected.status);
                assert_eq!(actual.message, expected.message);
            }
            #[cfg(feature = "multipart")]
            {
                let actual = make().multipart(1024, 1024).err().unwrap();
                assert_eq!(actual.status, expected.status);
                assert_eq!(actual.message, expected.message);
            }
        }
    });
}
