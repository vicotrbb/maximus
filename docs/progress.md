# Maximus implementation and qualification

The supplied specification was read in full and retained as `docs/original-spec.md`. The project is named Maximus as requested. The implementation plan is `docs/superpowers/plans/2026-10-15-maximus.md`.

## Implementation

All twelve requested crates are implemented. Maximus owns local worker lifecycle, resource policy, typed routing and framework integration. Maintained Compio, Hyper, Rustls and h3/Quinn engines provide I/O, execution, wire protocols and cryptography. Optional features remain gated; comparison servers and fuzz tooling are separate Cargo workspaces. Maximus-owned Rust source is 3,343 lines, excluding tests, examples and retained upstream source.

The qualified product revision is `2f2f79818df2845df624016939ac63444c3c6f7a`. Later changes package benchmark tooling and evidence without changing the product or comparison-server source. The protocol and integration suites passed on macOS, ARM64 Linux and x86-64 Linux. Exact commands, counts and boundaries are in [qualification](qualification.md); compact log hashes are in `benches/results/evidence.json`.

## Regressions found and fixed

- Cancelled io_uring accept futures could discard accepted sockets when another connection completed. The worker retains a submitted accept until completion; repeated connection churn covers it.
- Worker startup/runtime failure could leave siblings running. Cancellation now propagates and listeners are cleaned up before startup errors return.
- Streaming responses needed bounded producer and write deadlines. H1/H2/H3 stream, stalled-peer and shutdown tests exercise cancellation and draining.
- Routing needed literal-path method fallthrough and explicit HEAD precedence. Compile-time declarations, typed captures and dynamic routes have focused tests.
- H2 HEAD could emit DATA, and invalid or empty Host values needed shared policy. Body suppression and authority validation apply across the protocol integrations.
- HTTP/3's released decoder discarded malformed-field evidence before application validation. A narrow two-source-file patch rejects duplicate/out-of-order pseudo-fields, incorrect contexts, empty values, connection-specific fields, invalid TE and malformed trailers. Upstream provenance/licenses and the required external Cargo override are documented in `vendor/h3/PATCHES.md`. The retained upstream suite passed 229 unit/integration tests and 8 doc tests.
- H2/H3 split Cookie fields now normalize before generic handlers. HTTP/3 validates response Content-Length syntax and emitted length, resetting malformed, failed or truncated streams; bodyless 304 has a dedicated regression.
- A cancelled handler could leave an OpenTelemetry span unset. A minimal lifetime guard distinguishes produced responses from cancelled handlers; response-stream cancellation and metric balance are tested too.
- WebSocket tests cover negotiated subprotocols, frame/message bounds and tracked shutdown lifetime.

Hyper's RFC 9112 TE+CL handling gives Transfer-Encoding precedence, removes Content-Length and closes the connection. The raw pipeline regression verifies that a following request is not dispatched. Memory admission accounts conservatively for multiplexed bodies/windows/TLS and splits TCP/QUIC budgets; it is an estimate, not a hard RSS limit.

## Performance work

Reproducible k6 comparisons cover Maximus io_uring/epoll, Hyper, Axum, Actix and a native Nginx static-response reference. Verified TLS1.3 comparisons cover Maximus/Hyper/Axum over H1 and H2. A second Linux host adds response-checked wrk, one/two/four-worker comparisons, hardware PMU events and sustained saturation. Fixed-rate failures remain in the report alongside adequately provisioned reruns.

Allocation instrumentation distinguishes zero allocations for isolated static route/body dispatch, two for static text response conversion, and approximately twenty for the complete HTTP/1 keepalive path. A proposed first-poll timeout optimization failed to establish an allocation improvement and was discarded. Kernel socket tests compare verified copy, managed-buffer, multishot and zero-copy paths; the HTTP adapter keeps the maintained bounded path because the experiments did not justify promotion.

Perf's task-clock/cgroup discrepancy was investigated with scheduler runtime and per-core IRQ counters. The measured difference is consistent with interrupt accounting, so PMU task-context counters and cgroup CPU are reported as separate scopes. A deliberately invalid wrk workload verified that response failures cause a failed run and complete process cleanup.

The [performance report](../benches/results/README.md) contains actual trial counts, medians, tails, resource samples, connection churn, sustained results and limitations. The fresh independent product and benchmark reviews found no remaining required-capability gap. Passing finite tests and shared-host benchmarks does not establish zero bugs, universal performance leadership, multi-NUMA behavior or production acceptance.
