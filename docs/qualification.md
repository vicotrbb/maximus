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
cargo +nightly fuzz run http1 -- -max_total_time=60 -max_len=8192
cargo +nightly fuzz run hpack -- -max_total_time=60 -max_len=16384
cargo +nightly fuzz run http2 -- -max_total_time=60 -max_len=16384
```

The native engines also have an independent HTTP/2 conformance fixture. Build
`summerwind/h2spec` from commit `af83a65f0b6273ef38bf778d400d98892e7653d8`
outside the tracked repository, then run:

```sh
cargo build -p maximus --features tls,http2 --example conformance
python3 scripts/check-h2spec.py /path/to/h2spec
python3 scripts/check-h2spec.py /path/to/h2spec --tls
```

The wrapper requires all 147 legacy cases to execute and reports their raw
outcomes. Five specific closed-stream expectations differ from the minimal
processing explicitly permitted by [RFC 9113 §5.1](https://www.rfc-editor.org/rfc/rfc9113.html#section-5.1).
They remain visible as legacy differences. Three half-closed tests can also
race a completed response: their extra frame arrives after the stream closes.
The wrapper always repeats those exact cases against `/half-closed`, where the
fixture consumes the request and holds the response pending. All three must
pass there, with no skips; their original outcomes remain separately reported.
Any other failure, missing case, or skipped case fails the check. This is not a
claim of 147/147 h2spec compliance. The TLS fixture disables h2spec certificate
verification for its local self-signed certificate; the independent Python
suite separately verifies the local CA and hostname.

`check.sh` runs formatting, warnings-denied Clippy, workspace/all-feature tests, minimal-framework tests, each optional feature in isolation, TLS/H2 compatibility, compilation of every fuzz target, and warnings-denied documentation. Compile-fail examples validate route declarations and typed handler signatures. Linux tests require explicit io_uring support; an environment that denies it is not an io_uring pass. BPF tests need Linux BPF and network-administration capabilities and are excluded from ordinary test runs.

The independent client suite uses pinned Python packages in `scripts/interop-requirements.txt`; it tests the executable `features` example with HTTPX/h2, websockets and aioquic, verifies a locally generated certificate, and checks HTTP/1.1 plaintext/TLS, ALPN HTTP/2, QUIC HTTP/3, JSON, malformed JSON, multipart, SSE, compression, CORS, HEAD, typed routes, binary/text WebSockets, and sixteen concurrent 128 KiB uploads. `tests/wire.py` uses only the Python standard library and sends 2,062 deterministic wire mutations and early disconnects, then verifies the workers remain healthy. The Rust integration tests cover TLS resumption, strict TLS1.3 configuration, pipelining, deadlines, body/header policy, shutdown, worker failure, repeated io_uring acceptance, H2 flow control and H3 requests, malformed message lengths, response producer failures, withheld stream credit, trailer handling, and cancellation. The independent suite also sends malformed H3 pseudo/connection/trailer fields and validates split Cookie normalization for H2/H3. The vendored decoder retains its upstream tests plus focused regressions; its provenance and downstream Cargo override are documented separately.

Fuzzing exercises percent decoding, path validation, route matching, typed captures and query extraction, plus the native HTTP/1 and HTTP/2 wire engines and stateful HPACK codec under libFuzzer/AddressSanitizer. Generated corpus growth and build outputs are ignored; only deliberate `seed*` inputs belong in Git. The default 60-second run is a repeatable smoke campaign, not exhaustive protocol fuzzing.

## Requirement map

| Requirement | Implementation / executable evidence |
| --- | --- |
| Local workers, io_uring/epoll, affinity, NUMA, shutdown | `runtime`, `io`; backend/runtime tests and Linux suite |
| TCP/UDP reuseport and optional eBPF | `net`; socket tests; capability-enabled live BPF attachment/distribution test |
| Bounded slabs/buffers and admission | `memory`, `Limits`, framework server; stale-key/pool tests, budget tests and 20,000-connection workload |
| Incremental H1, pipeline, keepalive, streaming | Owned HTTP/1 engine; raw-wire server tests and mutation client |
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

See [native HTTP qualification](native-http-qualification.md) for the version 0.2 execution record, exact source identity, compatibility limits and test counts. The [benchmark comparison](../benches/results/native-http-comparison.md) records throughput, latency, allocations, memory and failed load targets. Historical version 0.1 reports remain in Git history.

Recorded execution used macOS ARM64 and shared ARM64/x86-64 Linux hosts. Results do not establish production acceptance, arbitrary-platform support, multi-NUMA scaling or performance beyond the measured workloads. Raw logs and generated corpora remain outside Git; compact results and hashes identify the retained local evidence.
