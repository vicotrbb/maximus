# Maximus design

The supplied specification in `docs/original-spec.md` is the product contract. The product and all packages are named Maximus. This is a Linux production server library, not a web application.

Use Compio's local executor and completion I/O as the mechanism beneath a specialized worker runtime. Each OS thread owns its runtime, listener, application instance, buffers and connection registry. Use io_uring by default with explicit epoll selection and automatic fallback. Socket reuseport, affinity and NUMA topology are configured at startup. No request task migrates across threads.

Reuse Hyper for HTTP/1 and HTTP/2, Rustls for TLS 1.3, and Compio QUIC plus h3 for HTTP/3. Protocol crates enforce limits and adapt the same request/response API. Keep HTTP/3's UDP transport independent. These dependencies implement security-sensitive wire protocols; Maximus owns integration, policies and lifecycle.

Static routing expands into Rust dispatch at compile time. Handler and middleware futures use static dispatch. Dynamic routing is opt-in. Requests expose typed paths, headers, queries and bounded bodies, and applications provide explicit typed state. Streaming is pull-based with bounded buffers. JSON, multipart, WebSockets, SSE, compression, CORS, telemetry, TLS and advanced HTTP protocols are feature-gated.

The requested twelve crates separate runtime, I/O, sockets, memory, TLS, protocols, core types, routing, macros and public integration. Avoid secondary internal abstractions unless required by two real callers.

Each requirement must have an implementation pointer and executable test or measured qualification evidence in the requirements ledger. Conditional optimizations such as kernel zero-copy, registered buffers, multishot and arenas require a benefit measurement; do not label available kernel capabilities as enabled optimizations. Best-in-class performance and production readiness require comparative evidence and cannot be assumed from passing tests.

Validation runs on native development host where supported and Linux Docker for all Linux behavior. Include protocol interop, malformed wire traffic, resource exhaustion, cancellation, graceful draining, feature combinations, compile-fail macros, fuzzing, concurrency, sustained load, latency distributions and competitor baselines. Retain commands and results. No finite suite guarantees absence of bugs.
