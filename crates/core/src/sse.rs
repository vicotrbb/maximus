use crate::{Body, Bytes, Error, HeaderValue, IntoResponse, Response, Result};
use futures_util::{Stream, StreamExt};
use std::fmt::Write;

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
            field(&mut output, "id:", &id);
        }
        if let Some(event) = self.event {
            field(&mut output, "event:", &event);
        }
        if let Some(retry) = self.retry {
            writeln!(output, "retry:{}", retry.as_millis()).expect("writing to String");
        }
        for (index, part) in self.data.split('\r').enumerate() {
            // A CRLF is one separator. Bare CR and LF each start a new line.
            let part = if index == 0 {
                part
            } else {
                part.strip_prefix('\n').unwrap_or(part)
            };
            for line in part.split('\n') {
                field(&mut output, "data:", line);
            }
        }
        output.push('\n');
        output.into()
    }
}
fn field(output: &mut String, prefix: &str, value: &str) {
    output.push_str(prefix);
    // SSE parsers remove one optional space after the colon.
    if value.starts_with(' ') {
        output.push(' ');
    }
    output.push_str(value);
    output.push('\n');
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
