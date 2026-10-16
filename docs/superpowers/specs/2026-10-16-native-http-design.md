# Native HTTP engines and measured optimization

Status: scope confirmed by the user: own HTTP/1.1 and HTTP/2; retain Rustls and h3/Quinn. Native implementation is frozen at `8a8fa9b`; correctness qualification is recorded in [native HTTP qualification](../../native-http-qualification.md). Comparative qualification is complete in the [native comparison report](../../../benches/results/native-http-comparison.md), including retained failed load targets and the HTTP/2 regression. Baseline: `9cb096a`, whose measured product source is `2f2f798`.

## Intent and boundaries

The requested outcome is ownership of HTTP execution without Hyper and Tokio compatibility utilities, better measured performance, retained compatibility, and tests of every optimization opportunity identified in the preceding analysis. Correctness and existing capabilities remain requirements. Perfect performance and exhaustive compatibility are not promises a finite qualification can establish.

Confirmed boundary:

- Own HTTP/1.1 parsing, framing, serialization and connection lifecycle.
- Own HTTP/2 framing, stream state, HPACK, flow control and scheduling. Replacing Hyper with the existing `h2` engine would not meet this ownership objective.
- Retain Compio for execution and kernel I/O, Rustls for TLS, and the existing h3/QUIC implementation for HTTP/3. Retain the existing WebSocket codec while replacing Hyper's upgrade handoff.
- Retain `http`, `http-body`, `bytes`, Serde and feature integrations as useful types/utilities. These are explicit dependencies, not claimed as Maximus-owned implementations.
- Remove product dependencies on `hyper`, `hyper-util` and `tokio-util`. Hyper/Tokio remain permissible in isolated benchmarks and independent test clients.
- Do not claim that the full product is Tokio-free: the current retained `h3` dependency uses Tokio transitively. Full removal would require an additional HTTP/3 dependency change.

Alternatives are an HTTP/1-only feasibility prototype before committing to HTTP/2, or ownership of HTTP/3/QPACK as well. The former limits initial investment; the latter materially expands protocol maintenance and qualification. Reimplementing cryptography or QUIC is not part of this recommendation.

## Architecture

Keep the twelve existing crates. Replace the HTTP engines inside their current crate boundaries; avoid a parallel framework or a permanent fallback engine. Git retains the original implementation for comparison.

Move shared request validation, response semantics and handler deadlines out of the HTTP/1 adapter into a protocol-neutral module in `core`. HTTP/1, HTTP/2 and HTTP/3 must use that same policy. Preserve strict authority, path, header, length and bodyless-response checks.

Replace `Body::Incoming(hyper::body::Incoming)` with a bounded protocol-neutral incoming-body representation. Fixed responses retain the existing inline body representation. A connection owns its parser, reusable input/output buffers and in-flight operations. Streaming backpressure must suspend reads or withhold receive credit instead of accumulating unbounded bodies. Dropping a request body must trigger a defined drain/close or stream-reset decision; never reuse unread body bytes as another request.

Plain TCP HTTP engines should use Compio's owned-buffer completion interface directly. TLS retains Rustls; its existing futures-compatible transport can initially remain behind a separate, explicit transport implementation. Removing Hyper does not automatically remove the Compio compatibility buffer used by TLS. Benchmark and report plaintext and TLS independently.

Keep socket-operation buffers alive until completion, including cancellation and shutdown. Preserve partial reads/writes, buffered bytes across message boundaries, and bounded output under stalled consumers. Prefer safe Rust; any unavoidable unsafe block needs a stated lifetime invariant and focused tests.

WebSocket handoff transfers the transport and every byte read beyond the HTTP headers exactly once, after the upgrade response is flushed. Keep the upgraded task inside connection admission, idle and shutdown accounting. Do not lose a first WebSocket frame coalesced with the handshake.

## Protocol compatibility contract

HTTP/1 must cover incremental parsing across arbitrary byte boundaries, request-target forms, HTTP/1.0 compatibility policy, HTTP/1.1 keepalive, ordered pipelining, fixed/chunked request bodies, chunk extensions, trailers, Expect/100-continue, streaming responses and upgrade handoff. Resolve framing ambiguity conservatively, including TE/CL combinations, duplicate lengths, malformed transfer codings, truncated bodies and response length mismatches. Preserve HEAD and status-specific body rules, Date handling, connection-close semantics and deadlines.

HTTP/2 must cover the connection preface, SETTINGS/ACK and negotiated bounds, all core frame handling, stream-ID/state validation, HEADERS/CONTINUATION ordering, complete HPACK decoding including Huffman and dynamic-table changes, request pseudo-header rules, trailers, flow-control accounting and overflow, WINDOW_UPDATE, cancellation/RST_STREAM, PING, GOAWAY and bounded graceful drain. A simple conforming encoder is acceptable; its compression choices must be disclosed in comparative tests. Bound compressed-header work, decoded-header size, stream churn and control-frame processing. Optional server push need not be emitted; unsupported extensions must follow the protocol's defined handling.

