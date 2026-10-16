#![no_main]
use libfuzzer_sys::fuzz_target;
use maximus_core::{Body, Request, decode, http};
fuzz_target!(|input: &str| {
    for query in [false, true] {
        if let Ok(decoded) = decode(input, query) {
            // Percent decoding cannot increase the byte length.
            assert!(decoded.len() <= input.len());
        }
    }
    let _ = maximus_router::validate_path(input);
    let _ = maximus_router::matches("/users/{id}", input);
    if let Ok(uri) = input.parse::<http::Uri>() {
        let mut request = Request::new(
            http::Request::builder()
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        );
        request.set_route("/users/{id}");
        let _ = request.path::<u64>("id");
        let _ = request.path::<String>("id");
        let _ = request.query::<String>("q");
        let _ = request.query::<u64>("q");
    }
});
