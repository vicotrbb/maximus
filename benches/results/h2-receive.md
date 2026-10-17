# HTTP/2 receive work

This change batches consumed receive credit until the stream or connection window is at most half available, and returns it after polling the consumer. EOF and stream removal return outstanding connection credit. A complete unpadded 16 KiB DATA payload transfers its existing allocation into the body queue when there is no partial tail. Smaller and padded frames retain the existing coalescing path and ordering.

The HTTP/2 runtime grows by **10 lines**, with no new fields, public API, configuration, dependencies or unsafe code. Compio, Rustls and h3/Quinn remain. Receive limits, response scheduling, deadlines and admission policy are unchanged. The added tests protect credit conservation, progress and ownership. The benchmark runner gains configurable echo size and CPU percentages based on the measured sampling interval.

Baseline: `cd66da45b8627a927dfe032b80ab3c957ecceee5`. The selected variant is `credit-copy`; separate credit-only and transfer-only experiments remain in the results. [Trial rows](h2-receive-trials.csv) and [build/evidence identities](h2-receive-evidence.json) retain all measured trials, including failed thresholds. This is a before/after Maximus comparison, not a new framework ranking.

## Work and allocation diagnostics

A local probe processes SETTINGS, HEADERS and eight 16 KiB DATA frames through the real `Connection`, collects the request body and echoes 128 KiB with ample response credit. It validates the output and counts logical output batches, WINDOW_UPDATE frames, incoming payload copies and polls. A batch is **not** a measured syscall or TLS record.

| Variant | Output batches | WINDOW_UPDATE frames | Incoming bytes copied | Polls |
| --- | ---: | ---: | ---: | ---: |
| Baseline | 15 | 15 | 131072 | 30 |
| Credit only | 11 | 7 | 131072 | 23 |
| Transfer only | 15 | 15 | 0 | 30 |
| Both, selected | 11 | 7 | 0 | 23 |

A separate System-allocator probe measures 1,000 such requests after 20 warmups. Three paired Linux executions produced identical integer means: **92 → 70 allocation calls** and **418,430 → 319,816 requested bytes per request** (about 24% less allocation traffic). Peak tracked live bytes were 269,100 → 268,780; outstanding tracked bytes returned to zero in both builds. This includes connection setup, protocol processing, body aggregation and output encoding, but excludes sockets and TLS. It uses temporary allocator instrumentation outside the product. These results do not establish lower process RSS or allocation-free HTTP.

The frozen Linux comparison executable is 9,528,056 → 9,528,072 bytes (+16). Rebuilding the final source produced a byte-for-byte match with the measured selected binary. The credit-only executable was 9,528,080 bytes; transfer-only was 9,594,080 bytes. No executable-size improvement is claimed.

## Socket measurements

Rust 1.96, Linux ARM64, thin LTO and one codegen unit. A single server worker uses VM CPU 0; k6 1.6.0 uses CPUs 1–2. HTTP/2 uses TLS 1.3, the same certificate across variants and verified client trust. Each trial warms the server for three seconds in a separate k6 process; measured client connections start cold. Trial order rotates by repetition. No task-initiated compilation or other benchmark ran concurrently with load.

The first screen used three 20-second, 64-VU large-echo runs per variant. Median RPS was 4,474 baseline, 4,547 credit-only and 4,603 combined; CPU was 208.36, 199.79 and 192.44 µs/response. Wide overlapping ranges prompted a second ablation rather than treating that screen as proof.

The second ablation used three 30-second runs per variant. Baseline, transfer-only and combined median RPS was 4,253 / 4,463 / 4,634; CPU was 202.13 / 199.41 / 182.87 µs/response. RPS ranges were 3,625–4,627 / 3,806–4,955 / 4,069–4,886. The combined candidate is retained for its reduced work and allocation traffic, supported by lower observed large-body CPU cost. The observed throughput improvement is about 9%, not a portable guarantee.

Medians below compare baseline with the selected candidate. All phases use three repetitions except the five-repetition follow-up. Echo sizes are request payloads; plaintext sends no request body.

| Workload | RPS before → after | p99 ms | p99.9 ms | CPU µs/response | Post-run RSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| H2 128 KiB echo, 64 VUs, 30s | 4253 → 4634 | 37.676 → 34.961 | 79.543 → 58.245 | 202.13 → 182.87 | 19972 → 20580 |
| H2 128 KiB echo, 1,000/s, 20s | 998 → 1000 | 11.387 → 9.414 | 36.451 → 27.910 | 287.34 → 244.99 | 18176 → 20248 |
| H2 plaintext, 64 VUs, 20s | 59514 → 60224 | 5.071 → 5.006 | 11.677 → 12.072 | 9.59 → 9.24 | 8564 → 8636 |
| H2 1 KiB echo, 1,000/s, 20s | 1000 → 1000 | 2.392 → 2.073 | 7.305 → 11.818 | 72.76 → 70.42 | 8240 → 7988 |
| H1 plaintext control, 1,000/s, 10s | 1000 → 1000 | 1.750 → 1.829 | 5.344 → 6.428 | 42.21 → 39.06 | 6912 → 6768 |
| H2 1 KiB echo follow-up, 1,000/s, 30s | 1000 → 1000 | 2.167 → 2.461 | 9.041 → 7.984 | 68.19 → 56.64 | 8728 → 8316 |

