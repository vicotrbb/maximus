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
    let mut gzip: Option<f32> = None;
    let mut wildcard: Option<f32> = None;
    for value in headers.get_all("accept-encoding") {
        let Ok(value) = value.to_str() else {
            continue;
        };
        for encoding in value.split(',') {
            let mut fields = encoding.split(';').map(str::trim);
            let name = fields.next().unwrap_or("");
            let quality = fields.find_map(|s| s.strip_prefix("q=")).map_or(1.0, |s| {
                s.parse::<f32>()
                    .ok()
                    .filter(|q| (0.0..=1.0).contains(q))
                    .unwrap_or(0.0)
            });
            let target = if name.eq_ignore_ascii_case("gzip") {
                &mut gzip
            } else if name == "*" {
                &mut wildcard
            } else {
                continue;
            };
            *target = Some(target.map_or(quality, |old| old.min(quality)));
        }
    }
    gzip.or(wildcard).unwrap_or(0.0) > 0.0
}
