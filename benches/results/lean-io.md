# HTTP/2 header allocation experiments

**Draft candidate: the no-regression performance gate is not met.** Correctness checks pass, but small-body socket results are mixed and the comparison executable grows. Do not treat the allocation reduction as approval to merge. Acceptance requires resolving those results with controlled measurements and, where needed, further simplification.

The candidate change encodes HPACK directly into the connection output buffer. Reverse insertion makes room for CONTINUATION envelopes when a header block spans frames. It removes the temporary encoded-block vector and the subsequent full-block copy. Static-table encoding also finds an exact value and the first matching name in one scan.

This adds **16 runtime lines**, with no new fields, public APIs, dependencies, configuration or unsafe code. Compio, Rustls and h3/Quinn remain. Stream traversal, scheduling, deadlines, flow-control defaults and admission policy are unchanged. Several broader experiments were rejected after testing; they are recorded below.

Baseline: `fe4129c95fc8a728f9b970087f43286b60a209fb`. The selected variant is `encoding`. [Trial rows](lean-io-trials.csv) retain all socket experiments, including rejected variants and failed load thresholds. [Evidence](lean-io-evidence.json) records source/build identities, probes, phase arguments and qualification. This is a before/after Maximus study, not a new ranking against other frameworks.

## Selected change

The isolated probe processes SETTINGS, HEADERS and eight 16 KiB DATA frames through the real connection, collects the upload and validates a 128 KiB echo. After 20 warmups, it measures 1,000 requests with a temporary System allocator wrapper outside product code. Five baseline and three final Linux release executions agree on the integer means:

| Measurement | Baseline | Selected |
| --- | ---: | ---: |
| Allocation calls/request | 70 | 67 |
| Requested bytes/request | 319816 | 319706 |
| Peak tracked live bytes | 268780 | 268780 |
| Tracked live bytes after completion | 0 | 0 |

This establishes **three fewer allocation calls per request (4.3%)** in this probe. It includes connection setup and protocol/body/output processing, excludes sockets and TLS, and does not establish lower process RSS. The performance goal is reduced temporary allocation and copying; the measured byte reduction is small.

Medians from three repetitions per variant:

| Workload | RPS before → selected | CPU µs/response | p99 ms | p99.9 ms | Post-run RSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| H2 1 KiB echo, 64 VUs, 20s | 51630 → 50905 | 13.35 → 13.12 | 5.691 → 5.857 | 10.446 → 10.406 | 8804 → 8808 |
| H2 1 KiB echo, 5,000/s, 20s | 4984 → 4981 | 39.07 → 40.49 | 7.565 → 7.813 | 13.705 → 15.379 | 8752 → 8820 |
| H2 128 KiB echo, 64 VUs, 20s | 5032 → 5232 | 176.88 → 167.55 | 27.266 → 24.201 | 37.708 → 31.258 | 19120 → 17908 |

Large-body throughput is about 4.0% higher and observed CPU cost about 5.3% lower in this phase. Small-body results are mixed: steady-load CPU is slightly lower, while the fixed-rate phase increases CPU and tail latency. Neither phase cancels the other. The fixed-rate target was missed by both revisions; it is not a passed capacity claim. The deterministic allocation/copy reduction makes this a candidate for review. The mixed socket results **do not satisfy the no-regression acceptance requirement**; this branch remains a draft.

The comparison executable grows **9,528,072 → 9,594,144 bytes** (+0.69%). Rebuilding the final source produced a byte-for-byte match with the measured `encoding` executable. This is not a binary-size optimization. The smaller allocation count must not be presented as a proportional throughput improvement or a universal no-regression guarantee.

## Measurement conditions

Rust 1.96.1, Linux ARM64, thin LTO and one codegen unit. One server worker uses VM CPU 0; k6 1.6.0 uses CPUs 1–2. TLS/H2 uses the same certificate with verified client trust. Each measured trial follows a three-second server warmup in a separate client process; measured client connections start cold. Variant order rotates across three repetitions. No task-initiated compilation or other benchmark overlaps socket measurements.

The shared three-CPU Docker VM also runs eleven unrelated service containers. CPU cost is the server cgroup delta over a wider interval than request timing; percentages use that actual elapsed interval. RSS is a post-run observation, not continuous peak or unique physical memory. Small-response closed-loop clients approached their two-CPU budget, so those throughput figures do not establish server capacity. The final fixed-rate small-body phase leaves more client headroom. Host variation remains a limitation.

