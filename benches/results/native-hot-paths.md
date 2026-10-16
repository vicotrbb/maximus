# Native HTTP/2 hot-path experiments

This campaign starts from Maximus `a13603b`, selects the HTTP/2 changes built at `28e0525`, then tests a larger TLS adapter limit at `2b59af8`. Final source `b6a08b8` keeps that limit and corrects its admission accounting to apply only to TCP TLS. The binaries are recorded separately: initial HTTP/2 comparisons use `28e0525`; final-source large-body, sustained-load and longer offered-load checks use `b6a08b8`. Compio, Rustls and h3/Quinn remain; TLS and WebSockets keep their established futures-I/O integration. Product builds still exclude Hyper, hyper-util, tokio-util and the h2 engine. Public application APIs and protocol features are preserved.

## Selected changes

- Put END_STREAM on the final buffered DATA frame, removing an empty DATA frame and an extra scheduling/write turn. Streaming bodies retain their exhaustion/trailer path; content-length and flow-control checks stay enabled.
- Validate outbound headers in their existing HeaderMap and pass borrowed fields directly into HPACK. This removes copies and temporary containers without weakening field validation or duplicate-header accounting.
- Return completed write buffers to the connection after successful write and flush. Reuse is bounded to twice `io_buffer_bytes` (32 KiB with the default); larger allocations are discarded. This can retain capacity on idle connections, so fewer allocations do not imply lower resident memory.
- Share the existing per-worker, per-second Date cache between HTTP/1 and HTTP/2. No custom date formatter or new dependency was introduced.
- Enable Compio networking, compatibility and protocol features only in the crates that consume them. Add isolated-crate dependency checks and compile every fuzz target in the normal check script.
- Keep the TLS ciphertext adapter’s initial size, but allow its maximum to grow to twice `io_buffer_bytes`. This lets record overhead fit without a second native write. TCP TLS admission reserves the additional two buffer units (32 KiB per default connection); QUIC accounting is unchanged. No new product dependency or public API is added.

## Experiment selection

The first selection covers HTTP/2 engine changes before the TLS adapter experiment.

The allocation diagnostic uses the process-wide System allocator, including server scheduling and the client, with 100 warmup and 10,000 measured keepalive requests. The cleartext HTTP/2 diagnostic client uses stack buffers and validates status, body and stream completion. Each entry is the median of three runs. These are requested allocation bytes, not live heap or RSS. The same diagnostic source was overlaid on every archived variant.

| Cumulative variant | Allocations/request | Allocated bytes/request |
| --- | ---: | ---: |
| Control | 65.00 | 9,659.22 |
| Final DATA ends stream | 59.00 | 8,914.00 |
| Borrowed outbound headers | 43.00 | 7,737.50 |
| Bounded write reuse | 40.00 | 7,500.95 |
| Shared Date cache, H2 selection | 38.00 | 7,443.04 |
| Fixed-array frame header experiment | 37.00 | 7,865.27 |
| Fixed array without write reuse | 40.00 | 8,103.50 |
| Neither fixed array nor write reuse | 41.00 | 7,679.91 |

The selected combination reduces measured allocation count by about 42% and requested bytes by about 23%. HTTP/1 stays near 16 allocations and 3,080 bytes per request. Static route/body dispatch remains zero allocations; static text/header conversion remains two allocations. These scopes do not establish allocation-free HTTP.

The first exploratory HTTP/2-over-TLS ablation of the `/plaintext` endpoint used three five-second trials per cumulative variant. Median RPS was 35,809 → 42,970 → 45,090 → 43,721 → 45,714. Buffer reuse initially looked slower despite saving allocations, so two additional paired campaigns tested that interaction rather than assuming fewer allocations meant faster execution.

