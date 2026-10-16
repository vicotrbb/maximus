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
    let metrics = opentelemetry_sdk::metrics::InMemoryMetricExporter::default();
    let meter_provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
        .with_reader(opentelemetry_sdk::metrics::PeriodicReader::builder(metrics.clone()).build())
        .build();
    opentelemetry::global::set_meter_provider(meter_provider.clone());
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
    meter_provider.force_flush().unwrap();
    use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
    let snapshots = metrics.get_finished_metrics().unwrap();
    let measured: Vec<_> = snapshots
        .iter()
        .flat_map(|r| r.scope_metrics())
        .flat_map(|s| s.metrics())
        .collect();
    let active = measured
        .iter()
        .find(|m| m.name() == "http.server.active_requests")
        .unwrap();
    let AggregatedMetrics::I64(MetricData::Sum(active)) = active.data() else {
        panic!("active counter type");
    };
    assert!(active.data_points().all(|p| p.value() == 0));
    let duration = measured
        .iter()
        .find(|m| m.name() == "http.server.request.duration")
        .unwrap();
    let AggregatedMetrics::F64(MetricData::Histogram(duration)) = duration.data() else {
        panic!("duration histogram type");
    };
    assert_eq!(duration.data_points().map(|p| p.count()).sum::<u64>(), 1);
    meter_provider.shutdown().unwrap();
    telemetry_ends_cancelled_handler_and_stream_spans();
}

fn telemetry_ends_cancelled_handler_and_stream_spans() {
    use opentelemetry::trace::Status;
    let exporter = opentelemetry_sdk::trace::InMemorySpanExporter::default();
    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    compio::runtime::Runtime::new().unwrap().block_on(async {
        let stalled = maximus::telemetry(
            |_: Request| std::future::pending::<Body>(),
            provider.tracer("cancel-test"),
        );
        assert!(
            compio::time::timeout(
                std::time::Duration::from_millis(5),
                stalled.handle(Request::new(maximus::http::Request::new(Body::empty())))
            )
            .await
            .is_err()
        );
        let streamed = maximus::telemetry(
            |_: Request| async { Body::stream(futures_util::stream::pending()) },
            provider.tracer("cancel-test"),
        );
        drop(
            streamed
                .handle(Request::new(maximus::http::Request::new(Body::empty())))
                .await,
        );
    });
    let spans = exporter.get_finished_spans().unwrap();
    assert_eq!(spans.len(), 2);
    assert!(
        spans
            .iter()
            .all(|span| matches!(span.status, Status::Error { .. })),
        "{spans:?}"
    );
}
