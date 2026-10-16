#![cfg(feature = "opentelemetry")]
use http_body_util::BodyExt;
use maximus::{Body, Handler, Request};
use opentelemetry::trace::TracerProvider;
#[test]
fn telemetry_propagates_parent_and_finishes_after_response_body() {
    let exporter = opentelemetry_sdk::trace::InMemorySpanExporter::default();
    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    opentelemetry::global::set_text_map_propagator(
        opentelemetry_sdk::propagation::TraceContextPropagator::new(),
    );
    let app = maximus::telemetry(
        |_: Request| async { "traced" },
        provider.tracer("maximus-test"),
    );
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let request = Request::new(
            maximus::http::Request::builder()
                .method("GET")
                .uri("/secret?token=hidden")
                .header(
                    "traceparent",
                    "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
                )
                .body(Body::empty())
                .unwrap(),
        );
        let response = app.handle(request).await;
        assert!(exporter.get_finished_spans().unwrap().is_empty());
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            "traced"
        );
    });
    let spans = exporter.get_finished_spans().unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].parent_span_id.to_string(), "00f067aa0ba902b7");
    assert_eq!(
        spans[0].span_context.trace_id().to_string(),
        "4bf92f3577b34da6a3ce929d0e0e4736"
    );
    assert!(!format!("{:?}", spans[0].attributes).contains("hidden"));
}