Compatibility includes the existing handler/extractor/middleware examples, TLS ALPN, HTTP/3 policy behavior and all feature combinations. Public APIs currently exposing Hyper types or errors cannot silently disappear: inventory them, preserve source compatibility where practical, and document unavoidable changes as a versioned migration. `Body::incoming`, HTTP engine error types, the WebSocket concrete alias and exported Hyper executor/timer helpers require review.

## Experiments before and during replacement

Each experiment changes one factor, retains the control result and records negative results. Temporary dependency patches, profilers and duplicate baseline checkouts stay outside tracked product code.

| Opportunity | Controlled experiment | Evidence required |
| --- | --- | --- |
| Allocating WouldBlock errors | Patch only Compio's internal retry-error construction in the old baseline | Allocation call sites/counts; response validity; throughput, CPU and tails |
| Compatibility adapter costs | Compare old baseline, patched baseline and native HTTP with identical policies | Copies/allocations, syscalls, CPU profiles, protocol and cancellation tests |
| Validation/deadlines | Add equivalent policy to a diagnostic Hyper baseline; measure validation and timer costs separately | No weakened validation or deadline semantics; malformed/slow-peer regression suite |
| Worker placement | Compare process affinity with distinct per-worker pinning | Migrations, context switches, per-worker traffic, one/two/four-worker throughput and tails |
| Retained memory | Profile allocation/free lifetimes over repeated 20,000-connection cycles | Live allocation bytes versus retained RSS, descriptor recovery, attribution before selecting pools/allocator |
| Buffer/write strategies | Sweep bounded buffer sizes and write aggregation for plaintext and TLS separately | Small/large bodies, streaming latency, fairness and maximum buffered memory |

The previous timeout experiment failed to demonstrate improvement. Do not present that rejected change as a new gain. Kernel zero-copy was slower in the recorded primitive test; retain that finding and require a representative workload benefit before promotion.

## Qualification and acceptance

1. Write behavior tests before implementation. Exercise fragmented inputs, coalesced messages, partial writes, EOF at every framing boundary, backpressure, cancellation and shutdown.
2. Run the existing host/Linux feature matrix, independent HTTPX/h2/aioquic/WebSocket clients and adversarial wire tests against the replacement. Keep all existing capabilities passing.
3. Add RFC-focused HTTP/1 and HTTP/2 cases, HPACK specification vectors and decoder fuzz targets. Use independent implementations as differential oracles, with the RFC resolving disagreements.
4. Run h2spec as additional evidence, not a complete RFC 9113 certificate: its advertised coverage is RFC 7540/7541. Add explicit cases for current requirements and protocol abuse that it does not cover.
5. Compare original Maximus, native Maximus, Hyper, Axum and Actix on the same host/build settings/workloads. Keep plaintext, TLS/H1, TLS/H2, echo, routing and serialized JSON results separate. Retain fixed-rate failures and validate every completed response.
6. Use longer repeated measurements, an external load generator where available, and a shared-host label where isolation is unavailable. Include CPU/allocation profiles, hardware counters, RSS, tail latency, multicore scaling, sustained load and connection churn.
7. Verify product dependency graphs independently of test/benchmark dependencies. Confirm removal of Hyper and Tokio compatibility utilities under supported feature configurations; enumerate remaining transitive dependencies honestly.

Correctness is a hard acceptance requirement. A performance claim requires reproducible improvement larger than trial variation, with no unexplained regression in the relevant latency, CPU or memory results. Architectural ownership can be delivered even if a workload remains slower, but that result cannot be called a performance win. No benchmark-specific response bypass may evade ordinary parsing, routing or resource policy.

## Execution order

First preserve/reproduce the baseline and run the small allocation, placement and memory experiments. Then decouple the body/policy/upgrade interfaces, implement and qualify native HTTP/1, implement and qualify HPACK/HTTP/2, and remove the obsolete dependencies and adapters. Finish with the complete comparative campaign and an independent review. Keep raw evidence ignored and only compact results, reproduction tools and necessary source in Git.

## References

- [HTTP semantics, RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html)
- [HTTP/1.1 framing and connection management, RFC 9112](https://www.rfc-editor.org/rfc/rfc9112.html)
- [HTTP/2, RFC 9113](https://www.rfc-editor.org/rfc/rfc9113.html)
- [HPACK, RFC 7541](https://www.rfc-editor.org/rfc/rfc7541.html)
- [h2spec scope](https://github.com/summerwind/h2spec)
- [Current qualification](../../qualification.md)
- [Original benchmark results](../../../benches/results/README.md)
