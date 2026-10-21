# API and operating guide

## Requests and routes

`Request` dereferences to `http::Request<Body>`. `path::<T>(name)`, `query::<T>(name)` and `header::<T>(name)` use `FromStr`. Duplicate query values and singleton extractor headers are rejected. Percent encoding is checked, UTF-8 must be valid, and decoded path separators, control characters and dot segments are rejected before dispatch. Extractors return an HTTP error suitable for `?` in a handler returning `Result<T, Error>`.

`routes! { GET "/users/{id:u64}" => user }` calls `user(request, id)` directly. Untyped captures remain available through `request.path`. Literal segments take precedence from left to right; the most specific matching path owns method resolution. Explicit HEAD wins over GET fallback. Unmatched paths return 404; a known path with an unsupported method returns 405 with Allow. Static handlers and middleware are monomorphized. Dynamic routing boxes only dynamic handler futures and enforces its configured registration count.

## State and middleware

`state(value, |request, state: State<T>| async move { ... })` holds typed state in a worker-local `Rc`. Create mutable local state with `Cell` or `RefCell`; never hold a `RefCell` borrow over an await. Use `Arc` explicitly if cross-worker sharing is necessary. `middleware(handler, |request, next| async move { next.handle(request).await })` wraps a statically typed handler. `guard(handler, predicate)` returns 403 when the predicate fails. Ordering follows nesting; put CORS outside compression, for example. There is no global service container or hidden thread migration.

## Bodies

`Body::from(Bytes)` reuses the payload. Static strings and empty bodies are unboxed. `Body::stream` accepts a local stream of `Result<Bytes, Error>`; `Body::frames` also supports trailers. Producers are polled on demand. `http_body_util::BodyExt` supplies collection and frame helpers. Network requests receive a byte-limited body before application code. Never collect an intentionally unlimited application-created stream.

JSON uses Serde and the configured network body limit. Multipart has additional whole-upload and per-field limits. SSE encodes multiline data and rejects metadata injection; send heartbeat events more frequently than the configured stream timeout. WebSocket callbacks remain part of connection admission and shutdown, with configurable frame/message bounds. Use an origin policy for browser WebSocket endpoints; CORS is available as an outer wrapper.

Compression applies gzip only when negotiated, smaller than the original, inside its configured size range, and not prohibited by range/cache/encoding headers. It weakens strong ETags and removes stale content length/digest metadata. Streaming compression is deliberately absent: buffered compression cannot preserve pull-based streaming or trailers. Add a streaming encoder explicitly at the application boundary if required.

## Limits and backpressure

`Limits` applies per worker. TCP and QUIC split `memory_bytes` equally when both are enabled. Admission reserves I/O buffers, headers, maximum request bodies, metadata, multiplexed stream budgets, flow-control windows and TLS record space. `protocol_capacity` exposes the generic connection estimate. TCP TLS additionally reserves two `io_buffer_bytes` units for ciphertext adapter growth (32 KiB per connection with the default); QUIC uses the generic estimate. A configuration unable to admit one connection is rejected.

This is a conservative admission estimate, **not a hard RSS allocator limit**. Application state, response payloads, allocator fragmentation, runtime overhead and kernel socket buffers require their own budget. Use container/process memory limits for a final OS boundary. Maximum body and stream settings can substantially reduce admitted TLS/H2/H3 connections: tune them together rather than raising only `connections`.

At capacity, TCP acceptance pauses and the kernel backlog is bounded; QUIC refuses new connections. There is no unbounded application accept queue. Protocol flow control and bounded I/O buffers limit queued transport data. Compio reuses per-connection I/O buffers and provides worker-local managed buffer pools; the kernel benchmark exercises managed/multishot reads separately. Aggressive kernel paths are not enabled for HTTP without an HTTP workload improvement.

`header_timeout` bounds HTTP/1 headers and TLS/QUIC setup. `request_timeout` bounds handler execution and each streamed response frame; a stalled frame terminates that stream. `idle_timeout` bounds connections without wire progress, including upgraded sockets. H2 keepalive frames count as wire activity; the stream count, windows and request budgets remain bounded. HTTP/2 DATA bytes repay bounded control-frame credit so long transfers can replenish flow-control windows; empty DATA, padding and control-only floods earn no credit. `drain_timeout` is the maximum shutdown wait. Continuous application streams need heartbeat data and cooperative futures. Blocking or infinite CPU work inside a handler blocks its worker; move it to an explicitly managed bounded blocking executor.

HTTP/1 rejects duplicate or malformed Content-Length fields and every request combining Transfer-Encoding with Content-Length before application dispatch. Rejection closes the connection, preventing following bytes from becoming another request. Chunked framing, extensions and trailers are validated incrementally. An unread request body closes HTTP/1 instead of reusing its bytes. HTTP/1.1 requires exactly one valid Host; duplicate Hosts are rejected for every version. URI authorities also receive authority validation. Split HTTP/2 and HTTP/3 Cookie fields are combined before application dispatch, after applying the raw header limits. HTTP/3 rejects invalid header ordering, pseudo-field multiplicity/context, forbidden connection fields and malformed trailers through a documented patch to the existing decoder. Both incoming and outgoing H3 Content-Length are checked against DATA bytes; failed or cancelled responses reset the stream. HEAD and bodyless status semantics are applied across engines; HTTP/3 omits optional HEAD Content-Length for client interoperability.

HTTP/1 also closes the connection after rejecting CONNECT, including an unregistered
tunnel converted to 501. Optimistic tunnel bytes cannot become subsequent HTTP
requests. This follows [RFC 9931 §8](https://www.rfc-editor.org/rfc/rfc9931.html#section-8).
Successful registered tunnels retain their upgrade and read-ahead behavior.

## TLS, placement and observability

`TlsConfig::from_pem` accepts a certificate chain and private key, enables only TLS 1.3 and disables early data. Pass `true` for H2 ALPN only with the `http2` feature. Ring uses supported hardware cryptography on the target CPU. Session storage is bounded to 256 entries. HTTP/3 advertises h3, disables 0-RTT and active migration, and retries unvalidated incoming addresses. Migration is disabled because reuseport routing uses the connection tuple. Certificate rotation currently requires starting a new server configuration; it is not an implicit background task.

`WorkerConfig.cpus` validates Linux affinity and pins before runtime/state allocation, enabling NUMA first-touch placement. `numa_node(cpu)` reports sysfs topology. Applications choose a CPU per worker; this does not override administrator cgroup/NUMA policy. eBPF attachment takes an owned program descriptor and relies on the kernel to check program type. The caller owns loading and policy, while Maximus owns the attached socket lifetime.

`telemetry(handler, tracer)` uses an explicitly supplied tracer and the globally configured propagator/meter. Configure the OpenTelemetry SDK/exporter in the application. Spans end with response-body completion or cancellation. Method/status attributes exclude raw paths, queries and sensitive headers. Keep SDK/exporter backlogs bounded. Middleware order affects whether compression sees a buffered body: place compression inside telemetry when both are used.
