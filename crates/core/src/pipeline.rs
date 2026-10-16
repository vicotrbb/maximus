use crate::{Handler, IntoResponse, Request, Response, Result};
use std::{future::Future, ops::Deref, rc::Rc};

/// Explicit worker-local state. Construct it inside the server's handler factory.
pub struct State<S>(pub Rc<S>);
impl<S> Clone for State<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<S> Deref for State<S> {
    type Target = S;
    fn deref(&self) -> &S {
        &self.0
    }
}

pub fn state<S: 'static, F, Fut, R>(value: S, handler: F) -> impl Handler
where
    F: Fn(Request, State<S>) -> Fut + 'static,
    Fut: Future<Output = R>,
    R: IntoResponse,
{
    let state = State(Rc::new(value));
    move |request| handler(request, state.clone())
}

pub fn middleware<H, F, Fut, R>(handler: H, layer: F) -> impl Handler
where
    H: Handler,
    F: Fn(Request, Rc<H>) -> Fut + 'static,
    Fut: Future<Output = R>,
    R: IntoResponse,
{
    let handler = Rc::new(handler);
    move |request| layer(request, handler.clone())
}

pub fn guard<H: Handler, F: Fn(&Request) -> Result<()> + 'static>(
    handler: H,
    check: F,
) -> impl Handler {
    Guard { handler, check }
}
struct Guard<H, F> {
    handler: H,
    check: F,
}
impl<H: Handler, F: Fn(&Request) -> Result<()> + 'static> Handler for Guard<H, F> {
    async fn handle(&self, request: Request) -> Response {
        match (self.check)(&request) {
            Ok(()) => self.handler.handle(request).await,
            Err(error) => error.into_response(),
        }
    }
}
