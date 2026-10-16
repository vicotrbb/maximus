use http_body_util::BodyExt;
use maximus_core::{Body, Error, Handler, Request, StatusCode, guard, middleware, state};
use std::{cell::Cell, rc::Rc};
#[test]
fn typed_state_guards_and_middleware_preserve_order() {
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let calls = Rc::new(Cell::new(0));
        let app = state(
            calls.clone(),
            |_req, state: maximus_core::State<Rc<Cell<u32>>>| async move {
                state.0.set(state.0.get() + 1);
                "ok"
            },
        );
        let app = guard(app, |request: &Request| {
            if request.headers().contains_key("authorization") {
                Ok(())
            } else {
                Err(Error::new(StatusCode::UNAUTHORIZED, "missing token"))
            }
        });
        let app = middleware(app, |request, next| async move {
            let mut response = next.handle(request).await;
            response
                .headers_mut()
                .insert("x-middleware", "yes".parse().unwrap());
            response
        });
        let response = app
            .handle(Request::new(http::Request::new(Body::empty())))
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(calls.get(), 0);
        let req = http::Request::builder()
            .header("authorization", "test")
            .body(Body::empty())
            .unwrap();
        let response = app.handle(Request::new(req)).await;
        assert_eq!(response.headers()["x-middleware"], "yes");
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            "ok"
        );
        assert_eq!(calls.get(), 1);
    });
}
