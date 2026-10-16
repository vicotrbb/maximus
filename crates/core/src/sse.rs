use crate::{Body, Bytes, Error, HeaderValue, IntoResponse, Response, Result};
use futures_util::{Stream, StreamExt};

/// One SSE event. Metadata cannot inject additional fields.
pub struct Event {
    data: String,
    id: Option<String>,
    event: Option<String>,
    retry: Option<std::time::Duration>,
}
impl Event {
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            id: None,
            event: None,
            retry: None,
        }
    }
    pub fn id(mut self, id: impl Into<String>) -> Result<Self> {
        let id = id.into();
        validate(&id)?;
        self.id = Some(id);
        Ok(self)
    }
    pub fn event(mut self, event: impl Into<String>) -> Result<Self> {
        let event = event.into();
        validate(&event)?;
        self.event = Some(event);
        Ok(self)
    }
    pub fn retry(mut self, retry: std::time::Duration) -> Self {
        self.retry = Some(retry);
        self
    }
    fn encode(self) -> Bytes {
        let mut output = String::new();
        if let Some(id) = self.id {
            output.push_str("id:");
            output.push_str(&id);
            output.push('\n');
        }
        if let Some(event) = self.event {
            output.push_str("event:");
            output.push_str(&event);
            output.push('\n');
        }
        if let Some(retry) = self.retry {
            output.push_str(&format!("retry:{}\n", retry.as_millis()));
        }
        let data = self.data.replace("\r\n", "\n").replace('\r', "\n");
        for line in data.split('\n') {
            output.push_str("data:");
            output.push_str(line);
            output.push('\n');
        }
        output.push('\n');
        output.into()
    }
}
fn validate(value: &str) -> Result<()> {
    if value.contains(['\n', '\r', '\0']) {
        Err(Error::bad_request("invalid SSE metadata"))
    } else {
        Ok(())
    }
}
pub struct Sse<S>(S);
impl<S> Sse<S> {
    pub fn new(stream: S) -> Self {
        Self(stream)
    }
}
impl<S: Stream<Item = Result<Event>> + 'static> IntoResponse for Sse<S> {
    fn into_response(self) -> Response {
        let mut response =
            Response::new(Body::stream(self.0.map(|event| event.map(Event::encode))));
        response.headers_mut().insert(
            "content-type",
            HeaderValue::from_static("text/event-stream"),
        );
        response
            .headers_mut()
            .insert("cache-control", HeaderValue::from_static("no-cache"));
        response
            .headers_mut()
            .insert("x-accel-buffering", HeaderValue::from_static("no"));
        response
    }
}
