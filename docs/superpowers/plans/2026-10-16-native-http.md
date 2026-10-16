# Native HTTP implementation and experiments

Spec: `docs/superpowers/specs/2026-10-16-native-http-design.md`.
Baseline: `9cb096a`; implementation stays on the existing isolated `implement` branch. Use subagent-driven development for disjoint codec, engine and experimental tasks, with integrated review before completion.

## Global constraints

- Own HTTP/1.1 and HTTP/2; retain Rustls and h3/Quinn.
- Remove Hyper and Tokio compatibility utilities from product dependencies, while retaining independent client/baseline dependencies.
- Preserve bounded streaming, malformed-message rejection, TLS, upgrades, cancellation, deadlines and all framework features.
- Keep the repository lean; raw experimental copies/caches stay ignored.
- Require measured gains and regression evidence for performance claims; retain negative results.
- Write failing behavioral tests before implementation. Do not weaken tests to accommodate broken protocol behavior.

## Review focus

- Arbitrary network fragmentation/coalescing must not confuse message boundaries or lose upgrade bytes.
- Slow consumers, dropped request bodies and stalled stream credit must not deadlock other streams or grow memory without bounds.
- HPACK dynamic-table and Huffman errors must fail safely without desynchronizing the connection.
- HTTP/2 settings/window changes, reset/closed-stream races and GOAWAY must preserve connection/stream error scope.
- Cancellation must preserve submitted I/O buffer lifetimes and terminate every owned connection/task by the drain deadline.

### Task 1: Controlled opportunity experiments

Own ignored `artifacts/native-opportunities/` and a final compact `benches/results/native-opportunities.md`; do not edit the active product or the previous results.

- [x] Create an isolated archived baseline from `9cb096a`, retaining original version identity.
- [x] Run real baseline versus allocation-free Compio WouldBlock construction, with the actual HTTP path and allocation/CPU/response checks.
- [x] Compare process affinity and explicit per-worker pinning, with repeated one/two/four-worker trials and migration/cache/load observations.
- [x] Instrument live allocation bytes versus RSS across connection churn; identify whether retention is live data or freed allocator pages before proposing a fix.
- [x] Measure policy/deadline costs using the archived Hyper-backed Maximus control and targeted profiles; preserve enforcement. Record the earlier rejected timeout optimization. The profiles isolate dispatch/timeout costs but do not establish full policy parity with the separate Tokio/Hyper server.
- [x] Report commands, source diffs/hashes, outputs and uncertainty. Coordinate hardware access to avoid overlapping timed campaigns.

Execution adjustment: reuse the archived Hyper-backed Maximus implementation as the control carrying framework policy, and use targeted profiles for policy attribution. A second implementation of all policies in the Tokio server would add another unqualified policy implementation. Bare Hyper remains an unequal-policy reference; its entire performance gap cannot be assigned to runtime or policy costs. See [the opportunity report](../../../benches/results/native-opportunities.md).

### Task 2: Owned HPACK codec

Own `crates/http2/src/hpack.rs` and its focused tests. No production dependency on another HPACK implementation.

Interfaces: `Decoder::new(max_table_size: usize, max_header_list_size: usize, max_headers: usize)`, `Decoder::decode(&mut self, block: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, Error>`, and `encode(headers: &[(&[u8], &[u8])], output: &mut Vec<u8>)`. Decoder errors represent compression errors; HTTP field semantics belong to the connection engine. Encoder may use static indices and never-indexed literals; full peer interoperability is required, not a particular compression ratio.

- [x] Add failing RFC 7541 request/response vectors, Huffman padding/EOS/truncation and integer overflow cases.
- [x] Implement bounded static/dynamic-table decoding, table size updates, literal/indexed representations and Huffman decoding.
- [x] Implement a deterministic conforming encoder and roundtrip/dynamic-state tests, with precise decoded size/count bounds.
- [x] Run focused tests and adversarial/differential vectors. Preserve provenance for standardized tables. Self-review and report exact test results.

### Task 3: Native HTTP/1 and protocol-neutral integration

Own `crates/core`, `crates/http1`, `crates/framework/src/{transport,websocket}.rs` and their tests. Coordinate manifest changes with the controller; do not edit HTTP/2 or the benchmark workspace.

Interfaces: retain `Request`, `Response`, fixed/streaming `Body` and handler APIs; replace Hyper-specific incoming bodies with a protocol-neutral bounded stream. Export shared `dispatch_request`, `content_length`, and response body semantics from core; temporarily re-export from HTTP/1 where needed by unchanged H3. Provide an owned-buffer Compio transport entry point usable by TCP and TLS, and document its exact signature for Task 4. Replace the WebSocket Hyper upgrade handoff without losing read-ahead bytes.

- [x] Add failing real-wire tests for framing, fragmentation, pipelining, Expect, trailers, malformed messages and complete/failed streams.
- [x] Implement native incremental HTTP/1 parsing/writing over owned buffers, without Hyper or a new HTTP parser dependency.
- [x] Preserve deadlines, idle handling, limits and drain; reject ambiguous framing and invalid lengths without desynchronizing requests.
- [x] Integrate bounded body streaming, TLS, all bodyless response rules and WebSocket handoff/lifetime.
- [x] Run focused HTTP/1/core/WebSocket tests, independent wire clients and feature regressions. Record public API changes and unresolved integration dependencies.

### Task 4: Native HTTP/2 engine

Own `crates/http2/src/{lib,frame}.rs`, HTTP/2 manifest and tests. Consume Task 2's codec and Task 3's protocol-neutral policy/body and transport interfaces.

- [x] Add failing tests for preface, frame bounds, SETTINGS/ACK, HPACK integration and basic concurrent streams.
- [x] Implement frame/stream state, bounded headers and pseudo-field validation, DATA and trailer handling, independent stream/connection windows and SETTINGS adjustments.
- [x] Implement fair bounded response scheduling, consumed-body receive credit, RST_STREAM, PING, GOAWAY, deadlines and shutdown; test cancellation and window starvation.
- [x] Run independent Hyper/h2 clients, h2spec plus RFC 9113 regressions, fragmented malformed sequences and concurrency tests.

### Task 5: Dependency removal and complete regression qualification

- [x] Remove obsolete Hyper/adaptation code and product manifests; retain benchmark/test clients in development-only graphs.
- [x] Document versioned public API changes and remaining Tokio transitives from h3.
- [x] Run fmt, Clippy, feature matrix, docs, host/Linux tests, independent H1/TLS/H2/H3 and WebSocket clients, patched h3 tests, BPF checks, downstream builds and audit.
- [x] Add native parser/HPACK fuzz targets and exercise malformed streams, early disconnects, cancellation and churn.
- [x] Review the complete change independently and fix concrete findings with regressions.

### Task 6: Comparative qualification and completion

- [x] Compare unchanged baseline, optimized baseline and native product with Hyper/Axum/Actix under matching workload/policy disclosures.
- [x] Test buffer/write strategies, plaintext and TLS separately, fixed arrivals, one/two/four workers, sustained saturation and repeated 20,000-connection churn.
- [x] Record raw source/tool identities, allocations, CPU/counters, p50/p99/p99.9, RSS, errors and drops. Preserve failed experiments.
- [x] Publish compact evidence, clean up only owned temporary infrastructure, commit final work and report measured gains or remaining gaps honestly.