| Paired campaign, three 10-second trials | `/plaintext` RPS median | 1 KiB echo RPS median |
| --- | ---: | ---: |
| Fixed-header comparison: selected combination | 46,636 | 34,002 |
| Fixed-header comparison: add fixed array | 45,865 | 33,317 |
| Fixed-header comparison: array, remove reuse | 47,689 | 33,784 |
| Selection comparison: selected combination | 47,593 | 34,569 |
| Selection comparison: array, remove reuse | 47,092 | 34,243 |
| Selection comparison: remove both | 44,799 | 33,756 |

The fixed-array read saved one allocation but added about 422 requested bytes per request and did not consistently improve throughput. It is rejected. Bounded reuse stays because the final combination uses fewer allocations/bytes and led both paths in the final selection medians. Ranges overlap, and the small gains over the array/no-reuse variant are not proof of statistical superiority. The complete trial rows retain the variation; cumulative changes cannot be assigned additive causal speedups.

## Matched comparisons and final validation

The comparison and final-validation set contains 170 trials and 133,879,773 checked responses, with zero failed checks, HTTP/network failures, dropped iterations or incomplete k6 iterations. The primary rate comparisons use three ten-second trials per cell, plus a three-repeat thirty-second TLS H1 echo follow-up and two five-minute sustained runs. Table engine labels identify builds: `control` is a13603b, `maximus` is the initial H2 selection 28e0525, `tlsbuffer` is the larger-adapter experiment 2b59af8, and `tlsfinal` is corrected final source b6a08b8. These are distinct binaries; the final source is measured again for large bodies, sustained load and the longer offered-load follow-up. A six-trial follow-up uses thirty-second measurements at 1,000 requests/second, starting with the final TLS engine and rotating order across repeats. Tables report medians of trial rates and percentiles, not pooled percentiles.

The complete CSV contains 229 completed attempt records: these 170 comparison/validation trials, 57 exploratory trials and the two failed arrival targets below. It records 154,018,168 completed responses. A separate initial sustained attempt has no valid completion result because its 1 GiB k6 container was OOM-killed; its environment and partial RSS samples remain in the evidence manifest.

The initial 15,000/s plaintext control failed the zero-drop gate. Its result and a separate selected-version attempt at the same target remain below; neither is pooled into the three-repeat 5,000/s controls. All started requests completed with valid responses. Drops mean the offered-load target was not met, even when completed requests succeeded. The warning reports exhaustion of 512 active VUs. Connection/startup pressure is a possible contributor; these one-off attempts do not establish sustainable server capacity or a framework ranking.

| 15,000/s attempt | Completed valid responses | Dropped arrivals | Exit |
| --- | ---: | ---: | ---: |
| control | 149,023 | 979 | 99 |
| maximus | 149,530 | 472 | 99 |

The initial H2 selection medians improve by 29.64% on `/plaintext` and 26.61% on 1 KiB `/echo` versus the control. Hyper remains ahead by about 4% and 1%, respectively; Maximus slightly exceeds Axum here. Median server CPU accounting falls from 21.58 to 16.40 microseconds per response and from 28.82 to 22.41, respectively. These are wider measurement-interval observations, not pure application CPU times. HTTP/1 plaintext medians are about 0.6% below the control at both one and four workers, with overlapping ranges; Hyper and Actix remain faster. TLS HTTP/1.1 medians improve by about 2.7% and 1.5% versus control, with overlapping ranges. No universal performance leadership is established.

### Initial H2 selection over TLS, 64 VUs

| Endpoint | Workers | Engine | RPS | p99 ms |
| --- | ---: | --- | ---: | ---: |
| `/plaintext` | 1 | control | 35,729 | 6.524 |
| `/plaintext` | 1 | maximus | 46,319 | 6.055 |
| `/plaintext` | 1 | hyper | 48,065 | 5.788 |
| `/plaintext` | 1 | axum | 45,777 | 6.109 |
| `/echo` | 1 | control | 26,877 | 8.158 |
| `/echo` | 1 | maximus | 34,029 | 7.992 |
| `/echo` | 1 | hyper | 34,254 | 8.192 |
| `/echo` | 1 | axum | 33,007 | 7.659 |

