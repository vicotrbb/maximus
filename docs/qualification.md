# Qualification

Run these checks from the repository root. Results apply to the checked revision and environment; neither a test suite nor a benchmark can guarantee the absence of bugs.

## Repeatable checks

```sh
scripts/check.sh
scripts/test-linux.sh
scripts/test-linux-ebpf.sh
scripts/test-interop-linux.sh
cargo audit
cargo llvm-cov --workspace --all-features --summary-only
cargo +nightly fuzz run request -- -max_total_time=60 -max_len=4096
```

`check.sh` runs formatting, warnings-denied Clippy, workspace/all-feature tests, minimal-framework tests, each optional feature in isolation, TLS/H2 compatibility, and warnings-denied documentation. Compile-fail examples validate route declarations and typed handler signatures. Linux tests require explicit io_uring support; an environment that denies it is not an io_uring pass. BPF tests need Linux BPF and network-administration capabilities and are excluded from ordinary test runs.

The independent client suite uses pinned Python packages in `scripts/interop-requirements.txt`; it tests the executable `features` example with HTTPX/h2, websockets and aioquic, verifies a locally generated certificate, and checks HTTP/1.1 plaintext/TLS, ALPN HTTP/2, QUIC HTTP/3, JSON, malformed JSON, multipart, SSE, compression, CORS, HEAD, typed routes, binary/text WebSockets, and sixteen concurrent 128 KiB uploads. `tests/wire.py` uses only the Python standard library and sends 2,062 deterministic wire mutations and early disconnects, then verifies the workers remain healthy. The Rust integration tests cover TLS resumption, strict TLS1.3 configuration, pipelining, deadlines, body/header policy, shutdown, worker failure, repeated io_uring acceptance, H2 flow control and H3 requests.

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

## Evidence boundary

Recorded results are in `docs/progress.md` and `benches/results/`; large raw files remain in the ignored local `artifacts/` directory. Linux qualification uses an aarch64 OrbStack VM with 3 vCPUs and approximately 4 GiB RAM. The VM does not expose hardware cycles/instructions/cache-miss counters; software CPU-clock profiling is available. These measurements do not establish production acceptance, bare-metal performance leadership, multi-NUMA-node behavior or near-linear scaling beyond the tested cores. The limits are documented, not represented as passed tests.
