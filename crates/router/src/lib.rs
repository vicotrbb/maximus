//! Allocation-free pattern matching and optional bounded dynamic registration.
#![forbid(unsafe_code)]
#[cfg(feature = "dynamic")]
use maximus_core::{Error, Result};

pub fn matches(pattern: &str, path: &str) -> bool {
    let mut path = path.split('/');
    for expected in pattern.split('/') {
        let Some(actual) = path.next() else {
            return false;
        };
        if expected.starts_with('{') && expected.ends_with('}') {
            if actual.is_empty() {
                return false;
            }
        } else if expected != actual {
            return false;
        }
    }
    path.next().is_none()
}

pub use maximus_core::validate_path;

#[cfg(feature = "dynamic")]
mod dynamic {
    use super::*;
    use maximus_core::{Handler, IntoResponse, Method, Request, Response, StatusCode};
    use std::{future::Future, pin::Pin, rc::Rc};
    type HandlerFn = Box<dyn Fn(Request) -> Pin<Box<dyn Future<Output = Response>>>>;
    struct Route {
        method: Method,
        pattern: String,
        shape: String,
        handler: HandlerFn,
    }
    // ponytail: bounded linear lookup; replace with an index when measured route counts justify it.
    pub struct DynamicRouter {
        routes: Vec<Route>,
        limit: usize,
    }
    impl DynamicRouter {
        pub fn new(limit: usize) -> Self {
            Self {
                routes: Vec::new(),
                limit,
            }
        }
        pub fn add<H: Handler>(&mut self, method: &str, pattern: &str, handler: H) -> Result<()> {
            let method = Method::from_bytes(method.as_bytes())
                .map_err(|_| Error::bad_request("invalid method"))?;
            if self.routes.len() == self.limit {
                return Err(Error::bad_request("route limit exceeded"));
            }
            validate_path(pattern)?;
            let shape = pattern
                .split('/')
                .map(|s| if s.starts_with('{') { "{}" } else { s })
                .collect::<Vec<_>>()
                .join("/");
            if self
                .routes
                .iter()
                .any(|r| r.method == method && r.shape == shape)
            {
                return Err(Error::bad_request("duplicate route"));
            }
            for segment in pattern.split('/') {
                if (segment.contains('{') || segment.contains('}'))
                    && !(segment.starts_with('{')
                        && segment.ends_with('}')
                        && segment.len() > 2
                        && !segment[1..segment.len() - 1].contains(['{', '}']))
                {
                    return Err(Error::bad_request("invalid route parameter"));
                }
            }
            let handler = Rc::new(handler);
            let index = self.routes.partition_point(|route| {
                route
                    .pattern
                    .split('/')
                    .map(|s| s.starts_with('{'))
                    .cmp(pattern.split('/').map(|s| s.starts_with('{')))
                    .then_with(|| (route.method != Method::HEAD).cmp(&(method != Method::HEAD)))
                    .is_le()
            });
            self.routes.insert(
                index,
                Route {
                    method,
                    shape,
                    pattern: pattern.to_owned(),
                    handler: Box::new(move |request| {
                        let handler = handler.clone();
                        Box::pin(async move { handler.handle(request).await })
                    }),
                },
            );
            Ok(())
        }
    }
    impl Handler for DynamicRouter {
        async fn handle(&self, mut request: Request) -> Response {
            if let Err(error) = validate_path(request.uri().path()) {
                return error.into_response();
            }
            let Some(index) = self
                .routes
                .iter()
                .position(|r| matches(&r.pattern, request.uri().path()))
            else {
                return StatusCode::NOT_FOUND.into_response();
            };
            let matching = &self.routes[index..];
            let routes = matching.iter().filter(|r| r.shape == matching[0].shape);
            if let Some(route) = routes.clone().find(|r| {
                request.method() == r.method
                    || (request.method() == Method::HEAD && r.method == Method::GET)
            }) {
                request.set_route(route.pattern.clone());
                return (route.handler)(request).await;
            }
            let mut allowed = String::new();
            for route in routes {
                if !allowed.is_empty() {
                    allowed.push_str(", ");
                }
                allowed.push_str(if route.method == Method::GET {
                    "GET, HEAD"
                } else {
                    route.method.as_str()
                });
            }
            let mut response = StatusCode::METHOD_NOT_ALLOWED.into_response();
            response
                .headers_mut()
                .insert("allow", allowed.parse().expect("validated method tokens"));
            response
        }
    }
}
#[cfg(feature = "dynamic")]
pub use dynamic::DynamicRouter;
