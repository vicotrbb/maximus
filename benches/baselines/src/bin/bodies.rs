//! Body construction and complete consumption. No allocator instrumentation.
use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited, StreamBody};
use maximus::{Body, Frame, HttpBody};
use std::{convert::Infallible, fmt::Debug, hint::black_box, time::Instant};
const PAYLOAD: &[u8] = b"Hello, World!";

async fn case<B>(engine: &str, name: &str, make: impl Fn() -> B, expected: Option<&[u8]>)
where
    B: HttpBody<Data = Bytes>,
    B::Error: Debug,
{
    let check = |result: Result<Bytes, B::Error>| match expected {
        Some(bytes) => assert_eq!(result.unwrap(), bytes),
        None => assert!(result.is_err()),
    };
    check(make().collect().await.map(|body| body.to_bytes()));
    for _ in 0..1000 {
        let _ = black_box(make().collect().await);
    }
    const N: usize = 500_000;
    let started = Instant::now();
    for _ in 0..N {
        let _ = black_box(
            black_box(make())
                .collect()
                .await
                .map(|body| body.to_bytes()),
        );
    }
    println!(
        "{engine},{name},{N},{:.3}",
        started.elapsed().as_nanos() as f64 / N as f64
    );
}
fn main() {
    if std::env::args().any(|arg| arg == "--layout") {
        println!(
            "{{\"Body\":{},\"Request\":{},\"Response\":{}}}",
            std::mem::size_of::<Body>(),
            std::mem::size_of::<maximus::Request>(),
            std::mem::size_of::<maximus::Response>()
        );
        return;
    }
    maximus_runtime::WorkerConfig::default()
        .build(0)
        .unwrap()
        .block_on(async {
            println!("engine,case,operations,ns_per_operation");
            case("maximus", "empty", Body::empty, Some(b"")).await;
            case("hyper", "empty", || Full::new(Bytes::new()), Some(b"")).await;
            case("axum", "empty", axum::body::Body::empty, Some(b"")).await;
            case(
                "maximus",
                "buffered",
                || Body::from(Bytes::from_static(PAYLOAD)),
                Some(PAYLOAD),
            )
            .await;
            case(
                "hyper",
                "buffered",
                || Full::new(Bytes::from_static(PAYLOAD)),
                Some(PAYLOAD),
            )
            .await;
            case(
                "axum",
                "buffered",
                || axum::body::Body::from(Bytes::from_static(PAYLOAD)),
                Some(PAYLOAD),
            )
            .await;
            for (name, limit, expected) in [
                ("limited-pass", 13, Some(PAYLOAD)),
                ("limited-reject", 12, None),
            ] {
                case(
                    "maximus",
                    name,
                    || Body::from(Bytes::from_static(PAYLOAD)).limited(limit),
                    expected,
                )
                .await;
                case(
                    "hyper",
                    name,
                    || Limited::new(Full::new(Bytes::from_static(PAYLOAD)), limit),
                    expected,
                )
                .await;
                case(
                    "axum",
                    name,
                    || Limited::new(axum::body::Body::from(Bytes::from_static(PAYLOAD)), limit),
                    expected,
                )
                .await;
            }
            case(
                "maximus",
                "stream",
                || {
                    Body::stream(futures_util::stream::iter([
                        Ok(Bytes::from_static(b"Hello, ")),
                        Ok(Bytes::from_static(b"World!")),
                    ]))
                },
                Some(PAYLOAD),
            )
            .await;
            case(
                "hyper",
                "stream",
                || {
                    StreamBody::new(futures_util::stream::iter([
                        Ok::<_, Infallible>(Frame::data(Bytes::from_static(b"Hello, "))),
                        Ok(Frame::data(Bytes::from_static(b"World!"))),
                    ]))
                },
                Some(PAYLOAD),
            )
            .await;
            case(
                "axum",
                "stream",
                || {
                    axum::body::Body::from_stream(futures_util::stream::iter([
                        Ok::<_, Infallible>(Bytes::from_static(b"Hello, ")),
                        Ok(Bytes::from_static(b"World!")),
                    ]))
                },
                Some(PAYLOAD),
            )
            .await;
            case(
                "maximus",
                "stream-reject",
                || {
                    Body::stream(futures_util::stream::iter([
                        Ok(Bytes::from_static(b"Hello, ")),
                        Ok(Bytes::from_static(b"World!")),
                    ]))
                    .limited(12)
                },
                None,
            )
            .await;
            case(
                "hyper",
                "stream-reject",
                || {
                    Limited::new(
                        StreamBody::new(futures_util::stream::iter([
                            Ok::<_, Infallible>(Frame::data(Bytes::from_static(b"Hello, "))),
                            Ok(Frame::data(Bytes::from_static(b"World!"))),
                        ])),
                        12,
                    )
                },
                None,
            )
            .await;
            case(
                "axum",
                "stream-reject",
                || {
                    Limited::new(
                        axum::body::Body::from_stream(futures_util::stream::iter([
                            Ok::<_, Infallible>(Bytes::from_static(b"Hello, ")),
                            Ok(Bytes::from_static(b"World!")),
                        ])),
                        12,
                    )
                },
                None,
            )
            .await;
        });
}
