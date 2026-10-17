# Single-pass HPACK lookup

The final candidate changes only the HPACK static-table lookup. One scan now remembers the first matching name while still preferring an exact name/value entry. Unknown names remain never-indexed literals. The direct header-buffer encoder from the first draft was removed entirely.

The runtime diff is **two additional source lines in one function**. No new dependencies, fields, public APIs, configuration or unsafe code. Compio remains. Buffering, framing, stream scheduling, flow control and limits match baseline `fe4129c95fc8a728f9b970087f43286b60a209fb`.

## Footprint and focused measurements

| Measurement | Baseline | Candidate |
| --- | ---: | ---: |
| ARM64 comparison executable, bytes | 9,528,072 | 9,528,072 |
| ARM64 text, bytes reported by `size` | 6,972,789 | 6,972,593 |
| x86-64 comparison executable, bytes | 9,884,456 | 9,884,328 |
| x86-64 text, bytes reported by `size` | 7,867,135 | 7,867,007 |
| Allocation calls in connection probe | 70 | 70 |
| Requested bytes in connection probe | 319,816 | 319,816 |
| Peak tracked live bytes | 268,780 | 268,780 |
| Tracked live bytes after completion | 0 | 0 |

The x86-64 BSS grows by 128 bytes, leaving the sum of text/data/BSS unchanged. ARM64 data and BSS are unchanged. Executable size includes comparison-framework code and is not the size of the Maximus crate. The rebuilt final ARM64 executable matches the frozen candidate byte-for-byte.

A preallocated-output probe encodes each header one million times per execution. Nine alternating repetitions on Linux ARM64 give these median nanoseconds per field. It checks decoded values before timing and prevents constant folding with `black_box`.

| Header | Baseline ns | Candidate ns |
| --- | ---: | ---: |
| `content-type: application/json` | 34.40 | 25.61 |
| `content-length: 1024` | 36.59 | 25.58 |
| `server: maximus` | 51.51 | 29.68 |
| Unknown `x-custom: opaque` | 50.15 | 28.29 |
| Non-static `:status: 418` | 71.59 | 63.10 |

The complete probe includes exact matches too. A second probe covers all 61 static-table positions, representing 52 distinct names, with exact values and a non-static literal value (200,000 encodes each, seven alternating repetitions). The median change across positions is −28.4% for literals and −0.3% for exact matches. Some exact cases increase by about 1 ns; the `:authority` literal increases by 0.5 ns. This is a literal-path optimization, not a claim that every input is faster. Samples for the first probe and per-position medians/ranges for the second are in the [evidence manifest](lean-io-evidence.json). These are codec timings on a shared local VM, not application throughput predictions. The connection allocation probe uses the real SETTINGS/HEADERS/DATA path and validates a 128 KiB echo. Its memory results are unchanged across three final runs.

To reproduce the codec probes, extract `reproduction.focused_source` or `reproduction.matrix_source` from the manifest into `probe.rs` in a temporary directory. Write the chosen revision's `crates/http2/src/hpack.rs` to `codec.rs`, appending `reproduction.codec_append` for the matrix probe. Build and run with the recorded `reproduction.compile` and `reproduction.run` commands on Linux. Alternate baseline/candidate order; keep the compiler, CPU, flags and repetitions identical. The temporary table accessor is absent from the product. The manifest also retains the k6 workload and homelab runner source.

## Homelab socket measurements

All values below are medians of five runs per revision; arrows show baseline → candidate. `/plaintext` is the response path; every socket phase uses TLS HTTP/2.

| Workload | Responses/s | Process CPU µs/response | p99 ms | p99.9 ms | Post-run RSS MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 KiB echo, 5,000/s, 64 VUs | 4,999.8 → 5,000.1 | 31.397 → 30.865 | 1.096 → 1.128 | 5.024 → 4.629 | 8.500 → 8.375 |
| 1 KiB echo, 5,000/s, 96 VUs | 5,000.2 → 5,000.1 | 32.994 → 32.375 | 1.396 → 1.269 | 5.166 → 5.106 | 9.500 → 9.375 |
| `/plaintext`, closed loop, 64 VUs | 23,545.2 → 23,488.8 | 20.159 → 19.788 | 10.775 → 10.802 | 16.439 → 16.182 | 8.250 → 8.250 |
| 128 KiB echo, closed loop, 64 VUs | 2,463.1 → 2,550.7 | 317.486 → 299.121 | 44.210 → 43.370 | 55.012 → 53.209 | 19.973 → 20.469 |