### HTTP/1.1 plaintext at the initial selection, wrk with 128 connections

| Endpoint | Workers | Engine | RPS | p99 ms |
| --- | ---: | --- | ---: | ---: |
| `/plaintext` | 1 | control | 118,166 | 1.641 |
| `/plaintext` | 1 | maximus | 117,433 | 1.630 |
| `/plaintext` | 1 | hyper | 128,389 | 1.754 |
| `/plaintext` | 1 | axum | 116,825 | 1.695 |
| `/plaintext` | 1 | actix | 129,245 | 1.401 |
| `/plaintext` | 4 | control | 371,144 | 2.173 |
| `/plaintext` | 4 | maximus | 368,851 | 2.072 |
| `/plaintext` | 4 | hyper | 411,599 | 0.614 |
| `/plaintext` | 4 | axum | 371,063 | 0.823 |
| `/plaintext` | 4 | actix | 411,351 | 2.778 |

### HTTP/1.1 over TLS at the initial selection, 64 VUs

| Endpoint | Workers | Engine | RPS | p99 ms |
| --- | ---: | --- | ---: | ---: |
| `/plaintext` | 1 | control | 43,185 | 7.232 |
| `/plaintext` | 1 | maximus | 44,332 | 7.108 |
| `/plaintext` | 1 | hyper | 43,807 | 7.440 |
| `/plaintext` | 1 | axum | 44,098 | 7.292 |
| `/echo` | 1 | control | 39,565 | 8.414 |
| `/echo` | 1 | maximus | 40,174 | 8.201 |
| `/echo` | 1 | hyper | 39,242 | 8.337 |
| `/echo` | 1 | axum | 39,399 | 8.516 |

### Initial H2 selection over TLS, 128 KiB echo, 64 VUs

| Endpoint | Workers | Engine | RPS | p99 ms |
| --- | ---: | --- | ---: | ---: |
| `/echo` | 1 | control | 1,891 | 45.172 |
| `/echo` | 1 | maximus | 1,969 | 43.214 |
| `/echo` | 1 | hyper | 3,967 | 22.815 |
| `/echo` | 1 | axum | 3,878 | 24.387 |

The 128 KiB echo remains a substantial gap: selected Maximus reaches 1,969 RPS versus Hyper’s 3,967 and Axum’s 3,878, despite improving about 4% over the control. Small-response gains do not generalize to large buffered bodies.

### TLS adapter follow-up

A local diagnostic encrypted and decrypted a fixed 128 KiB response in eight flushed batches. Raising only the adapter maximum from 16 KiB to 32 KiB reduced native completion writes from 16 to 8; both settings emitted 16 TLS records and 131,507 ciphertext bytes. Three repetitions were identical. The 1 KiB case remained one write/record. This proves the operation-count mechanism, not an equivalent throughput gain. The candidate permits 32 KiB more retained adapter capacity per default TCP TLS connection; admission reserves it. A review caught and corrected an unrelated QUIC admission reduction before final selection.

| Campaign | Endpoint | Engine | RPS | p99 ms |
| --- | --- | --- | ---: | ---: |
| 128 KiB H2, experiment | `/echo` | maximus | 1,977 | 41.743 |
| 128 KiB H2, experiment | `/echo` | tlsbuffer | 2,198 | 42.591 |
| 128 KiB H2, experiment | `/echo` | hyper | 3,925 | 23.433 |
| Small / 1 KiB H2 | `/plaintext` | maximus | 44,567 | 6.415 |
| Small / 1 KiB H2 | `/plaintext` | tlsbuffer | 46,171 | 5.983 |
| Small / 1 KiB H2 | `/echo` | maximus | 33,961 | 8.010 |
| Small / 1 KiB H2 | `/echo` | tlsbuffer | 33,744 | 7.952 |
| Small / 1 KiB TLS H1 | `/plaintext` | maximus | 43,806 | 7.096 |
| Small / 1 KiB TLS H1 | `/plaintext` | tlsbuffer | 44,008 | 7.175 |
| Small / 1 KiB TLS H1 | `/echo` | maximus | 40,113 | 8.074 |
| Small / 1 KiB TLS H1 | `/echo` | tlsbuffer | 38,744 | 8.479 |
| 128 KiB H2, final source | `/echo` | maximus | 1,990 | 43.431 |
| 128 KiB H2, final source | `/echo` | tlsfinal | 2,264 | 38.351 |
| 128 KiB H2, final source | `/echo` | hyper | 3,938 | 24.718 |
| TLS H1 echo, 30s follow-up | `/echo` | tlsfinal | 42,176 | 6.969 |
| TLS H1 echo, 30s follow-up | `/echo` | maximus | 42,563 | 6.847 |

