#![cfg(feature = "macros")]
use http_body_util::BodyExt;
use maximus::{Body, Handler, IntoResponse, Request, StatusCode, routes};
async fn hello(_: Request) -> &'static str {
    "hello"
}
async fn user(_: Request, id: u64) -> String {
    format!("user {id}")
}
#[test]
fn compiled_routes_extract_typed_parameters_and_match_methods() {
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let handler = routes! { GET "/" => hello, GET "/users/{id:u64}" => user };
        let req = |method, uri| {
            Request::new(
                maximus::http::Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
        };
        assert_eq!(
            handler
                .handle(req("GET", "/"))
                .await
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
            "hello"
        );
        assert_eq!(
            handler
                .handle(req("GET", "/users/42"))
                .await
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
            "user 42"
        );
        assert_eq!(
            handler.handle(req("GET", "/users/no")).await.status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            handler.handle(req("POST", "/")).await.status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
        assert_eq!(
            handler.handle(req("GET", "/missing")).await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            handler.handle(req("GET", "/users/%2f")).await.status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            StatusCode::NO_CONTENT.into_response().status(),
            StatusCode::NO_CONTENT
        );
    });
}

#[test]
fn literal_paths_own_method_errors_and_explicit_head_wins() {
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let router = routes! { GET "/fixed" => hello, HEAD "/fixed" => head, POST "/{id}" => hello, GET "/{first}/tail" => hello, GET "/prefix/{last}" => head };
        async fn head(_: Request) -> &'static str { "explicit" }
        let req = |method, path| Request::new(maximus::http::Request::builder().method(method).uri(path).body(Body::empty()).unwrap());
        assert_eq!(router.handle(req("POST", "/fixed")).await.status(), StatusCode::METHOD_NOT_ALLOWED);
        let response = router.handle(req("HEAD", "/fixed")).await;
        assert_eq!(response.into_body().collect().await.unwrap().to_bytes(), "explicit");
        let response = router.handle(req("GET", "/prefix/tail")).await;
        assert_eq!(response.into_body().collect().await.unwrap().to_bytes(), "explicit");
        let response = router.handle(req("PATCH", "/fixed")).await;
        assert!(response.headers()["allow"].to_str().unwrap().contains("HEAD"));
    });
}
