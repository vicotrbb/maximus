use futures_util::Stream;
use maximus_core::{Body, Bytes, Error, Frame, Handler, HeaderMap, HttpBody, Request, Response};
use opentelemetry::{
    Context, KeyValue, global,
    metrics::{Histogram, UpDownCounter},
    propagation::Extractor,
    trace::{FutureExt, SpanKind, Status, TraceContextExt, Tracer},
};
use std::{
    pin::Pin,
    task::{Context as TaskContext, Poll},
    time::Instant,
};

/// Uses the supplied tracer and the globally configured meter and text-map propagator.
/// Spans and duration measurements finish when the response body finishes or is cancelled.
pub fn telemetry<H: Handler, T: Tracer + 'static>(handler: H, tracer: T) -> impl Handler
where
    T::Span: Send + Sync + 'static,
{
    let meter = global::meter("maximus");
    Instrumented {
        handler,
        tracer,
        duration: meter
            .f64_histogram("http.server.request.duration")
            .with_unit("s")
            .build(),
        active: meter
            .i64_up_down_counter("http.server.active_requests")
            .build(),
    }
}
struct Instrumented<H, T> {
    handler: H,
    tracer: T,
    duration: Histogram<f64>,
    active: UpDownCounter<i64>,
}
impl<H: Handler, T: Tracer + 'static> Handler for Instrumented<H, T>
where
    T::Span: Send + Sync + 'static,
{
    async fn handle(&self, request: Request) -> Response {
        let attributes = vec![KeyValue::new(
            "http.request.method",
            request.method().to_string(),
        )];
        let parent = global::get_text_map_propagator(|p| p.extract(&Headers(request.headers())));
        let span = self
            .tracer
            .span_builder("HTTP request")
            .with_kind(SpanKind::Server)
            .with_attributes(attributes.clone())
            .start_with_context(&self.tracer, &parent);
        let context = parent.with_span(span);
        self.active.add(1, &attributes);
        let mut body = TracedBody {
            body: Body::empty(),
            context: context.clone(),
            started: Instant::now(),
            duration: self.duration.clone(),
            active: self.active.clone(),
            attributes,
            responded: false,
            finished: false,
        };
        let response = self
            .handler
            .handle(request)
            .with_context(context.clone())
            .await;
        let status = response.status();
        context.span().set_attribute(KeyValue::new(
            "http.response.status_code",
            status.as_u16() as i64,
        ));
        if status.is_server_error() {
            context.span().set_status(Status::error("server error"));
        }
        body.attributes.push(KeyValue::new(
            "http.response.status_code",
            status.as_u16() as i64,
        ));
        let (parts, inner) = response.into_parts();
        body.body = inner;
        body.responded = true;
        Response::from_parts(parts, Body::frames(body))
    }
}
struct Headers<'a>(&'a HeaderMap);
impl Extractor for Headers<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.to_str().ok()
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|key| key.as_str()).collect()
    }
}
struct TracedBody {
    body: Body,
    context: Context,
    started: Instant,
    duration: Histogram<f64>,
    active: UpDownCounter<i64>,
    attributes: Vec<KeyValue>,
    responded: bool,
    finished: bool,
}
impl TracedBody {
    fn finish(&mut self, cancelled: bool) {
        if self.finished {
            return;
        }
        self.finished = true;
        if cancelled {
            self.context
                .span()
                .set_status(Status::error("response cancelled"));
        }
        self.context.span().end();
        self.duration
            .record(self.started.elapsed().as_secs_f64(), &self.attributes);
        self.active.add(-1, &self.attributes[..1]);
    }
}
impl Stream for TracedBody {
    type Item = Result<Frame<Bytes>, Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        let result = Pin::new(&mut self.body).poll_frame(cx);
        match &result {
            Poll::Ready(None) => self.finish(false),
            Poll::Ready(Some(Err(_))) => self.finish(true),
            _ => {}
        }
        result
    }
}
impl Drop for TracedBody {
    fn drop(&mut self) {
        self.finish(!self.responded || !self.body.is_end_stream());
    }
}