All 87 recorded socket trials completed 22,164,685 responses with 0 HTTP failures and 0 incorrect-response checks; every started iteration completed. There were 3,661 missed scheduled arrivals across 17 failed-threshold trials. Those rows remain. The selected encoder's three 5,000/s runs missed 1,018 arrivals; baseline missed 1,660 in that phase. All six large-body final trials passed their thresholds. The CSV also includes earlier variants that were subsequently rejected.

## Rejected traversal and I/O experiments

The allocation count alone did not determine retention:

- **HTTP/1 buffer transfer:** the initial screen found essentially unchanged median CPU for 128 KiB echoes (148.653 → 148.670 µs/response), mixed tails and additional small-request allocation bytes. Reverted; all 24 initial screen trials remain, including dropped arrivals.
- **One stream-ID search at a time:** removed the per-poll ID vector but added a tree search per visited stream. Waiting-stream timings regressed. Rejected.
- **64-ID stack batches:** reduced the isolated upload probe to 45 allocation calls. Waiting-stream microbenchmarks improved or stayed close to baseline, but socket results did not justify the added traversal code and stack work. The first 500/s small-echo phase increased median CPU from 73.63 to 93.98 µs/response; a 30-second steady-load follow-up narrowed the gap to 12.64 → 13.02 µs/response. A borrowed-slice iteration variant was tested as well. The final ablation favored the encoding-only variant on CPU cost. Both batch variants were removed; none of their allocation savings is claimed for the selected change.
- **Input processing during pending writes:** the prototype demonstrated input progress, but independent review found that delayed input could bypass SETTINGS/stream deadlines and shutdown could extend a newly established drain deadline. The scheduler experiment was removed. It needs separate lifecycle hardening; this PR does not solve that existing progress limitation.
- **Scatter/gather writes:** deferred. The current transport boundary would need vectored and partial-write handling without an established TLS gain. No speculative interface or dependency was added.

A separate timing probe constructs 1–1,024 concurrent streams, then measures ready-response processing or 100 polls of waiting handlers. All ready bodies are checked. Five rotated repetitions use executables **without allocator instrumentation**. Median nanoseconds per response (ready) or per stream-poll (waiting) are below. These modes are different operations and cannot be compared with each other. All three candidate columns are rejected traversal variants, not the selected encoder-only implementation.

| Streams | Ready baseline | Ready single-ID / owned batch / borrowed batch | Waiting baseline | Waiting single-ID / owned batch / borrowed batch |
| --- | ---: | ---: | ---: | ---: |
| 1 | 868.71 | 790.60 / 733.03 / 727.12 | 32.56 | 24.16 / 32.23 / 28.57 |
| 32 | 838.09 | 682.73 / 677.08 / 698.72 | 34.60 | 38.00 / 31.84 / 32.79 |
| 128 | 826.89 | 678.73 / 677.59 / 666.21 | 40.41 | 47.91 / 39.97 / 40.52 |
| 1024 | 807.71 | 686.77 / 728.25 / 698.22 | 53.44 | 62.62 / 53.97 / 52.33 |

Earlier timings with the allocator wrapper disabled at runtime remain in the evidence archive. The table uses the subsequent uninstrumented executables and matching HPACK source for each variant. Better isolated timings did not establish a better routed TLS application.

## Optional application tuning

The following probes used the earlier owned-array `batch` prototype. They evaluate tuning mechanisms, rather than qualify the final `encoding` source. No PGO profiles or changed window defaults ship.

A plaintext HTTP/2 probe uploads and validates a 4 MiB echo. A private Docker network namespace applies half the nominal RTT as loopback delay in each direction; server and client use CPUs 0 and 1. The client disables Nagle and honors both receive windows. Connection credit stays at 1 MiB; client response credit is ample. Three sequential connections per setting give these upload-time medians:

| Nominal RTT | 65,535-byte stream window | 262,144-byte stream window |
| --- | ---: | ---: |
| 0 ms | 4.2 ms | 1.3 ms |
| 20 ms | 2.306 s | 0.888 s |
| 80 ms | 8.831 s | 3.058 s |

