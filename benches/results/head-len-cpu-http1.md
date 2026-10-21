# Head-length H1 control with wall and thread CPU clocks

**Disposition: archived artifact-only experiment; no performance acceptance or
product promotion.** All 56 processes pass, supplying 168 joint wall/CPU rows
and 84 matched pairs. Several CPU medians improve, direct owned error worsens,
and adverse pairs plus unchanged-control variation remain. Guest thread CPU
accounting does not establish the user's strict no-regression gate.

This is a separate instrumented cohort, rather than a replacement for the
[adverse elapsed-time cohort](head-len-memory-http1.md). Frozen baseline and
candidate differ only in H1 header scanning: `head_len` returns a buffered
prefix length so the original parsing site can borrow before draining it.
Both snapshots contain earlier unqualified performance experiments. Those
historical identities remain unchanged; root has restored its performance
hunks to HEAD and retains the RFC 9931 CONNECT correction. Checks of that
restored root are pending separately from this report.

## Scope and validation

Seven rounds alternate source order and rotate four engines identically across
variants. Each process runs plain, static error and owned error. Each case has
ten warmup connections and 1,000 timed connections, each containing 1,000
pipelined requests: **168,000,000 timed and 1,680,000 warmup checked responses**
across the complete cohort. Direct Maximus uses a `Kind` handler; direct Hyper
uses its service function. Dynamic Maximus and Axum include routing.

The final formatted fixture checks HTTP/1.1, status/body, Content-Type,
Content-Length, no Transfer-Encoding, exact response count, final-request
closure and no trailing bytes. Every process exits zero. Native candidate H1
controls previously passed all 19 cases, with the fragmentation/pipeline
control passing against baseline. Those historical controls are not final
checks for the restored product.

`Instant` supplies wall time. Safe rustix 1.1.5 calls the Linux executing-thread
CPU clock; Timespec subtraction precedes checked conversion to Duration.
Every stderr records **one-nanosecond nominal clock resolution**. Both printed
timings divide by the same one-million-response denominator and must be finite
and positive. Resolution is not measurement accuracy or physical-cycle
precision. CPU accounting covers the executing guest thread, rather than
whole-process work, and does not prove absent host contention, cache/frequency
effects, or physical-core isolation.

Both intervals include connection/protocol work, allocation, memory-wire writes
and retained output buffers. Input/handler setup is outside timing; final
validation and destruction occur afterward. The wall and CPU clock-read
boundaries differ slightly. Raw clock start/end values and complete wire output
buffers are not archived independently. The always-ready memory transport has
no network backpressure. No H2, native TCP I/O, socket capacity,
latency-distribution, allocation-counter or RSS result follows.

## Every CPU pair

Medians are ns/request over seven rounds. The final column lists **all seven
candidate/baseline percentage changes in round order**; positive means slower.

| Engine | Case | Baseline CPU median | Candidate CPU median | CPU pair changes, rounds 0–6 |
| --- | --- | ---: | ---: | --- |
| Direct Maximus | Plain | 908.400 | 900.305 | −0.89, −1.29, +15.69, +7.51, −22.83, −13.00, −7.01% |
| Direct Maximus | Static error | 909.786 | 892.802 | −21.82, −2.24, +24.05, +12.18, +39.77, −13.00, −6.43% |
| Direct Maximus | Owned error | 874.725 | 949.925 | +8.60, +0.92, +15.58, −0.54, −23.06, −5.02, +2.35% |
| Dynamic Maximus | Plain | 1,080.787 | 1,054.730 | −2.41, +2.54, +21.48, −11.50, −1.44, −7.46, −5.61% |
| Dynamic Maximus | Static error | 1,128.727 | 1,042.853 | −2.51, +0.28, +2.16, −14.83, −24.35, −11.97, −10.93% |
| Dynamic Maximus | Owned error | 1,083.321 | 1,029.856 | −4.94, +0.60, +28.11, +3.84, −41.71, −4.43, −2.49% |
| Hyper control | Plain | 366.454 | 341.243 | −13.27, −0.27, +3.64, +6.77, −17.35, +2.26, −7.85% |
| Hyper control | Static error | 332.808 | 322.014 | −2.88, +0.08, +15.49, −2.11, −32.56, −2.86, −11.47% |
| Hyper control | Owned error | 333.606 | 330.058 | −5.15, −2.49, +12.05, −24.21, −38.11, −1.35, +0.93% |
| Axum control | Plain | 612.955 | 614.759 | +4.65, −0.60, +23.10, +23.85, +0.29, −0.11, −2.05% |
| Axum control | Static error | 606.792 | 602.536 | −1.47, −0.93, +13.97, +8.49, +2.55, −2.26, +0.07% |
| Axum control | Owned error | 616.705 | 606.964 | +0.89, −0.16, +8.78, −6.75, +4.51, −12.54, −6.95% |

