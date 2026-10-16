# Native HTTP qualification

The [subsequent HTTP/2 optimization report](../benches/results/native-hot-paths.md) records the current hot-path changes and their fresh validation. The evidence below describes the original native-engine replacement at the named revisions.

The native product revision is `8a8fa9b23c652ae4b6e40fec6322cb9e094e22c9`, version 0.2.0. Benchmark build revision `f85abcc841ee47c4d7739e21d669fd062112414b` changes only the comparison lockfile package versions; product source is identical. It replaces the original Hyper-backed HTTP/1 and HTTP/2 engines. The original qualification at `2f2f798` remains historical; it is not evidence for the replacement.

## Executed checks

| Check | Result and scope |
| --- | --- |
| Host `scripts/check.sh` | Formatting, warnings-denied Clippy, all-feature/minimal tests, isolated feature combinations, product dependency exclusion and warnings-denied docs passed; 139 test executions |
| Linux ARM64 workspace | 118 test executions passed; one separately gated BPF test ignored |
| Linux x86-64 workspace | Locked/offline build and 118 test executions passed; one separately gated BPF test ignored |
| Independent executable clients | HTTP/1 plaintext/TLS, HTTP/2 ALPN, HTTP/3 QUIC, JSON/errors, multipart, SSE, gzip, CORS, HEAD, typed routing, text/binary WebSockets, sixteen concurrent 128 KiB uploads and malformed H3 cases passed |
| Raw network stress | 2,062 mutations/disconnects with sixteen concurrent clients; post-stress health passed |
| External application | Compiled against the public API; exact documented patched h3 resolution verified |
| Optional Linux BPF | Two capability-enabled tests passed; BPF code unchanged by the final H1/H2 fixes |
| Patched upstream h3 | 229 unit/integration tests and eight doc tests passed; vendored source unchanged |
| Dependency audit | No advisories among 236 locked dependencies at the recorded audit time |
| Host coverage | 84.62% lines, 83.18% regions; excludes independent clients, fuzz runs and Linux-only execution |
| Fresh source review | No blocking findings after corrections; independently exercised 17 H2 unit tests, 30 H2 wire tests, eight review probes, fifteen native H1 wire tests, Date behavior and one-byte I/O |

Counts include separate configurations and doc tests; they are not counts of distinct features. Qualification uses Rust 1.96 on macOS ARM64, Linux ARM64 and Linux x86-64. The benchmark archive verifies all 220 tracked source files before and after its locked build. The native protocol modules use safe Rust. The repository still contains the existing optional, documented eBPF ABI boundary.

## Fuzz evidence

Each target ran under libFuzzer/AddressSanitizer for 61 seconds with no crash. The request, H1 and H2 runs used the final corresponding code; HPACK was unchanged after its recorded run. This totals 13,937,029 executions, not distinct test cases or exhaustive coverage.

| Target | Executions | Maximum input bytes |
| --- | ---: | ---: |
| Request/extractors | 8,012,241 | 4,096 |
| HTTP/1 wire engine | 1,283,528 | 16,384 |
| Stateful HPACK | 2,201,724 | 16,384 |
| HTTP/2 wire engine | 2,439,536 | 16,384 |

The wire targets use fragmented in-memory I/O, bounded output, request/body limits and short deadlines. Independent socket/TLS tests exercise real transports separately. Generated corpus files remain ignored; deliberate starting seeds are tracked.

## Protocol evidence

The native HTTP/1 tests exercise fragmentation, pipelining, fixed/chunked body framing, trailers, Expect, HTTP/1.0 closure, ambiguous framing rejection, streaming and bodyless responses, producer failure, TLS closure, upgrades and preserved read-ahead bytes, CONNECT handoff, deadlines and bounded drain. A separate one-byte reader/writer checks partial I/O behavior.

HTTP/2 wire tests cover independent flow-control windows, SETTINGS changes, bounded data/header handling, consumed-body credit, HPACK synchronization, resets, trailers, response validation, stream fairness, deadlines and shutdown. Review found and corrected late-header HPACK desynchronization, the first HEADERS payload deadline, the early-response reset code, and a pending-trailer CONTINUATION race that could redispatch a closed stream. Each has executable regression coverage.

