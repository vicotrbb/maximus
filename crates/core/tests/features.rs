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

#[cfg(feature = "sse")]
#[test]
fn sse_preserves_all_line_endings_and_empty_data() {
    compio::runtime::Runtime::new().unwrap().block_on(async {
        // Enumerate mixed CR, LF, CRLF, trailing separators and UTF-8. The
        // normalized wire representation is the existing public contract.
        let alphabet = ["a", "\r", "\n", "é"];
        for length in 0..=6 {
            for mut index in 0..4usize.pow(length) {
                let mut data = String::new();
                for _ in 0..length {
                    data.push_str(alphabet[index % 4]);
                    index /= 4;
                }
                let normalized = data.replace("\r\n", "\n").replace('\r', "\n");
                let mut expected = String::new();
                for line in normalized.split('\n') {
                    expected.push_str("data:");
                    expected.push_str(line);
                    expected.push('\n');
                }
                expected.push('\n');
                let response = maximus_core::Sse::new(futures_util::stream::iter([Ok(
                    maximus_core::Event::new(data.clone()),
                )]))
                .into_response();
                assert_eq!(
                    response.into_body().collect().await.unwrap().to_bytes(),
                    expected,
                    "{data:?}"
                );
            }
        }
        let event = maximus_core::Event::new("").retry(std::time::Duration::from_millis(1234));
        let response =
            maximus_core::Sse::new(futures_util::stream::iter([Ok(event)])).into_response();
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            "retry:1234\ndata:\n\n"
        );
    });
}

#[cfg(feature = "sse")]
#[test]
fn sse_preserves_leading_spaces_in_data_and_metadata() {
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let event = maximus_core::Event::new(" leading\n  two\r\n three")
            .id(" id")
            .unwrap()
            .event(" event")
            .unwrap();
        let response =
            maximus_core::Sse::new(futures_util::stream::iter([Ok(event)])).into_response();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        // WHATWG removes exactly one optional ASCII space after the colon.
        let fields: Vec<_> = std::str::from_utf8(&bytes)
            .unwrap()
            .lines()
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name, value.strip_prefix(' ').unwrap_or(value)))
            .collect();
        assert_eq!(
            fields,
            [
                ("id", " id"),
                ("event", " event"),
                ("data", " leading"),
                ("data", "  two"),
                ("data", " three")
            ]
        );
    });
}
