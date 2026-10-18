# Reuse the HTTP/1 chunk terminator buffer

PR #7 now retains only HTTP/1 no-trailer completion buffer reuse and its one-byte pipelining regression test. This adds three net production lines in one existing module. Shared request extraction, routing and HTTP/2 are restored byte-for-byte to merged baseline `23ec7c87a55c6c53043b5fc59befda627b08c560`. Dependencies, public APIs, configuration and product unsafe code are unchanged; Compio remains.

The former four-change bundle was not qualified because its H2 tail-latency intervals were inconclusive. Those observations did not identify a bad component. The other three candidates are **deferred**, not diagnosed as regressions. The complete earlier [report](https://github.com/vicotrbb/maximus/blob/d792d98a1cda02e8f65a6d36868f2fceb424c5b5/benches/results/lean-ownership.md), [evidence](https://github.com/vicotrbb/maximus/blob/d792d98a1cda02e8f65a6d36868f2fceb424c5b5/benches/results/lean-ownership-evidence.json) and [trials](https://github.com/vicotrbb/maximus/blob/d792d98a1cda02e8f65a6d36868f2fceb424c5b5/benches/results/lean-ownership-trials.csv) remain in commit history. None of their passing socket results qualify this reduced build.

## Direct evidence

The writer already owns an emptied output buffer when the body ends. Reusing it for `0\r\n\r\n` avoids constructing and growing a short-lived vector. Explicit trailer frames, including an empty trailer map, retain the original path and validation. The existing complete-write/error/activity behavior and final flush are preserved.

| Fixture | Allocation/reallocation calls, before → after | Requested bytes, before → after |
| --- | ---: | ---: |
| Fixed-length control | 7 → 7 | 17,116 → 17,116 |
| Chunked response | 8 → 6 | 17,139 → 17,128 |
| Empty stream | 7 → 5 | 17,083 → 17,072 |
| Trailers control | 12 → 12 | 17,976 → 17,976 |
| Empty trailers control | 8 → 8 | 17,291 → 17,291 |

Counts include fixture construction, not a complete socket request. All five complete wire outputs match between revisions and across Linux x86-64/macOS ARM64. Allocation results also match across architectures. The allocation build uses a fixed valid Date. Timing uses the normal allocator, validates payloads before measurement, and keeps a byte-identical A/A binary alongside the pair.

| Focused case | Linux time change (95% interval) | ARM64 time change (95% interval) |
| --- | ---: | ---: |
| fixed | +1.24% [-0.43%, +2.99%] | +2.61% [+1.48%, +3.84%] |
| chunked | -5.09% [-6.02%, -4.13%] | -7.96% [-9.20%, -6.56%] |
| empty | -1.35% [-2.34%, -0.32%] | -8.01% [-9.31%, -6.57%] |
| trailers | -3.63% [-4.79%, -2.47%] | +0.48% [-0.47%, +1.47%] |
| empty-trailers | -7.79% [-10.33%, -5.54%] | +1.24% [-0.11%, +2.67%] |

Each focused comparison has 25 rotating repetitions. The ARM64 fixed-length control has an adverse signal: +2.61%, with a 95% paired bootstrap interval of +1.48% to +3.84%. A/A does not erase it or identify its cause. It remains disclosed and unresolved for that fixture; neither every-path improvement nor universal performance parity is claimed. The affected chunked/empty fixtures improve on both architectures. Socket qualification below is a separate, predefined engineering screen.

The Linux qualification executable shrinks by 176 bytes (3,451,992 → 3,451,816). Its text section shrinks by 360 bytes, while BSS grows by 320 bytes; the ELF section total falls by 40 bytes. The H2 library artifact has the same SHA-256 before and after, corroborating the restored implementation; this is not a claim that every linked server layout is identical.

## Reduced-build socket qualification

All three workloads pass every predefined socket gate. The 60 timed trials complete 24,421,641 requests; six preflights add 48. There are zero request/check failures, dropped iterations or client errors.

| Workload | Checked requests | Throughput | CPU/request | p99 | p99.9 | Gates |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| plain | 8,939,595 | -0.26% | +0.24% | +0.21% | +1.37% | pass |
| stream | 8,086,563 | +0.69% | -2.05% | -1.04% | -1.36% | pass |
| tls-stream | 7,395,483 | +0.57% | -2.09% | -0.50% | -0.43% | pass |

Exactly ten alternating baseline/candidate pairs per workload use 64 VUs for 20 seconds, after an eight-iteration preflight on each build/workload. Latencies cover requests completing after five seconds. Throughput and server process CPU/request include the whole run; CPU excludes separately scheduled kernel work. Every response checks status, complete body and HTTP/1.1. TLS is restricted to TLS 1.3 and verifies a fresh localhost certificate. Any request/check failure, dropped iteration or nonzero client exit stops the controller. Counts must be positive and match starts, completions and HTTP requests.

The server uses one io_uring worker pinned to CPU 2; k6 1.6.0 uses CPUs 4/6, separate physical cores on the shared AMD Ryzen 7 5825U/Linux 6.6.68 host. Both containers have two-CPU limits. There is no exclusive host reservation and no concurrent build or benchmark from this task on that host. Rust 1.96.1, identical lockfiles, the same build path, thin LTO and one codegen unit are used. Local focused probes use Rust 1.96.0 on ARM64.

The fixed socket gates require the paired 95% bootstrap interval to stay above −3% for throughput, below +3% for CPU/request, and below +5% for p99 and p99.9, with zero validity failures. All observations and A/A controls are retained. No additional repetitions are allowed to obtain a pass. These bounds are scoped regression screens, not maximum capacity, competitor rankings or an overall 95% guarantee across all measurements.

## Correctness and reproduction

The exact reduced source passes formatting, warnings-denied Clippy, feature and documentation builds, 159 local test executions, 137 Linux test executions and downstream compilation. One unchanged privileged eBPF test remains skipped. Independent HTTP/1 plain/TLS, HTTP/2 TLS, verified QUIC HTTP/3, WebSocket and feature clients pass, including concurrent uploads and 2,062 wire mutations/disconnects with post-stress health checks. The permanent regression test verifies exact no-trailer chunk boundaries between pipelined responses through one-byte I/O; existing tests cover explicit trailers.

The [evidence manifest](lean-ownership-evidence.json) records source/build identities, all focused observations, paired intervals and reproduction inputs. The [trial CSV](lean-ownership-trials.csv) contains this reduced build's complete socket campaign, including preflights. Historical bundle data is linked above, not silently relabeled. Temporary harnesses, binaries and raw logs remain ignored under `artifacts/h1-retained/`.

The temporary namespace was deleted and verified absent after the complete result archive was downloaded and checksum-verified.
