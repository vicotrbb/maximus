# Optional I/O features and rejected allocation experiments

The shipped change narrows optional I/O features and prunes the standalone fuzz lockfile. It preserves the HTTP/2 runtime implementation from baseline `7b03eb31160c9461291a40ba9ad4541286fba1cf`. No new abstraction, dependency or unsafe code is added.

The measurements below compare that baseline with experimental commit `94291485e48505c80b2ab6f2e6b8b250acc24173`, which reused the completed HTTP/2 read future allocation with `Pin::set`. That experiment saved allocations but did not establish performance parity. It was reverted to honor the requirement that efficiency changes preserve performance. **The allocation savings below are not part of the shipped change.**

Plain HTTP/1 enables Compio's `io` feature; WebSockets explicitly enable `io-compat`. Both native and futures transport APIs remain available. The Linux normal dependency graph is unchanged at 64 minimal / 127 all-feature packages, counting internal packages. The standalone fuzz lockfile drops from 141 to 99 packages, with no additions or version changes. Those 42 removed lockfile entries are not 42 removed runtime dependencies. After forcing release-package rebuilds at identical container paths, the final comparison server (TLS, HTTP/2, dynamic routing) is byte-for-byte identical to baseline: 9,528,056 bytes, SHA-256 `57f5f3395b940fbbf11c28904a5085abda3b8846c8bab01928638abdfaa0472a`. This also matches the frozen baseline used for the experiment. A standalone consumer with default features disabled changes from 1,200,968 to 1,200,984 bytes; GNU `size` reports identical text/data/BSS sizes (807,129 / 26,664 / 2,480 bytes), but the files are not byte-identical. No runtime speed or executable-size improvement is claimed for this cleanup. The consumer source, manifest and exact rebuild script are in the evidence JSON.

## Rejected experiment: allocation and size evidence

The [allocation rows](native-buffers-allocations.csv) retain five alternating pairs and all eight diagnostic cases. Rust 1.96 builds use thin LTO and one codegen unit on Linux ARM64. Both diagnostic client and server are pinned to VM CPU 0. Counters cover the entire process, including runtime and client activity; network cases measure 10,000 requests after 100 warmup requests. Requested bytes measure allocation traffic, not retained memory or RSS.

| Complete request | Allocations, before → after | Requested bytes | Instrumented time, ns |
| --- | ---: | ---: | ---: |
| HTTP/1 keepalive | 15.006 → 15.013 | 2,628.782 → 2,631.791 | 4,450.32 → 4,269.29 |
| HTTP/2 keepalive | 37.008 → 36.010 | 6,935.739 → 5,648.651 | 6,308.22 → 6,466.12 |

The rejected candidate saves approximately one allocation and 18.6% of requested allocation bytes per request in this diagnostic. Its median instrumented time rises 2.5%; the unchanged HTTP/1 and routing controls also vary. These measurements establish allocation savings, not a general speedup. The comparison executable changes from 9,528,056 to 9,528,152 bytes (+96 bytes).

Rejected experiments remain outside the product: retaining frame payload storage saved allocations but could retain up to 16 KiB per idle connection after an upload. Inline frame/preface arrays increased the comparison executable by 82,184 bytes; keeping only the nine-byte frame array still added 75,584 bytes. Neither tradeoff met this pass's size/memory criteria. Native TLS, WebSocket and multipart replacements would also require addressing concrete public types from the existing libraries; no compatible, smaller, measured replacement was established. Compio, Rustls and h3/Quinn remain.

## Rejected experiment: socket load

The [trial rows](native-buffers-trials.csv) retain every completed trial, including failures. Each workload has three alternating pairs, with a separate three-second warmup. HTTP/2 uses TLS and the same certificate, cipher configuration and client validation for both variants. Echo sends 1 KiB. Target-load trials use 1,000 requests/s for 20 seconds; the HTTP/1 control and HTTP/2 64-VU closed-loop trials run for 10 seconds.

Medians across three trials per cell:

| Workload | RPS, before → after | p99 ms | p99.9 ms | CPU µs/response | RSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| HTTP/2 small, 1,000/s | 1000 → 1000 | 1.690 → 1.893 | 7.670 → 4.939 | 41.48 → 45.91 | 8520 → 8564 |
| HTTP/2 echo, 1,000/s | 1000 → 1000 | 2.148 → 2.105 | 5.899 → 13.088 | 61.34 → 63.54 | 8200 → 8296 |
| HTTP/1 control, 1,000/s | 1000 → 1000 | 1.094 → 1.249 | 3.401 → 12.090 | 31.28 → 31.61 | 7304 → 6860 |
| HTTP/2 small, 64 VUs | 59286 → 59869 | 4.859 → 4.866 | 9.777 → 9.962 | 9.77 → 9.66 | 8620 → 8540 |