The adapter experiment improves 128 KiB throughput by 11.2% over the initial H2 selection, with nonoverlapping ranges across three repeats. Measured CPU per response falls about 5.5%, but p99 rises from 41.743 to 42.591 ms and one candidate trial reaches 55.422 ms. Small H2/text and TLS H1/text results overlap. The ten-second TLS H1 echo comparison shows a 3.4% lower median; the separate thirty-second final-source follow-up shows only a 0.9% lower median (42,176 versus 42,563 RPS), with mixed paired changes and overlapping ranges. This does not establish identical performance or a latency win. The larger adapter is retained for the repeated large-body throughput/CPU benefit, accepting its documented memory allowance.

The corrected final-source confirmation reaches 2,264 RPS versus 1,990 for the initial H2 selection, a 13.7% gain in that matched campaign. Its median p99 is 38.351 versus 43.431 ms and CPU accounting is 369.52 versus 399.85 microseconds per response. Hyper reaches 3,938 RPS with 24.718 ms p99 in the same campaign. The 11.2% experiment and 13.7% confirmation are separate estimates of the adapter change and must not be added; differences between those campaigns do not establish a speedup from the admission-scope correction.

### Matched offered-load controls

| Protocol | Endpoint | Offered RPS | Control / tested build p50 ms | Control / tested build p99 ms | Control / tested build p99.9 ms |
| --- | --- | ---: | ---: | ---: | ---: |
| HTTP/2 TLS | `/plaintext` | 1,000 | 0.104 / 0.102 | 0.226 / 0.690 | 2.130 / 3.585 |
| HTTP/2 TLS | `/echo` | 1,000 | 0.119 / 0.113 | 0.282 / 0.268 | 3.673 / 3.225 |
| HTTP/1.1 TLS | `/plaintext` | 1,000 | 0.090 / 0.095 | 0.209 / 0.241 | 2.732 / 2.713 |
| HTTP/1.1 plaintext, 5,000/s | `/plaintext` | 5,000 | 0.089 / 0.092 | 1.061 / 1.199 | 3.065 / 4.361 |
| HTTP/2 TLS, final source, 30s | `/plaintext` | 1,000 | 0.105 / 0.099 | 0.226 / 0.208 | 1.953 / 1.484 |

The short offered-load controls show mixed tails, including a worse initial-selection H2 small-response p99. The thirty-second final-build comparison has a lower p99 (0.208 versus 0.226 ms), but its duration and build both differ from the original ten-second run, so it does not isolate the cause of that earlier result. These controls are well below saturation and do not establish near-capacity latency.

### Five-minute HTTP/2 echo runs

| Engine | Checked responses | RPS | p99 ms | First / last sampled RSS KiB | Largest sampled RSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| control | 8,407,698 | 28,026 | 5.849 | 9,472 / 9,472 | 9,472 |
| tlsfinal | 11,010,477 | 36,702 | 6.599 | 9,472 / 9,472 | 9,472 |

