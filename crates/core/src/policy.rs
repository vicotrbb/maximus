use crate::{
    Body, Error, Handler, IntoResponse, Limits, Request, Response, Result, StatusCode, decode, http,
};
use http_body_util::BodyExt;
use std::rc::Rc;
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

pub async fn dispatch_request<H: Handler>(
    request: Request,
    handler: Rc<H>,
    limits: Rc<Limits>,
) -> Response {
    let head = request.method() == http::Method::HEAD;
    suppress_body(dispatch_inner(request, handler, limits).await, head)
}

async fn dispatch_inner<H: Handler>(
    mut request: Request,
    handler: Rc<H>,
    limits: Rc<Limits>,
) -> Response {
    if !(request.method() == http::Method::OPTIONS && request.uri().path() == "*"
        || request.method() == http::Method::CONNECT && request.uri().authority().is_some())
        && let Err(error) = validate_path(request.uri().path())
    {
        return error.into_response();
    }
    let hosts = request.headers().get_all("host").iter().count();
    if hosts > 1 || request.version() == http::Version::HTTP_11 && hosts != 1 {
        return (
            StatusCode::BAD_REQUEST,
            "exactly one Host header is required",
        )
            .into_response();
    }
    if request
        .uri()
        .authority()
        .is_some_and(|authority| !valid_authority(authority.as_str()))
        || request.headers().get("host").is_some_and(|value| {
            value
                .to_str()
                .ok()
                .is_none_or(|host| !valid_authority(host))
        })
    {
        return (StatusCode::BAD_REQUEST, "invalid Host authority").into_response();
    }
    match content_length(request.headers()) {
        Err(error) => return error.into_response(),
        Ok(Some(length)) if length > limits.body_bytes as u64 => {
            return StatusCode::PAYLOAD_TOO_LARGE.into_response();
        }
        _ => {}
    }
    if request.headers().len() > limits.headers
        || request.headers().iter().fold(0usize, |sum, (name, value)| {
            sum.saturating_add(name.as_str().len() + value.len() + 4)
        }) > limits.header_bytes
    {
        return StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE.into_response();
    }
    // RFC 9113 §8.2.3 / RFC 9114 §4.2.1: generic handlers receive one Cookie field.
    if matches!(
        request.version(),
        http::Version::HTTP_2 | http::Version::HTTP_3
    ) && request.headers().get_all("cookie").iter().count() > 1
    {
        let mut combined = Vec::new();
        for (index, value) in request.headers().get_all("cookie").iter().enumerate() {
            if index != 0 {
                combined.extend_from_slice(b"; ");
            }
            combined.extend_from_slice(value.as_bytes());
        }
        let mut value = crate::HeaderValue::from_bytes(&combined)
            .expect("valid Cookie fields joined with a valid delimiter");
        value.set_sensitive(true);
        request.headers_mut().insert("cookie", value);
    }
    match compio::time::timeout(limits.request_timeout, handler.handle(request)).await {
        Ok(response) => response.map(|body| {
            if body.is_buffered() {
                return body;
            }
            let timeout = limits.request_timeout;
            Body::frames(futures_util::stream::unfold(
                Some(body),
                move |state| async move {
                    let mut body = state?;
                    match compio::time::timeout(timeout, body.frame()).await {
                        Ok(Some(frame)) => Some((frame, Some(body))),
                        Ok(None) => None,
                        Err(_) => Some((
                            Err(crate::Error::new(
                                StatusCode::REQUEST_TIMEOUT,
                                "response stream stalled",
                            )),
                            None,
                        )),
                    }
                },
            ))
        }),
        Err(_) => StatusCode::REQUEST_TIMEOUT.into_response(),
    }
}

/// Strict shared Content-Length validation; wire engines may already have normalized duplicates.
pub fn content_length(
    headers: &crate::HeaderMap,
) -> std::result::Result<Option<u64>, crate::Error> {
    let mut values = headers.get_all("content-length").iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some()
        || value.is_empty()
        || !value.as_bytes().iter().all(u8::is_ascii_digit)
    {
        return Err(crate::Error::bad_request("invalid Content-Length"));
    }
    value
        .to_str()
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Some)
        .ok_or_else(|| crate::Error::bad_request("invalid Content-Length"))
}

fn valid_authority(value: &str) -> bool {
    if value.is_empty()
        || value.contains('@')
        || value.parse::<crate::http::uri::Authority>().is_err()
    {
        return false;
    }
    let port = if let Some(literal) = value.strip_prefix('[') {
        let Some((address, suffix)) = literal.split_once(']') else {
            return false;
        };
        let future = address
            .strip_prefix(['v', 'V'])
            .and_then(|s| s.split_once('.'))
            .is_some_and(|(version, name)| {
                !version.is_empty()
                    && version.bytes().all(|b| b.is_ascii_hexdigit())
                    && !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:".contains(&b))
            });
        if address.parse::<std::net::Ipv6Addr>().is_err() && !future {
            return false;
        }
        if suffix.is_empty() {
            None
        } else {
            Some(match suffix.strip_prefix(':') {
                Some(port) => port,
                None => return false,
            })
        }
    } else {
        value.split_once(':').map(|(_, port)| port)
    };
    port.is_none_or(|port| port.bytes().all(|b| b.is_ascii_digit()))
}

/// Apply method/status body semantics consistently across all wire engines.
pub fn suppress_body(mut response: Response, head: bool) -> Response {
    use crate::HttpBody;
    if head
        || response.status().is_informational()
        || response.status() == StatusCode::RESET_CONTENT
        || response.status() == StatusCode::NO_CONTENT
        || response.status() == StatusCode::NOT_MODIFIED
    {
        if head
            && !response.headers().contains_key("content-length")
            && let Some(size) = response.body().size_hint().exact()
        {
            response.headers_mut().insert("content-length", size.into());
        }
        if response.status().is_informational() || response.status() == StatusCode::NO_CONTENT {
            response.headers_mut().remove("content-length");
        }
        if response.status() == StatusCode::RESET_CONTENT {
            response
                .headers_mut()
                .insert("content-length", http::HeaderValue::from_static("0"));
        }
        *response.body_mut() = Body::empty();
    }
    response
}