The first small-echo phase had worse candidate p99.9. The longer follow-up did not reproduce that direction, but candidate p99 rose instead. Tail results are mixed; neither phase erases the other. The unchanged HTTP/1 control also moved, so host variation cannot be separated completely from the patch. The small-response closed-loop client approached its two-CPU budget while the server used substantially less than one CPU; that phase does not establish server capacity.

All **52 measured trials completed 9,914,036 responses**, with zero HTTP failures, zero incorrect-response checks and every started iteration completed. Three baseline trials exited 99 for dropped scheduled arrivals: 137 and 29 large-echo arrivals, and 20 small-echo arrivals. Those rows remain in the CSV. Every selected-candidate trial passed the configured thresholds; these thresholds check errors, correctness and dropped arrivals, not a tail-latency SLA.

The host is a shared three-CPU Docker VM with eleven unrelated service containers; additional transient qualification containers were observed. CPU µs/response includes cgroup work across a broader interval than k6 request timing. CPU percentages use that elapsed sampling interval; client CPU covers its container lifetime inside it. RSS is a process observation, not unique physical memory or a continuous peak. Large-upload post-run RSS increased despite lower requested allocation traffic. Configured storage bounds are unchanged, but **a process-memory improvement is not established**. These runs do not prove universal performance parity, production capacity or superiority over other frameworks.

## Verification and reproduction

The candidate passed `scripts/check.sh` (156 test executions, including minimal/all/isolated features, Clippy, docs and fuzz-target compilation), downstream consumer checks, Linux workspace tests (134 passes; one privileged BPF test skipped), and 237 vendored h3 test executions. The final HTTP/2 test assertions and Clippy check passed after review. Independent clients passed HTTP/1 plaintext/TLS, HTTP/2 TLS, HTTP/3, WebSocket, extraction, JSON, multipart, SSE, compression, CORS, HEAD and concurrent uploads. Raw-wire stress completed 2,062 mutations/disconnects plus a healthy post-stress request.

Both plaintext and TLS h2spec runs passed the existing compatibility gate: 139/147 raw cases passed, five documented RFC 7540/9113 policy differences remained, and three response-closure races passed when rerun with the response held open. This is not 147/147 legacy conformance. A fresh HTTP/2 libFuzzer/AddressSanitizer smoke run completed **2,438,301 executions in 61 seconds** with no finding, seeded with full, padded and split DATA frames (`-max_total_time=60 -max_len=65536`). It is not exhaustive fuzzing or privileged BPF qualification.

The retained benchmark runner also passed a 501-request, 128 KiB TLS/H2 smoke run with periodic memory sampling; negative body sizes fail argument validation. This tooling check is separate from the 52 comparative trials.

Focused tests cover one-poll credit return, a one-byte stream window, EOF, distributed connection pressure, exactly-once reset credit, early responses with unread data, moved allocation ownership, partial-tail ordering and maximum-sized padded frames. Overlaying the tests on baseline failed the credit-timing, connection-pressure and ownership assertions. Review removed an incidental dependency on stream visitation order.

For each source revision, build the locked benchmark workspace using the same compiler and release profile. Freeze its comparison executable before load; the recorded build force-cleans the affected release packages at the same `/work` container path. The existing runner accepts the measured workloads, for example:

```sh
python3 benches/run.py --engines maximus --paths /echo --body-bytes 131072 --protocol h2 --rates 0 --vus 64 --duration 30s --repeats 3 --client-cpus 1,2 --label large
python3 benches/run.py --engines maximus --paths /echo --body-bytes 1024 --protocol h2 --rates 1000 --vus 64 --duration 30s --repeats 5 --client-cpus 1,2 --label small
```

The local campaign adapter mounts each frozen binary read-only as `/server` and invokes `/server maximus` for every variant label, rotates those labels using the runner's existing ordering, and reuses its TLS certificate across phases. Sequentially running one entire revision and then the other has different ordering effects. Exact phase arguments, image digests, source/binary hashes and log hashes are recorded in the evidence manifest. The CSV normalizes `body_bytes` to zero for plaintext GETs; raw adapter rows recorded the unused echo-size default there. Raw probes, instrumentation, binaries, corpora and logs remain ignored under `artifacts/h2-batching/`; this compact report does not publish that full local archive.
