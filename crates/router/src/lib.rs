//! Allocation-free pattern matching and optional bounded dynamic registration.
#![forbid(unsafe_code)]
use maximus_core::{Error, Result, decode};

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

/// Reject ambiguous separators and dot segments before routing or authorization.
pub fn validate_path(path: &str) -> Result<()> {
    if !path.starts_with('/') {
        return Err(Error::bad_request("expected absolute request path"));
    }
    for segment in path.split('/') {
        let decoded = decode(segment, false)?;
        if decoded == "."
            || decoded == ".."
            || decoded.contains(['/', '\\', '\0'])
            || decoded.chars().any(char::is_control)
        {
            return Err(Error::bad_request("ambiguous path segment"));
        }
    }
    Ok(())
}

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
            let normalized = |s: &str| {
                s.split('/')
                    .map(|s| if s.starts_with('{') { "{}" } else { s })
                    .collect::<Vec<_>>()
                    .join("/")
            };
            if self
                .routes
                .iter()
                .any(|r| r.method == method && normalized(&r.pattern) == normalized(pattern))
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
            self.routes.push(Route {
                method,
                shape: normalized(pattern),
                pattern: pattern.to_owned(),
                handler: Box::new(move |request| {
                    let handler = handler.clone();
                    Box::pin(async move { handler.handle(request).await })
                }),
            });
            self.routes.sort_by_key(|route| {
                (
                    route
                        .pattern
                        .split('/')
                        .map(|s| s.starts_with('{'))
                        .collect::<Vec<_>>(),
                    route.method != Method::HEAD,
                )
            });
            Ok(())
        }
    }
    impl Handler for DynamicRouter {
        async fn handle(&self, mut request: Request) -> Response {
            if let Err(error) = validate_path(request.uri().path()) {
                return error.into_response();
            }
            let mut allowed = String::new();
            let mut selected: Option<&str> = None;
            for route in &self.routes {
                if selected.is_none_or(|shape| shape == route.shape)
                    && matches(&route.pattern, request.uri().path())
                {
                    selected = Some(&route.shape);
                    if request.method() == route.method
                        || (request.method() == Method::HEAD && route.method == Method::GET)
                    {
                        request.set_route(route.pattern.clone());
                        return (route.handler)(request).await;
                    }
                    if !allowed.is_empty() {
                        allowed.push_str(", ");
                    }
                    allowed.push_str(if route.method == Method::GET {
                        "GET, HEAD"
                    } else {
                        route.method.as_str()
                    });
                }
            }
            if allowed.is_empty() {
                StatusCode::NOT_FOUND.into_response()
            } else {
                let mut response = StatusCode::METHOD_NOT_ALLOWED.into_response();
                response
                    .headers_mut()
                    .insert("allow", allowed.parse().expect("validated method tokens"));
                response
            }
        }
    }
}
#[cfg(feature = "dynamic")]
pub use dynamic::DynamicRouter;
