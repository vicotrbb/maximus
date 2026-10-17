use crate::{Error, Request, Result};
#[cfg(feature = "json")]
use crate::{IntoResponse, Response, StatusCode};
#[cfg(any(feature = "json", feature = "multipart"))]
use http_body_util::BodyExt;

#[cfg(feature = "json")]
pub struct Json<T>(pub T);
#[cfg(feature = "json")]
impl<T: serde::Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        match serde_json::to_vec(&self.0) {
            Ok(bytes) => {
                let mut response = Response::new(bytes.into());
                response.headers_mut().insert(
                    "content-type",
                    crate::HeaderValue::from_static("application/json"),
                );
                response
            }
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}
#[cfg(feature = "json")]
impl Request {
    pub async fn json<T: serde::de::DeserializeOwned>(self) -> Result<T> {
        let content_type = self.header_str("content-type")?;
        let media = content_type.split(';').next().unwrap_or("").trim();
        if !(media.eq_ignore_ascii_case("application/json")
            || media.starts_with("application/") && media.ends_with("+json"))
        {
            return Err(Error::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "expected JSON content type",
            ));
        }
        let bytes = self.into_body().collect().await?.to_bytes();
        serde_json::from_slice(&bytes).map_err(|_| Error::bad_request("invalid JSON body"))
    }
}
#[cfg(feature = "multipart")]
impl Request {
    /// Stream fields under both whole-upload and per-field byte limits.
    pub fn multipart(
        self,
        total_bytes: u64,
        field_bytes: u64,
    ) -> Result<multer::Multipart<'static>> {
        let content_type = self.header_str("content-type")?;
        let boundary = multer::parse_boundary(content_type)
            .map_err(|_| Error::bad_request("invalid multipart boundary"))?;
        let constraints = multer::Constraints::new().size_limit(
            multer::SizeLimit::new()
                .whole_stream(total_bytes)
                .per_field(field_bytes),
        );
        // Multer requires Send. All accesses remain on the request's originating worker.
        let stream = send_wrapper::SendWrapper::new(self.into_body().into_data_stream());
        Ok(multer::Multipart::with_constraints(
            stream,
            boundary,
            constraints,
        ))
    }
}
