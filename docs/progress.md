# Maximus progress

Plan: docs/superpowers/plans/2026-10-15-maximus.md

- Original specification read in full; copied without changes.
- Host Rust 1.96.0. Docker Linux aarch64 available with 3 CPUs.
- Goal active. No product capability or performance claim verified yet.
- Architecture decision: use maintained Rust protocol engines and Compio beneath Maximus-owned worker orchestration, limits and API. This reduces custom unsafe and protocol code; dependency behavior remains subject to integration tests.
- User supplied implementation authorization and full specification; execute continuously in the new isolated maximus directory.

## Foundation checkpoint

- Added memory, io, net and runtime crates with no owned unsafe code.
- Host suite: 12 tests passed. Host clippy with warnings denied passed.
- Linux Docker suite: 13 tests passed including explicit io_uring, epoll, CPU pinning, local non-Send tasks and shutdown notification. Output: artifacts/foundation-linux.log.
- Fixed-capacity slab rejects stale keys; buffer cache discards oversized returned buffers. The buffer cache does not itself bound caller-owned growth; server body and connection budgets must do so.
- NUMA topology discovery and first-touch allocation follow worker pinning. eBPF attachment and kernel optimization measurements remain open.
- Core API tests observed missing API failures before implementation. Core integration now in progress.

## Protocol and framework checkpoint

- Implemented all twelve crates and optional integrations. Static routing uses direct typed async calls; dynamic registration is separately gated.
- Host and Linux all-feature suites pass; isolated feature checks, minimal build, Clippy and docs pass. Exact final commands are rerun after review fixes.
- Independent HTTPX/h2, websockets and aioquic checks pass on macOS and Linux, including verified TLS, 16 concurrent uploads and H3 flow control.
- Real Linux BPF program attachment/distribution passed with explicit container capabilities; ordinary builds do not require them.
- Fixed regressions found during testing: cancelled io_uring accepts discarding accepted sockets; worker failure leaving siblings alive; streaming responses without deadlines; literal-path method fallthrough; explicit HEAD precedence; H2 HEAD carrying DATA; invalid/empty Host acceptance.
- RFC 9112 TE+CL handling is delegated to Hyper: Transfer-Encoding wins, Content-Length is removed, and the connection closes. The pipeline regression verifies the next request is not dispatched.
- Memory admission now accounts for multiplexed bodies/windows/TLS and splits the TCP/QUIC worker budget. The guide distinguishes admission estimates from total RSS.
- Independent wire stress: 2,062 deterministic malformed/truncated requests, 16 clients; post-stress server remained healthy.
- Fuzz campaign: 4,696,284 executions in 61 seconds, no crash (artifacts/fuzz.log). Parser/extractor target; not an exhaustive protocol audit.
- Host Rust coverage snapshot: 85.48% lines, 83.51% regions. Independent client runs and Linux-only paths are additional evidence, not included in that host coverage number.
- 20,000 Linux connections connected in 3.02 seconds, every response checked, idle then all revalidated, closed in a total 13.95 seconds. Memory samples saved separately.
- Allocation snapshot: static route/body 0 allocations; text response conversion 2 allocations; complete HTTP/1 keepalive ~20.12 allocations/request. No claim of zero allocations on the wire.
- Kernel primitive comparisons and initial k6 comparisons executed. Final comparative results, performance interpretation, profiling, sustained stress and fresh review remain open.