The completed sustained runs contain 19,418,175 valid responses. Final-source throughput is about 31% higher, while p99 is higher at its greater achieved rate. All 52 baseline and 52 final RSS samples are 9,472 KiB; this is stable sampled RSS for this 1 KiB workload, not a continuous peak or a general memory-leak proof. The original 1 GiB-client attempt is incomplete and excluded from these rows; both completed runs use a 4 GiB client limit and the unchanged 4 GiB server limit.

## Dependency scope

Unique `(package, version)` entries in the Linux x86-64 normal dependency graph include the root package and exclude build/dev dependencies:

| Build | Before | Selected |
| --- | ---: | ---: |
| Minimal Maximus | 65 | 65 |
| All-feature Maximus | 129 | 129 |
| Standalone maximus-core | 56 | 52 |
| Standalone maximus-io | 52 | 46 |

Feature scoping removes unnecessary networking adapters from standalone components; the full server still needs those packages. No dependency versions change. Reproduce the graph with `cargo tree --locked --target x86_64-unknown-linux-gnu -p PACKAGE --no-default-features --edges normal --no-dedupe --prefix none --format '{p}'`; count unique package/version pairs, or substitute `--all-features` for the full server. Replacing established TLS/QUIC implementations or small shared utilities was not justified by these measurements. Minimal TCP has no Tokio dependency; optional HTTP/3 retains Tokio utilities transitively without starting a Tokio executor.

## Correctness and limits