All 18 echoes validate. The larger-window 80 ms trials range from 1.829 to 5.243 seconds. This is a tuning demonstration without bandwidth shaping, not an internet-capacity prediction. [RFC 9113 §5.2.3](https://www.rfc-editor.org/rfc/rfc9113.html#section-5.2.3) discusses matching windows to bandwidth and delay. Tune `stream_window_bytes` and `connection_window_bytes` with application memory/admission limits; these measurements do not justify increasing framework defaults.

Application PGO follows the [Rust compiler's generate/train/merge/use workflow](https://doc.rust-lang.org/rustc/profile-guided-optimization.html). A temporary application-only shutdown hook lets the instrumented server exit normally and flush profiles. Both control and optimized applications contain that hook; it is absent from the shipped framework and benchmark application. Training covers plaintext, JSON, typed routing and 64 KiB echoes over H1 and TLS/H2 with 16 VUs. All eight training runs validate responses. Evaluation uses separate runs, 128 KiB echoes with 64 VUs, and a fixed-rate H1 route:

| Evaluation | Control → PGO RPS | CPU µs/response | p99 ms | p99.9 ms |
| --- | ---: | ---: | ---: | ---: |
| H2 128 KiB echo, 64 VUs | 4896 → 5317 | 184.33 → 166.24 | 28.013 → 24.420 | 39.937 → 33.505 |
| H1 typed route, 500/s | 500 → 500 | 58.37 → 48.59 | 1.311 → 1.116 | 4.900 → 7.725 |

All 12 evaluation trials validate responses. Large-body throughput improves about 8.6% and observed CPU cost falls about 9.8%. H1 p99.9 worsens despite lower CPU cost. The application executable is 9,594,248 → 8,920,848 bytes (7.0% smaller), but it includes unused comparison-framework code, so this cannot be attributed wholly to Maximus. PGO remains application-specific and needs representative training; it is not a blanket latency improvement.

## Verification and reproduction

The final encoder-only source passes `scripts/check.sh`: formatting, Clippy, feature builds, fuzz-target compilation, documentation and 158 test executions. Linux workspace tests pass 136 executions with one privileged eBPF test skipped. The final-source downstream-consumer build also passes. Vendored H3 separately passes 237 unit/doc tests; that component is unchanged. Existing warnings from vendored H3 remain outside the owned-code Clippy gate.

Independent clients validate plaintext and TLS HTTP/1.1, TLS HTTP/2, QUIC HTTP/3, extraction, JSON, multipart uploads, SSE, compression, CORS, HEAD and WebSocket text/binary echoes. Concurrent upload checks and 2,062 raw-wire mutations/disconnects pass, including post-stress health. Final-source HTTP/2 and HPACK fuzz runs complete 1,760,709 and 1,351,102 executions respectively without a finding.

The existing h2spec policy gate passes on plaintext and TLS. Each raw legacy suite passes 139/147 cases: five documented RFC 7540/9113 policy differences and three response-closure races remain. All three races pass the held-response rerun. This is not a claim of 147/147 unmodified legacy-suite conformance.

Tests protect exact static HPACK entries, name-only fallbacks, output prefixes, multi-frame headers and END_STREAM/END_HEADERS flags. The allocation probe validates every echo and returns tracked live bytes to zero. Independent review found no correctness issues in the retained framing and lookup changes; its scheduler findings caused that prototype to be removed.

Build the locked benchmark workspace with identical compiler/profile for each revision and freeze each executable before load. The existing runner supports the workloads:

```sh
python3 benches/run.py --engines maximus --paths /echo --body-bytes 1024 --protocol h2 --rates 5000 --vus 64 --duration 20s --repeats 3 --client-cpus 1,2 --label small
python3 benches/run.py --engines maximus --paths /echo --body-bytes 131072 --protocol h2 --rates 0 --vus 64 --duration 20s --repeats 3 --client-cpus 1,2 --label large
```

The local adapter mounts each frozen variant read-only as `/server`, invokes `/server maximus`, rotates order and reuses the TLS fixture. Phase arguments, image digests, binary hashes, probe values and log hashes are in the evidence manifest. Temporary sources, allocator instrumentation, profiles, binaries, corpora, keys and raw logs stay ignored under `artifacts/lean-io/`; this report publishes a compact evidence record rather than that full local archive.