Both plaintext and TLS runs execute all **147 h2spec cases**: **142 pass and five legacy closed-stream expectations differ**. Maximus uses the minimal processing of closed streams explicitly permitted by [RFC 9113 §5.1](https://www.rfc-editor.org/rfc/rfc9113.html#section-5.1), retaining HPACK and connection flow-control state without reopening streams. The wrapper lists the exact differences and fails on any unknown failure, missing case or skip. Its negative gate checks were independently exercised. This is not 147/147 legacy h2spec compliance or a complete RFC 9113 certification.

The h2spec fixture consumes request bodies before responding, avoiding accidental early-response cancellation of malformed live streams. Its TLS client disables verification only for that tool's local self-signed fixture; the independent Python client separately verifies the local CA and hostname. The tool was built from `summerwind/h2spec` commit `af83a65f0b6273ef38bf778d400d98892e7653d8`.

HPACK supports dynamic tables and Huffman decoding. In addition to RFC vectors and malformed-input tests, independent differential checks exercised 2,000 stateful decodes, 2,000 encoder checks and 5,000 malformed blocks. The encoder deliberately uses static indices and never-indexed literals; dynamic/Huffman response compression remains outside this implementation. HTTP/2 CONNECT tunneling and extended CONNECT return 501. These limits are documented in the [migration guide](native-http-migration.md).

## Ownership and optimization decisions

The normal product dependency graph contains no Hyper, hyper-util, tokio-util or h2 engine. Independent test clients and benchmark servers still use them. On the macOS host, unique package/version entries in the normal dependency graph fell from 71 to 64 for minimal builds and from 136 to 127 with all features (including workspace packages). Counts are target-dependent. Minimal TCP builds have no Tokio dependency; optional HTTP/3 retains Tokio transitively through h3. Compio supplies the executor and completion I/O, Rustls supplies TLS, and h3/Quinn supply HTTP/3. TLS and WebSocket transports retain explicit futures I/O integration.

Native protocol ownership increases maintained Rust source from the original 3,343 lines to 6,429 lines under `crates/*/src`, including in-module tests. It trades dependency ownership for protocol code and regression obligations; it does not make the implementation smaller. Generated data, third-party tooling, raw traces, source copies and load-test caches remain ignored.

The first native H1 writer regressed throughput. Controlled experiments isolated response write batching/buffer reuse and then a per-worker, per-second Date cache. Both were selected after behavioral review and measurements. [Write batching](../benches/results/native-http-write-batching.md) improved throughput in all six worker/workload combinations while worsening some four-worker saturation p99 values. [Date caching](../benches/results/native-http-date-cache.md) removed two allocations per request and improved the measured one-worker medians; plaintext ranges overlap. Four-worker 5,000/s controls also improved p50 but worsened p99/p99.9 medians: plaintext p99 rose from 0.924 to 1.097 ms and echo from 1.122 to 1.326 ms. The throughput/allocation improvement is retained with this explicit tail-latency tradeoff. Shared-host measurements do not establish the cause. The 5,000/s arrival rate is far below saturation and cannot characterize near-capacity tails. Final Date-enabled product results remain a separate comparison.

The [opportunity report](../benches/results/native-opportunities.md) records the Compio WouldBlock allocation experiment, CPU pinning, live-heap versus RSS churn, and policy/adapter profiles. The Compio patch was not added to the product or vendored. CPU pinning remains configurable because it did not consistently improve throughput. Synthetic policy profiles are partial attribution; bare Hyper does not enforce every Maximus policy, so its comparison is not an equivalent-policy causal experiment.

## Comparative load evidence

The primary comparison contains 217 trials and 215,599,359 checked completed requests. Independent validation reconciles raw client outputs, warmups, thresholds, exit status, commands, CSV rows, source and binary identities, and environment metadata. Of these trials, 193 meet their workload gates. All 24 TLS fixed-arrival trials at 5,000/s and 64 preallocated users fail the zero-drop target; completed responses pass validation. These failed targets remain part of the comparison. Separate sizing attempts and an earlier over-capacity HTTP/2 attempt are retained outside the primary matrix.

A separate 36-cell buffer-size sweep completes 6,414,454 checked responses with no failed or incomplete iterations. Ten cycles of 20,000 simultaneous connections validate 400,000 responses and return to six file descriptors after each close. Live Rust requested bytes step upward by 288 KiB in cycles nine and ten; the cause remains unresolved. The current published runner separately passes a live Kubernetes smoke with 91,415 starts, completions and valid responses.

The 300-second native soak completes 34,849,362 checked responses. Four-worker plaintext throughput improves about 3.6% over the old engine in the measured median, while one-worker native HTTP/2 throughput regresses about 19% for plaintext responses and 17% for echo. Hyper and Actix remain faster in the four-worker plaintext comparison. These results establish neither performance leadership nor the cause of the HTTP/2 gap. See the [comparison report](../benches/results/native-http-comparison.md) for per-workload medians, tails, failed targets and measurement limits.

## Reproduction and limits

Use the commands and requirement map in [qualification](qualification.md), the [benchmark instructions](../benches/README.md), and the [native comparison](../benches/results/native-http-comparison.md). Raw local logs remain under ignored `artifacts/native-http/`; the compact evidence manifest records their hashes and execution scope.

Benchmark review hardened required-metric validation, resume identity checks, CPU sampling intervals and interrupted-iteration detection. The final tracked tooling passed 56 local fixture assertions and two actual local k6 cases. The negative case returned five valid responses, interrupted its sixth iteration and exited successfully; the runner now rejects it because starts exceed completions. These are tooling checks, not Maximus throughput measurements. The comparative campaign used an earlier frozen harness and workload; its raw results receive separate validation, and the report preserves excluded attempts. The new counter adds client work and must be identical across future compared variants.

The tests, differential checks, fuzz campaigns and stress runs are finite. They establish the observed behavior of the recorded source in the tested environments. They do not guarantee absence of bugs, universal performance leadership, production acceptance, arbitrary platform support or untested multi-NUMA scaling.