Fixed-rate responses/s uses the final 20-second response count; closed-loop rates use the whole run. All 8,834,687 responses in the 42 trials with matching started/completed counts passed HTTP/body/protocol checks. Across the pilot and paired phases, 515 arrivals were missed and 8 trials failed a load threshold. Those are failed offered-load targets, even though completed responses were correct.

- `small-fixed` / `before`: 99 dropped arrivals; 3/5 failed threshold trials.
- `small-fixed` / `option`: 9 dropped arrivals; 2/5 failed threshold trials.
- `fixed-admitted` / `before`: 70 dropped arrivals; 1/5 failed threshold trials.
- `fixed-admitted` / `option`: 0 dropped arrivals; 0/5 failed threshold trials.

End-to-end differences are descriptive observations on a shared host. The small 64-VU case has slightly worse median p99 and the plaintext case slightly lower throughput; these runs do not establish statistical equivalence. Large-response throughput and CPU favor the candidate in all five pairs, but that magnitude cannot be attributed entirely to this small codec change. Large-response sampled RSS rises in four pairs (median +520,192 bytes, 2.5%). Its median p99.9 improves, but two pairs worsen, including 73.84 ms versus 54.95 ms. Tracked allocations are unchanged; neither sampled memory nor latency improves in every run.

Rust 1.96.1, locked dependencies, thin LTO and one codegen unit; k6 1.6.0; verified TLS 1.3 and HTTP/2. A temporary pod runs on an AMD Ryzen 7 5825U worker. Server CPU 2 and client CPUs 4/6 are different physical cores. Each container has a two-CPU limit. Other services remain on the machine, so affinity does not mean exclusive ownership. No task compilation or competing benchmark runs on that host during socket trials.

Five pairs per main phase alternate baseline/candidate order. Each run lasts 25 seconds. Latency statistics exclude the first five seconds; all requests still undergo correctness checks. Fixed-rate phases ramp from 100/s to 5,000/s during those five seconds, then hold 5,000/s for 20 seconds. Reported CPU uses process user+system ticks divided by all requests, including warmup. It excludes separately scheduled kernel work. RSS is a post-run sample, not peak memory. The CSV's whole-run RPS includes the ramp; `steady_requests` counts responses completing in the last 20 seconds.

The initial 10-second pilot used an immediate 5,000/s target. The first ramped phase used 64 VUs; the subsequent admitted-pool check uses 96. A separate 256-VU diagnostic exceeded the configured 117-connection allowance: 112,739 iterations started but only 112,600 completed before the grace period expired. The counter-equality gate rejected it and the campaign stopped before running the candidate. That invalid baseline-only attempt is retained in the manifest, separately from paired performance trials. Every paired trial and the initial pilot, including missed arrivals and failed thresholds, remain in the [CSV](lean-io-trials.csv). These comparisons do not establish server capacity or a ranking against other frameworks.

## Correctness and scope

- Final-source formatting, Clippy, feature builds, docs and 157 test executions pass. Linux has 135 passes and one privileged eBPF skip. Downstream-consumer compilation passes.
- Tests cover all 61 static entries and exact/name-only/unknown-name cases. A separate deterministic differential check verifies byte-identical encoding of 100,000 header lists, including opaque bytes, repeated fields and output prefixes.
- Independent HTTP/1.1, TLS HTTP/2, QUIC HTTP/3 and WebSocket clients pass, including extraction, JSON, multipart, SSE, compression, CORS, HEAD and concurrent uploads. All 2,062 raw-wire mutation/disconnect checks pass, including post-stress health.
- Final-source HPACK and HTTP/2 fuzz runs complete 1,676,394 and 1,724,505 executions without a finding.
- The existing plaintext/TLS h2spec policy gates pass. Each raw legacy suite is 139/147: five documented RFC policy differences and three closure races; all three pass held-response reruns. This is not 147/147 unmodified legacy-suite conformance.
- Independent source review found no actionable correctness or simplification issues.

The earlier direct-encoding, traversal, ownership, scheduling, PGO and window experiments are **not retained changes**. Their full results, including unfavorable trials, remain accessible at [the original draft commit](https://github.com/vicotrbb/maximus/tree/25589aee52d8eeba86354dd2dd16a0a6c5513aa7/benches/results). The current tree keeps the smaller final report and data. Temporary launchers, probes, binaries, profiles, corpora, certificates and raw logs remain ignored under `artifacts/lean-gains/` and `artifacts/lean-io/`.
