//! SSE construction and complete body consumption, without network or allocator hooks.
use http_body_util::BodyExt;
use maximus::IntoResponse;
use std::{convert::Infallible, hint::black_box, time::Instant};

fn main() {
    maximus_runtime::WorkerConfig::default()
        .build(0)
        .unwrap()
        .block_on(async {
            println!("engine,case,operations,ns_per_operation");
            for (name, text, expected) in [
                ("empty", "", "data:\n\n"),
                ("single", "hello", "data:hello\n\n"),
                (
                    "multiline",
                    "one\ntwo\nthree",
                    "data:one\ndata:two\ndata:three\n\n",
                ),
                (
                    "crlf",
                    "one\r\ntwo\rthree\n",
                    "data:one\ndata:two\ndata:three\ndata:\n\n",
                ),
            ] {
                const N: usize = 200_000;
                let mut last = bytes::Bytes::new();
                let started = Instant::now();
                for _ in 0..N {
                    let event = maximus::Event::new(black_box(text));
                    let response =
                        maximus::Sse::new(futures_util::stream::iter([Ok(event)])).into_response();
                    let bytes = response.into_body().collect().await.unwrap().to_bytes();
                    last = black_box(bytes);
                }
                let elapsed = started.elapsed();
                assert_eq!(last, expected);
                println!(
                    "maximus,{name},{N},{:.3}",
                    elapsed.as_nanos() as f64 / N as f64
                );
                // Hyper is the lower-level pre-encoded response reference.
                let started = Instant::now();
                for _ in 0..N {
                    let response = hyper::Response::builder()
                        .header("content-type", "text/event-stream")
                        .header("cache-control", "no-cache")
                        .header("x-accel-buffering", "no")
                        .body(http_body_util::Full::new(bytes::Bytes::from_static(
                            black_box(expected.as_bytes()),
                        )))
                        .unwrap();
                    black_box(response.into_body().collect().await.unwrap().to_bytes());
                }
                println!(
                    "hyper-preencoded,{name},{N},{:.3}",
                    started.elapsed().as_nanos() as f64 / N as f64
                );
                // Axum rejects CR in data; compare its native API only on accepted input.
                if !text.is_empty() && !text.contains('\r') {
                    use axum::response::IntoResponse as _;
                    let started = Instant::now();
                    for _ in 0..N {
                        let event = axum::response::sse::Event::default().data(black_box(text));
                        let response =
                            axum::response::Sse::new(futures_util::stream::iter([Ok::<
                                _,
                                Infallible,
                            >(
                                event
                            )]))
                            .into_response();
                        let bytes = response.into_body().collect().await.unwrap().to_bytes();
                        last = black_box(bytes);
                    }
                    let elapsed = started.elapsed();
                    // Axum inserts one optional space after each field colon.
                    let decoded = std::str::from_utf8(&last)
                        .unwrap()
                        .replace("data: ", "data:");
                    assert_eq!(decoded, expected);
                    println!(
                        "axum,{name},{N},{:.3}",
                        elapsed.as_nanos() as f64 / N as f64
                    );
                }
            }
        });
}