The selected source passed `scripts/check.sh`: formatting, warnings-denied Clippy, all-feature and minimal tests, isolated feature builds, dependency guards, all fuzz-target compilation and warnings-denied documentation. The final macOS CI check executed 151 passing tests across configurations, not 151 distinct features. Linux ARM64 executed 130 passing tests with the separately gated privileged BPF test ignored. [GitHub CI](https://github.com/vicotrbb/maximus/actions/runs/38951145784) passed both jobs: Linux x86-64 executed 130 passing tests (one privileged BPF test ignored) plus independent interoperability checks; macOS passed the main checks, downstream application compilation and patched h3 tests.

Independent clients passed HTTP/1 plaintext and TLS, HTTP/2 ALPN, HTTP/3 QUIC, JSON/errors, multipart, SSE, gzip, CORS, HEAD, typed routing, text/binary WebSockets and sixteen concurrent uploads. Raw socket stress completed 2,062 mutations/disconnects with sixteen clients and a healthy request afterward. Added regressions cover final DATA framing at frame/window boundaries, content-length errors, trailers, duplicate header limits, partial writes, flush failures, TLS adapter write bounds and TCP/QUIC admission boundaries. The final source review found no actionable issues.

AddressSanitizer/libFuzzer completed 4,363,148 HTTP/2 executions and 5,450,305 stateful HPACK executions without a crash, 181 seconds per target. HTTP/2 used a 16,384-byte input limit; the HPACK run used libFuzzer’s corpus-derived 7,087-byte limit. The H2 fuzz run uses `d802594`; its protocol source and HPACK source are unchanged in the final selection. The subsequent TLS adapter and admission changes are covered by their focused tests, final interoperability checks and TLS conformance rerun. Generated corpus entries are ignored. These are finite executions, not distinct cases or exhaustive coverage.

Both plaintext and TLS h2spec runs execute all 147 legacy cases: 139 pass, five retain the previously documented RFC 7540/9113 closed-stream policy differences, and three race with response completion. Each of those three must additionally pass against a fixture that holds the response open. Both held-response runs passed 3/3; an old fixture failed this gate, and twelve controlled gate scenarios were checked. A deterministic wire regression establishes the half-closed state with a PING barrier and verifies STREAM_CLOSED plus a healthy second stream. The wrapper rejects unknown failures, missing/duplicate cases and skips. It does not claim 147/147 legacy compliance or complete RFC certification. See [qualification](../../docs/qualification.md) for the exact commands and [RFC 9113 §5.1](https://www.rfc-editor.org/rfc/rfc9113.html#section-5.1) for closed-stream processing.

Existing BPF privileged qualification and the older coverage percentage are not rerun or reattributed to this change. No new unsafe product code is introduced.

## Reproduction

[Trial rows](native-hot-path-trials.csv), [allocation rows](native-hot-path-allocations.csv) and the [identity/command manifest](native-hot-path-evidence.json) retain exploratory, selection and final measurements. Full logs, generated certificates, source archives, vendor caches and binaries remain in ignored local artifacts.

Use the [Kubernetes archive runner](../README.md#kubernetes-archive-comparisons-and-resume). Archive each commit named in the manifest, overlay the same selected `benches/baselines/src/bin/allocations.rs` only for the diagnostic, record that overlay, and build the isolated benchmark workspace with `cargo build --manifest-path benches/baselines/Cargo.toml --release --locked`. Do not compile during timed trials. Supply each archive and binary through `--binary-map`; engine labels can differ from their `argument` (`maximus`, `hyper`, `axum`, `actix`). The `maximus` entry uses the same initial-selection binary as the exploratory `candidate` entry. `tlsbuffer` identifies the adapter experiment, and `tlsfinal` identifies the corrected final source; do not combine those labels as though they were one build. The 128 KiB workload changes only `BODY_BYTES || 1024` to `BODY_BYTES || 131072` in a separate copy of `load.js`; its hash is recorded separately.

The frozen comparison lockfile resolves Hyper 1.12.0, Axum 0.8.9 and Actix Web 4.17.0, with Tokio 1.53.3 for those baselines; Maximus uses Compio 0.19.2 and TLS uses Rustls 0.23.46.

All compared builds use Rust 1.96.1, thin LTO and one codegen unit on one shared AMD Ryzen 7 5825U Linux host. The server uses the first N CPUs of configured mask `0,2,4,6`, where N is the worker count: CPU `0` for one worker and `0,2,4,6` for four. Client mask `8,10,12,14` uses disjoint physical cores. Each container has a four-CPU quota. The server has a 4 GiB memory limit throughout. The client initially has 1 GiB; after an incomplete five-minute control attempt was OOM-killed, the owned Pod was recreated with a 4 GiB client limit and the original binary, source, tool and certificate artifacts restored. Subsequent TLS experiments and final validation use that larger client allowance; memory limits are recorded per campaign. Four-worker processes permit migration within their mask; one-worker servers stay on CPU `0`. The powersave governor, other host workloads, shared caches and interrupts remain confounders. Maximus explicitly selects io_uring for the timed server trials. The comparison binary raises its admission budget to 4 GiB and allows persistent benchmark connections; production defaults are unchanged. k6 1.6.0 and Debian wrk 4.1.0 use identical response-checked workloads within each comparison. Each timed trial has a separate three-second warmup process, and engine order rotates across repeats. New measurement processes establish new connections; TLS setup contributes to startup pressure, while k6 request-duration percentiles exclude the full handshake.

TLS uses the same certificate, verified local CA, Rustls/Ring, TLS 1.3 and ALPN assertions. HTTP/2 stream count and receive windows are matched: 32 concurrent streams, a 65,535-byte stream window and a 1 MiB connection window. The Hyper TLS branch includes the harness's shared router/body adapter and is not a minimal Hyper lower bound. Actix is compared only in the plaintext harness; HTTP/3 performance is not measured. The TLS echo workload buffers the response. These tests do not measure arbitrary middleware, streaming throughput, many-stream single-connection saturation, multi-NUMA scaling or production traffic.

Closed-loop latency changes accompany different achieved rates. Fixed-arrival controls use the same offered load but are well below measured saturation, so they do not characterize near-capacity tails. k6 request-duration percentiles and wrk corrected latency histograms have different semantics and must not be pooled. CPU snapshots cover launch, measurement, result observation and drain; CPU microseconds per response are accounting observations over that wider interval. RSS samples are observations, not continuous peaks or proof of live-heap behavior. Results from a shared host and finite tests cannot establish universal performance leadership or absence of bugs.
