//! SSE metadata/retry construction and body consumption through each public API.
use http_body_util::BodyExt;
use maximus::IntoResponse;
use std::{
    convert::Infallible,
    hint::black_box,
    time::{Duration, Instant},
};
fn fields(bytes: &[u8]) -> Vec<(&str, &str)> {
    std::str::from_utf8(bytes)
        .unwrap()
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name, value.strip_prefix(' ').unwrap_or(value)))
        .collect()
}
fn main() {
    maximus_runtime::WorkerConfig::default()
        .build(0)
        .unwrap()
        .block_on(async {
            println!("engine,case,operations,ns_per_operation");
            for (name, metadata, retry, wire) in [
                ("retry", false, true, "retry:1234\ndata:hello\n\n"),
                (
                    "metadata",
                    true,
                    false,
                    "id:id\nevent:event\ndata:hello\n\n",
                ),
                (
                    "combined",
                    true,
                    true,
                    "id:id\nevent:event\nretry:1234\ndata:hello\n\n",
                ),
            ] {
                const N: usize = 200_000;
                let mut last = bytes::Bytes::new();
                let start = Instant::now();
                for _ in 0..N {
                    let mut event = maximus::Event::new(black_box("hello"));
                    if metadata {
                        event = event.id("id").unwrap().event("event").unwrap();
                    }
                    if retry {
                        event = event.retry(Duration::from_millis(black_box(1234)));
                    }
                    let response =
                        maximus::Sse::new(futures_util::stream::iter([Ok(event)])).into_response();
                    last = black_box(response.into_body().collect().await.unwrap().to_bytes());
                }
                let elapsed = start.elapsed();
                assert_eq!(last, wire);
                println!(
                    "maximus,{name},{N},{:.3}",
                    elapsed.as_nanos() as f64 / N as f64
                );
                let start = Instant::now();
                for _ in 0..N {
                    let response = hyper::Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("x-accel-buffering", "no")
                        .body(http_body_util::Full::new(bytes::Bytes::from_static(
                            black_box(wire.as_bytes()),
                        )))
                        .unwrap();
                    black_box(response.into_body().collect().await.unwrap().to_bytes());
                }
                println!(
                    "hyper-preencoded,{name},{N},{:.3}",
                    start.elapsed().as_nanos() as f64 / N as f64
                );
                use axum::response::IntoResponse as _;
                let start = Instant::now();
                for _ in 0..N {
                    let mut event = axum::response::sse::Event::default();
                    if metadata {
                        event = event.id("id").event("event");
                    }
                    if retry {
                        event = event.retry(Duration::from_millis(black_box(1234)));
                    }
                    event = event.data(black_box("hello"));
                    let response =
                        axum::response::Sse::new(futures_util::stream::iter([
                            Ok::<_, Infallible>(event),
                        ]))
                        .into_response();
                    last = black_box(response.into_body().collect().await.unwrap().to_bytes());
                }
                let elapsed = start.elapsed();
                assert_eq!(fields(&last), fields(wire.as_bytes()));
                println!(
                    "axum,{name},{N},{:.3}",
                    elapsed.as_nanos() as f64 / N as f64
                );
            }
        });
}
