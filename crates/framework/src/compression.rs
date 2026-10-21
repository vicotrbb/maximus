use maximus_core::{Body, Handler, HeaderMap, HeaderValue, Request, Response};
use std::io::Write;

/// Gzip for bounded, fully buffered responses. Streaming bodies retain their backpressure and trailers.
#[derive(Clone, Copy)]
pub struct Compression {
    pub minimum_bytes: usize,
    pub maximum_bytes: usize,
    pub level: u32,
}
impl Default for Compression {
    fn default() -> Self {
        Self {
            minimum_bytes: 1024,
            maximum_bytes: 1024 * 1024,
            level: 1,
        }
    }
}
impl Compression {
    pub fn wrap<H: Handler>(self, handler: H) -> impl Handler {
        Compressed {
            config: self,
            handler,
        }
    }
}
struct Compressed<H> {
    config: Compression,
    handler: H,
}
impl<H: Handler> Handler for Compressed<H> {
    async fn handle(&self, request: Request) -> Response {
        let accepts = accepts_gzip(request.headers()) && !request.headers().contains_key("range");
        let mut response = self.handler.handle(request).await;
        response
            .headers_mut()
            .append("vary", HeaderValue::from_static("Accept-Encoding"));
        if !accepts
            || response.headers().contains_key("content-encoding")
            || response.headers().contains_key("content-range")
            || response.headers().get_all("cache-control").iter().any(|v| {
                v.to_str().is_ok_and(|s| {
                    s.split(',')
                        .any(|s| s.trim().eq_ignore_ascii_case("no-transform"))
                })
            })
        {
            return response;
        }
        let Some(bytes) = response.body_mut().take_bytes() else {
            return response;
        };
        if bytes.len() < self.config.minimum_bytes || bytes.len() > self.config.maximum_bytes {
            *response.body_mut() = Body::from(bytes);
            return response;
        }
        let mut encoder = flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::new(self.config.level.min(9)),
        );
        let compressed = encoder.write_all(&bytes).and_then(|()| encoder.finish());
        if let Ok(compressed) = compressed
            && compressed.len() < bytes.len()
        {
            response
                .headers_mut()
                .insert("content-encoding", HeaderValue::from_static("gzip"));
            response.headers_mut().remove("content-length");
            response.headers_mut().remove("content-md5");
            response.headers_mut().remove("digest");
            if let Some(etag) = response.headers_mut().remove("etag")
                && let Ok(value) = etag.to_str()
            {
                let weak = if value.starts_with("W/") {
                    value.to_owned()
                } else {
                    format!("W/{value}")
                };
                if let Ok(weak) = weak.parse() {
                    response.headers_mut().insert("etag", weak);
                }
            }
            *response.body_mut() = compressed.into();
        } else {
            *response.body_mut() = bytes.into();
        }
        response
    }
}
fn accepts_gzip(headers: &HeaderMap) -> bool {
    let mut gzip: Option<bool> = None;
    let mut wildcard: Option<bool> = None;
    for value in headers.get_all("accept-encoding") {
        let Ok(value) = value.to_str() else {
            continue;
        };
        for encoding in value.split(',') {
            let mut fields = encoding.split(';').map(str::trim);
            let name = fields.next().unwrap_or("");
            let target = if name.eq_ignore_ascii_case("gzip") {
                &mut gzip
            } else if name == "*" {
                &mut wildcard
            } else {
                continue;
            };
            // A qvalue only decides whether this coding is acceptable. Repeated
            // entries preserve the existing zero-veto policy.
            let accepted = fields
                .find_map(|field| {
                    let (name, value) = field.split_once('=')?;
                    name.eq_ignore_ascii_case("q").then_some(value)
                })
                .is_none_or(positive_quality);
            *target = Some(target.unwrap_or(true) && accepted);
        }
    }
    gzip.or(wildcard).unwrap_or(false)
}

// RFC 9110 section 12.4.2: reject malformed weights rather than recovering
// signs, exponents or excess precision through a floating-point parser.
fn positive_quality(value: &str) -> bool {
    match value.as_bytes() {
        b"1" => true,
        [b'1', b'.', fraction @ ..] => {
            fraction.len() <= 3 && fraction.iter().all(|&digit| digit == b'0')
        }
        [b'0', b'.', fraction @ ..] => {
            fraction.len() <= 3
                && fraction.iter().all(u8::is_ascii_digit)
                && fraction.iter().any(|&digit| digit != b'0')
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiation_preserves_explicit_zero_and_repeated_field_policy() {
        for (values, expected) in [
            (vec!["gzip"], true),
            (vec!["GZIP;Q=0"], false),
            (vec!["gzip;q=0.001"], true),
            (vec!["*;q=1, gzip;q=0"], false),
            (vec!["gzip;q=0", "*;q=1"], false),
            (vec!["gzip;q=1", "gzip;q=0"], false),
            (vec!["gzip;q=0", "gzip;q=1"], false),
            (vec!["br;q=0, *;q=0.5"], true),
            (vec!["br;q=1"], false),
            (vec![""], false),
        ] {
            let mut headers = HeaderMap::new();
            for value in &values {
                headers.append("accept-encoding", HeaderValue::from_str(value).unwrap());
            }
            assert_eq!(accepts_gzip(&headers), expected, "{values:?}");
        }
    }

    #[test]
    fn quality_uses_http_decimal_grammar() {
        for value in ["1", "1.", "1.0", "1.00", "1.000", "0.1", "0.001", "0.999"] {
            assert!(positive_quality(value), "{value}");
        }
        for value in [
            "0", "0.", "0.0", "0.000", "", ".5", "+0.5", "1e0", "NaN", "1.001", "0.0001", "1.0000",
            "2", "-0.1",
        ] {
            assert!(!positive_quality(value), "{value}");
        }
        // Exhaust every three-digit fraction, including the smallest nonzero q.
        for fraction in 0..1000 {
            assert_eq!(positive_quality(&format!("0.{fraction:03}")), fraction != 0);
            assert_eq!(positive_quality(&format!("1.{fraction:03}")), fraction == 0);
        }
    }
}
