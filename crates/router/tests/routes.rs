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

#[test]
fn dynamic_shapes_preserve_precedence_captures_and_allow_order() {
    use http_body_util::BodyExt;
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let mut router = DynamicRouter::new(12);
        for (method, pattern, output) in [
            ("POST", "/{id}", "parameter"),
            ("GET", "/fixed", "get"),
            ("GET", "/other", "other"),
            ("POST", "/fixed", "post"),
            ("HEAD", "/fixed", "head"),
            ("GET", "/{first}/tail", "wildcard-first"),
            ("GET", "/prefix/{last}", "literal-first"),
        ] {
            router
                .add(method, pattern, move |_| async move { output })
                .unwrap();
        }
        assert!(
            router
                .add("POST", "/{renamed}", |_| async { "duplicate" })
                .is_err()
        );
        router
            .add("GET", "/users/{id}", |req: Request| async move {
                req.path::<u64>("id").unwrap().to_string()
            })
            .unwrap();
        router
            .add("POST", "/users/{name}", |req: Request| async move {
                req.path::<String>("name").unwrap()
            })
            .unwrap();
        for (method, path, status, body, allow) in [
            ("GET", "/fixed", StatusCode::OK, "get", None),
            ("POST", "/fixed", StatusCode::OK, "post", None),
            ("HEAD", "/fixed", StatusCode::OK, "head", None),
            ("GET", "/prefix/tail", StatusCode::OK, "literal-first", None),
            ("GET", "/users/42", StatusCode::OK, "42", None),
            ("HEAD", "/users/42", StatusCode::OK, "42", None),
            ("POST", "/users/alice", StatusCode::OK, "alice", None),
            (
                "PATCH",
                "/fixed",
                StatusCode::METHOD_NOT_ALLOWED,
                "",
                Some("HEAD, GET, HEAD, POST"),
            ),
            ("GET", "/a/b/c", StatusCode::NOT_FOUND, "", None),
            (
                "GET",
                "/%2f",
                StatusCode::BAD_REQUEST,
                "ambiguous path segment",
                None,
            ),
        ] {
            let request = Request::new(
                maximus_core::http::Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            );
            let response = router.handle(request).await;
            assert_eq!(response.status(), status, "{method} {path}");
            assert_eq!(
                response.headers().get("allow").map(|v| v.to_str().unwrap()),
                allow
            );
            assert_eq!(
                response.into_body().collect().await.unwrap().to_bytes(),
                body
            );
        }
    });
}
