use maximus_core::{
    Error, Handler, HeaderValue, IntoResponse, Method, Request, Response, Result, StatusCode,
    http::header::HeaderName,
};

pub struct Cors {
    origins: Vec<HeaderValue>,
    methods: Vec<Method>,
    headers: Vec<HeaderName>,
    credentials: bool,
}
impl Cors {
    pub fn new<const O: usize, const M: usize, const H: usize>(
        origins: [&str; O],
        methods: [Method; M],
        headers: [&str; H],
        credentials: bool,
    ) -> Result<Self> {
        if credentials && origins.contains(&"*") {
            return Err(Error::bad_request(
                "credentialed CORS requires explicit origins",
            ));
        }
        Ok(Self {
            origins: origins
                .into_iter()
                .map(|s| s.parse().map_err(|_| Error::bad_request("invalid origin")))
                .collect::<Result<_>>()?,
            methods: methods.into(),
            headers: headers
                .into_iter()
                .map(|s| {
                    s.parse()
                        .map_err(|_| Error::bad_request("invalid CORS header name"))
                })
                .collect::<Result<_>>()?,
            credentials,
        })
    }
    pub fn wrap<H: Handler>(self, handler: H) -> impl Handler {
        CorsHandler {
            config: self,
            handler,
        }
    }
}
struct CorsHandler<H> {
    config: Cors,
    handler: H,
}
impl<H: Handler> Handler for CorsHandler<H> {
    async fn handle(&self, request: Request) -> Response {
        let mut origins = request.headers().get_all("origin").iter();
        let Some(origin) = origins.next() else {
            return self.handler.handle(request).await;
        };
        if origins.next().is_some() {
            return StatusCode::BAD_REQUEST.into_response();
        }
        let origin = origin.clone();
        let wildcard = self.config.origins.iter().any(|o| o == "*");
        if !wildcard && !self.config.origins.contains(&origin) {
            return StatusCode::FORBIDDEN.into_response();
        }
        let preflight = request.method() == Method::OPTIONS
            && request
                .headers()
                .contains_key("access-control-request-method");
        let mut response = if preflight {
            let Ok(method) = request.header::<Method>("access-control-request-method") else {
                return StatusCode::BAD_REQUEST.into_response();
            };
            if !self.config.methods.contains(&method) {
                return StatusCode::FORBIDDEN.into_response();
            }
            if request
                .headers()
                .contains_key("access-control-request-headers")
            {
                let Ok(headers) = request.header::<String>("access-control-request-headers") else {
                    return StatusCode::BAD_REQUEST.into_response();
                };
                for name in headers.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    if !name
                        .parse::<HeaderName>()
                        .is_ok_and(|name| self.config.headers.contains(&name))
                    {
                        return StatusCode::FORBIDDEN.into_response();
                    }
                }
            }
            let mut response = StatusCode::NO_CONTENT.into_response();
            response.headers_mut().insert(
                "access-control-allow-methods",
                HeaderValue::from_str(method.as_str()).expect("validated method"),
            );
            if !self.config.headers.is_empty() {
                response.headers_mut().insert(
                    "access-control-allow-headers",
                    self.config
                        .headers
                        .iter()
                        .map(HeaderName::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                        .parse()
                        .expect("validated header names"),
                );
            }
            response
                .headers_mut()
                .insert("access-control-max-age", HeaderValue::from_static("600"));
            response.headers_mut().append(
                "vary",
                HeaderValue::from_static(
                    "Access-Control-Request-Method, Access-Control-Request-Headers",
                ),
            );
            response
        } else {
            self.handler.handle(request).await
        };
        response.headers_mut().insert(
            "access-control-allow-origin",
            if wildcard {
                HeaderValue::from_static("*")
            } else {
                origin
            },
        );
        response
            .headers_mut()
            .append("vary", HeaderValue::from_static("Origin"));
        if self.config.credentials {
            response.headers_mut().insert(
                "access-control-allow-credentials",
                HeaderValue::from_static("true"),
            );
        }
        response
    }
}