Direct owned error has an adverse CPU median and four slower pairs. Other
Maximus cases retain adverse pairs, including static error +39.77% and dynamic
owned error +28.11%. Unchanged controls retain broad CPU variation, including
Hyper owned error −38.11% to +12.05% and Axum plain −2.05% to +23.85%. These
controls are preserved, without normalization or causal attribution.

## Every wall-time pair from this cohort

| Engine | Case | Baseline wall median | Candidate wall median | Wall pair changes, rounds 0–6 |
| --- | --- | ---: | ---: | --- |
| Direct Maximus | Plain | 916.727 | 901.883 | −1.62, −1.26, +15.96, +7.89, −22.79, −16.66, −7.01% |
| Direct Maximus | Static error | 914.777 | 893.109 | −21.82, −2.24, +24.85, +11.75, +39.50, −14.03, −6.44% |
| Direct Maximus | Owned error | 876.181 | 950.394 | +8.47, +0.95, +15.71, −0.22, −23.07, −12.59, +2.30% |
| Dynamic Maximus | Plain | 1,113.327 | 1,063.270 | −2.71, +3.32, +21.53, −11.58, −0.52, −15.44, −6.30% |
| Dynamic Maximus | Static error | 1,177.263 | 1,043.330 | −2.52, −1.59, +2.03, −14.90, −24.47, −14.00, −14.59% |
| Dynamic Maximus | Owned error | 1,085.853 | 1,030.396 | −5.11, +0.63, +28.25, +3.80, −41.70, −3.76, −2.57% |
| Hyper control | Plain | 368.106 | 346.121 | −13.57, −0.80, +2.38, +5.78, −16.72, +0.59, −7.05% |
| Hyper control | Static error | 333.215 | 322.411 | −2.74, +0.06, +15.76, −1.17, −32.28, −2.95, −11.40% |
| Hyper control | Owned error | 338.115 | 330.161 | −6.89, −3.39, +12.12, −24.14, −39.51, −2.65, +1.01% |
| Axum control | Plain | 624.782 | 618.929 | +6.32, −1.74, +23.49, +23.87, +0.50, −7.90, −1.73% |
| Axum control | Static error | 606.923 | 602.612 | −1.47, −1.39, +13.96, +8.51, +2.55, −2.05, +0.08% |
| Axum control | Owned error | 619.063 | 607.038 | +0.88, −0.15, +8.40, −6.96, +3.32, −12.49, −6.95% |

The earlier elapsed cohort remains unchanged and separate. No current-cohort
host snapshot is captured in this evidence; its absence is not proof of an idle
host. The earlier host snapshot is not presented as a capture of this run.
Neither metric establishes a source-level cause for the remaining variation.

## Final inputs, dependency controls and footprint

| Frozen input | SHA-256 |
| --- | --- |
| Baseline CPU probe | `0839324cc3452b6ccaec3e5c82343f0f658821442ddfad8b391dc65eb88b67eb` |
| Candidate CPU probe | `89e391dedf6d02cd3b0a98b2234d3acfc990411e16e99ebc64c7a9d9ebdd4cfb` |
| Final formatted probe source | `63be563b192599c16aa1b6570b7ea075428c5afec20be65c0b48617f4a377fab` |
| Baseline product manifest | `71c2090f3dac94ab0a6c286b23691f2f2b42986e58a45faca45769c5f0379ac9` |
| Candidate product manifest | `ffeb7e51da92cdce6466cb087cdb06057b7f622e01b53d597baf0d991c6ae11a` |

Both final build logs complete release Clippy with warnings denied and release
builds. Metadata records Rust 1.96.1/aarch64 Linux and CPU 0 affinity. All 120
dependency identities match the existing frozen comparator lock, including
rustix 1.1.5 and libc 0.2.190. Only the standalone probe package and direct
rustix `time` feature are added to the probe; product dependencies are unchanged.
The initially generated libc 0.2.191 lock and preformat builds are explicitly
unmeasured and excluded from selected identities and statistics.

The independently recorded `head_len` product footprint remains hello/
conformance GNU text −136/−392 bytes, data +24 bytes each, unstripped files
+200/+184 bytes, unchanged stripped lengths and H1 future sizes 4,328/4,352
bytes. These are archived product-artifact observations, not an instrumented
probe size or proof of size-gate acceptance.

The [offline analyzer](../analyze-head-len-cpu-http1.py) validates all ordered
outcomes, final completion marker, exact final source/build/manifest hashes,
commands, dependencies, positive finite wall/CPU fields, each resolution record,
case/denominator contracts, protocol controls and footprint/layout provenance.
Root observed driver exit zero in session 74022. Maintained
[evidence](head-len-cpu-http1-evidence.json), [trials](head-len-cpu-http1-trials.csv)
and [joint pairs](head-len-cpu-http1-pairs.csv) preserve all values, 138 input
hashes, embedded sources/locks/manifests/drivers/logs, and every adverse result.
No rows are discarded and no earlier cohort is pooled with this one.
