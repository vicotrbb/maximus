use maximus_core::{Body, Handler, Request, StatusCode};
use maximus_router::{DynamicRouter, matches, validate_path};
#[test]
fn matching_and_validation_reject_ambiguous_paths() {
    assert!(matches("/users/{id}", "/users/42"));
    assert!(!matches("/users/{id}", "/users/"));
    assert!(!matches("/users/{id}", "/users/42/more"));
    assert!(validate_path("/ok/%E2%82%AC").is_ok());
    for path in ["/a/%2f", "/a/%5c", "/a/%00", "/a/%ff", "/a/%zz", "/a/.."] {
        assert!(validate_path(path).is_err(), "{path}");
    }
}
#[test]
fn dynamic_router_bounds_and_method_semantics() {
    let rt = compio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut router = DynamicRouter::new(2);
        router
            .add("GET", "/{id}", |_req| async { "parameter" })
            .unwrap();
        router
            .add("GET", "/fixed", |_req| async { "fixed" })
            .unwrap();
        assert!(
            router
                .add("GET", "/extra", |_req| async { "extra" })
                .is_err()
        );
        let req = |method, path| {
            Request::new(
                maximus_core::http::Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
        };
        use http_body_util::BodyExt;
        let response = router.handle(req("GET", "/fixed")).await;
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            "fixed"
        );
        assert_eq!(
            router.handle(req("POST", "/fixed")).await.status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
        assert_eq!(
            router.handle(req("GET", "/a/b")).await.status(),
            StatusCode::NOT_FOUND
        );
    });
}
