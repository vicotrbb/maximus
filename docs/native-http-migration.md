# Native HTTP migration

Version 0.2 changes low-level integration APIs from the original Hyper-backed 0.1 design. Application handlers, routes, `Request`, fixed and streaming responses, extraction, middleware and `Server` configuration retain their interfaces. Existing applications using those APIs need no adapter changes. This repository version does not imply a published release.

| Previous low-level API | Native API |
| --- | --- |
| `Body::incoming(hyper::body::Incoming, limit)` | `Body::incoming(stream, limit)`, where the local stream yields `Result<Frame<Bytes>, maximus_core::Error>` |
| HTTP engine `serve` with futures I/O | `serve_futures` for futures I/O; `serve` accepts Compio owned-buffer I/O with independent read/write halves |
| HTTP engine `hyper::Error` return | `std::io::Error` |
| HTTP/1 `adapt`, `Executor`, `Timer`, Hyper-specific `dispatch` | Removed; only independent test clients keep their own Hyper adapters |
| Hyper upgrade future / concrete WebSocket transport | Owned `maximus_http1::Upgraded`, implementing futures `AsyncRead` and `AsyncWrite`; `UpgradeSlot::register` receives this transport in its callback |
| Shared HTTP validation imported from HTTP/1 | `maximus_core::{dispatch_request, content_length, suppress_body, validate_path}`; existing path/dispatch re-exports remain |

HTTP/1 now rejects TE+CL requests before dispatch. Previously the Hyper integration used Transfer-Encoding and closed after responding. Repeated Content-Length fields are rejected even when equal. Unconsumed HTTP/1 bodies close the connection. Header/body limits and deadlines remain active.

HPACK decoding supports Huffman strings and dynamic tables. The encoder uses static references and never-indexed literals with its dynamic table disabled. This interoperates with peer table-size settings, but does not provide dynamic or Huffman compression of response fields. Compression ratio and throughput should be assessed separately.

Hyper, hyper-util, tokio-util and h2 remain available only to independent clients and benchmark servers. `cargo tree -p maximus --all-features --edges normal` shows the product graph. HTTP/3 still uses Tokio transitively through the retained h3 dependency; no Tokio executor is started by Maximus. TLS and WebSocket paths retain Compio/futures compatibility I/O.

HTTP/1 CONNECT can hand off a tunnel only through a registered `UpgradeSlot`; otherwise a successful CONNECT handler result becomes 501. HTTP/2 CONNECT tunneling and extended CONNECT are not implemented and return 501. Handler-produced informational final responses are rejected rather than emitting an incomplete HTTP exchange. Automatic HTTP/1 100-continue remains supported.

HTTP/2 minimally processes late frames on closed streams: HPACK state and connection flow control remain synchronized, without reopening the stream. This follows RFC 9113 §5.1, including its permission to use the policy for all closed streams. Older RFC 7540 conformance tools may expect connection errors instead. The conformance fixture consumes full request bodies before responding so early-response cancellation does not mask malformed live-stream checks.