The 24 trials completed 3,860,376 responses with zero HTTP failures and zero failed response checks. Every started iteration completed. One candidate HTTP/2 echo trial dropped two scheduled iterations and exited 99; its 20,001 completed responses and metrics remain in the dataset. The other 23 trials passed their thresholds. Candidate target-load CPU was higher at the median, and echo p99.9 worsened. The unchanged HTTP/1 control also showed worse tails. Saturation throughput medians differ by about 1%, with overlapping trial ranges. These results are mixed: the allocation improvement is established, but performance parity across latency and CPU metrics is not proven.

The shared Docker Linux ARM64 VM has three CPUs. The server uses CPU 0 and k6 uses CPUs 1–2. Eleven unrelated service containers remained running. No task-initiated compilation or other benchmark ran during these trials. CPU figures divide cgroup CPU usage over the broader measurement interval by completed responses. RSS is a post-run observation, not a continuous peak. Closed-loop client CPU is close to its two-CPU allowance, so these runs do not establish server capacity or framework rankings. The timing and CPU changes do not establish a consistent speed improvement or guarantee absence of regressions in other environments.

## Verification and reproduction

Both the experimental source and final cleanup passed `scripts/check.sh` (152 test executions), `scripts/check-downstream.sh`, Linux ARM64 tests (130 passes; one privileged BPF test skipped), independent HTTP/1 plaintext/TLS, HTTP/2 TLS, HTTP/3, WebSocket text/binary, JSON, multipart, SSE, compression, CORS and HEAD clients, 16 parallel uploads, and 2,062 raw-wire mutations/disconnects with a successful health check afterward. The check script includes formatting, warnings-denied Clippy/docs, minimal/all/isolated feature checks and compilation of every fuzz target. This pass does not claim a fresh parser fuzz campaign, privileged BPF qualification or full RFC conformance.

The final cleanup also replaces a continuation deadline wire test that required the client to resume within a 100 ms scheduling margin. Its deterministic replacement reads SETTINGS, processes HEADERS through connection state, reads a nonempty CONTINUATION with a different default timeout, and checks the original deadline and expired-read rejection. Existing wire tests retain live timeout/GOAWAY coverage. Deliberately resetting the CONTINUATION deadline made the new test fail; restoring the implementation passed. Increasing the old baseline test's client sleep to 600 ms caused its elapsed-time assertion to fail despite the correct server deadline. A 350 ms sleep passed locally. This demonstrates sensitivity to client scheduling, but does not reproduce the exact `BrokenPipe` from the failed macOS PR CI job or establish its scheduling delay.

[Build identities and evidence hashes](native-buffers-evidence.json) identify the measured sources, binaries, compiler/container images, runner and logs. The final cleanup reverts the measured runtime experiment; its binary equivalence check is recorded separately. Raw logs, prototypes and binaries remain ignored under `artifacts/native-buffers/`.

For each source revision, run the unchanged allocation diagnostic with `cargo run --release --locked --manifest-path benches/baselines/Cargo.toml --bin allocations`. For paired Linux placement, build in `rust:1.96-bookworm`, copy each binary aside, and run each with `docker run --rm --cpuset-cpus 0 --security-opt seccomp=unconfined -v "$PWD/artifacts/native-buffers:/results:ro" rust:1.96-bookworm /results/allocations-before` (then `allocations-after`).

The load campaign adapts `benches/run.py` only to resolve ROOT to this checkout, store output under `artifacts/native-buffers/load`, mount the selected frozen binary read-only as `/server`, execute `/server maximus` for both labels, and reuse the TLS certificate across phases. Its hash is recorded. To reproduce with the standard runner, build each source's comparison executable into `maximus-bench-target` and use distinct labels:

```sh
python3 benches/run.py --engines maximus --paths /plaintext,/echo --rates 1000 --vus 64 --duration 20s --repeats 3 --client-cpus 1,2 --protocol h2 --label h2
python3 benches/run.py --engines maximus --paths /plaintext --rates 1000 --vus 64 --duration 10s --repeats 3 --client-cpus 1,2 --label h1-control
python3 benches/run.py --engines maximus --paths /plaintext --rates 0 --vus 64 --duration 10s --repeats 3 --client-cpus 1,2 --protocol h2 --label h2-saturation
```

Reproducing alternating order requires staging both binaries as described above; sequentially running all baseline then all candidate trials has different ordering effects.
