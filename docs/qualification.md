# Qualification

Run these checks from the repository root. Results apply to the checked revision and environment; neither a test suite nor a benchmark can guarantee the absence of bugs.

## Repeatable checks

```sh
scripts/check.sh
scripts/test-linux.sh
scripts/test-linux-ebpf.sh
scripts/test-interop-linux.sh
scripts/check-downstream.sh
cargo test --manifest-path vendor/h3/Cargo.toml --all-features --locked
cargo audit
cargo llvm-cov --workspace --all-features --summary-only
cargo +nightly fuzz run request -- -max_total_time=60 -max_len=4096
```

`check.sh` runs formatting, warnings-denied Clippy, workspace/all-feature tests, minimal-framework tests, each optional feature in isolation, TLS/H2 compatibility, and warnings-denied documentation. Compile-fail examples validate route declarations and typed handler signatures. Linux tests require explicit io_uring support; an environment that denies it is not an io_uring pass. BPF tests need Linux BPF and network-administration capabilities and are excluded from ordinary test runs.

The independent client suite uses pinned Python packages in `scripts/interop-requirements.txt`; it tests the executable `features` example with HTTPX/h2, websockets and aioquic, verifies a locally generated certificate, and checks HTTP/1.1 plaintext/TLS, ALPN HTTP/2, QUIC HTTP/3, JSON, malformed JSON, multipart, SSE, compression, CORS, HEAD, typed routes, binary/text WebSockets, and sixteen concurrent 128 KiB uploads. `tests/wire.py` uses only the Python standard library and sends 2,062 deterministic wire mutations and early disconnects, then verifies the workers remain healthy. The Rust integration tests cover TLS resumption, strict TLS1.3 configuration, pipelining, deadlines, body/header policy, shutdown, worker failure, repeated io_uring acceptance, H2 flow control and H3 requests, malformed message lengths, response producer failures, withheld stream credit, trailer handling, and cancellation. The independent suite also sends malformed H3 pseudo/connection/trailer fields and validates split Cookie normalization for H2/H3. The vendored decoder retains its upstream tests plus focused regressions; its provenance and downstream Cargo override are documented separately.

Fuzzing exercises percent decoding, path validation, route matching, typed captures and query extraction under libFuzzer/AddressSanitizer. Generated corpus growth and build outputs are ignored; only seven deliberate seeds belong in Git. The default 60-second run is a repeatable smoke campaign, not exhaustive protocol fuzzing.

## Requirement map

| Requirement | Implementation / executable evidence |
| --- | --- |
| Local workers, io_uring/epoll, affinity, NUMA, shutdown | `runtime`, `io`; backend/runtime tests and Linux suite |
| TCP/UDP reuseport and optional eBPF | `net`; socket tests; capability-enabled live BPF attachment/distribution test |
| Bounded slabs/buffers and admission | `memory`, `Limits`, framework server; stale-key/pool tests, budget tests and 20,000-connection workload |
| Incremental H1, pipeline, keepalive, streaming | Hyper integration; raw-wire server tests and mutation client |
| TLS1.3, ALPN, resumption | `tls`; Rust resumption test and independently verified TLS client |
| H2 multiplexing, HPACK, flow control | `http2`; concurrent large response test and HTTPX concurrent uploads |
| H3 QUIC/QPACK and independent UDP | `http3`; Rust client and independent aioquic with multiplexed large uploads |
| Static/typed routing, dynamic registration | `macros`, `router`; precedence/extraction tests and compile-fail docs |
| State, middleware, guards, errors | `core/pipeline`; pipeline integration tests |
| JSON/multipart/SSE/WebSocket/CORS/gzip/OTel | Feature tests, SDK in-memory trace test, independent executable checks |
| Timeouts, cancellation and bounded drain | Handler/stream/idle deadlines; worker/startup failure tests; WebSocket drain tests |
| Pools, multishot, zero-copy and batching | Maintained Compio primitives; verified kernel socket benchmark. HTTP promotion requires measured HTTP benefit |
| Allocation/CPU/memory/latency/load comparisons | Isolated allocation binary, k6 scenarios, process/cgroup samples, perf and kernel benchmarks |
| Unsafe isolation | Only optional product eBPF ABI adapter; Linux live/rejection tests. Allocator instrumentation and BPF loading are test tools |

## Recorded execution

The qualified product revision is `2f2f79818df2845df624016939ac63444c3c6f7a`; the later evidence commit changes benchmark tooling and documentation only.

| Check | Result |
| --- | --- |
| Host formatting, Clippy, feature matrix and docs | Passed; product warnings denied |
| Host workspace/minimal/doc tests | 57 test executions passed |
| Linux ARM64 workspace tests | 51 passed; one separately gated BPF test ignored |
| Linux x86-64 workspace tests | 51 passed; one separately gated BPF test ignored |
| Capability-enabled Linux BPF checks | 2 passed |
| Patched upstream h3 suite | 229 unit/integration tests and 8 doc tests passed |
| External application/dependency override | Compiled; exact patched h3 resolution verified |
| Independent network clients | H1/TLS/H2/H3, streaming, framework integrations and malformed H3 cases passed |
| Adversarial wire exercise | 2,062 mutations/disconnects with 16 concurrent clients; subsequent health check passed |
| Fuzz/AddressSanitizer smoke | 4,696,284 parser/extractor executions in 61 seconds; no crash |
| Host Rust coverage | 87.36% lines, 85.05% regions; excludes independent clients and Linux-only execution |
| Dependency advisory audit | No advisories among 236 locked dependencies at the recorded audit time |

Test counts include separate configurations and doc tests, not distinct product features. The capability-gated BPF tests are deliberate privileged checks, not missing ordinary-suite coverage. Regressions found and fixed during qualification are recorded in [progress](progress.md). Hardware and VM load results, raw trial summaries, resource measurements and measurement caveats are in [the benchmark report](../benches/results/README.md). Raw local logs are ignored; compact hashes and results preserve the evidence identity.

## Evidence boundary

Linux qualification used an ARM64 OrbStack VM with three vCPUs and approximately 4 GiB RAM, plus an x86-64 Linux 6.6 homelab node with an eight-core Ryzen 7 5825U and 32 GiB RAM. The VM exposes software CPU-clock profiling but no usable hardware PMU; the homelab exposes measured cycles, instructions and cache misses. Both hosts were shared. Server/client affinity masks were disjoint; existing workloads could still share those cores and caches.

The benchmark report distinguishes k6 client limits, fixed-rate dropped arrivals, wrk's corrected latency histogram, task-attached PMU events and cgroup CPU. Five-minute sustained runs and repeated 20,000-connection churn are finite stress exercises. They do not establish production acceptance, absence of all bugs, universal performance leadership, multi-NUMA-node behavior or scaling beyond the measured cores. The initial API is version 0.1; no published-release compatibility promise is inferred from this qualification.
