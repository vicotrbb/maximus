# Borrowed H1 header parsing with head length: elapsed-time control

**Decision: artifact-only candidate UNQUALIFIED under the strict no-regression
requirement.** The completed cohort has adverse elapsed-time medians in all six
Maximus engine/case combinations. Unchanged Hyper/Axum controls also show large
variation. Every observation remains included; no result is normalized by a
comparator or discarded because of host activity. The candidate is not promoted
to root product code.

The independent `head_len` alternative retains parsing at the original outer
connection-loop site. The scanner returns the complete prefix length; parsing
borrows that prefix before draining it, avoiding the collected header `Vec` in
the source path. Only `crates/http1/src/lib.rs` differs between frozen
`h1-head-baseline-source` and `h1-head-len-source`. Both already include the
preceding error/completion-state/borrowed-timer work. This is an incremental
H1-header comparison, separate from the
[rejected direct Head-return variant](h1-head-direct.md).

## Completed cohort and protocol contract

The fixed plan completes **56 processes, 168 rows, and 84 matched engine/case
pairs**, all process exits zero. Seven rounds alternate source order and rotate
the four engines identically across variants. Every process runs plain, static
error, then owned error. Each case warms up ten connections, then times 1,000
connections containing 1,000 pipelined requests each. Recorded totals are
**168,000,000 timed and 1,680,000 warmup checked responses**.

The exact unchanged probe source checks HTTP/1.1, status, body, Content-Type,
Content-Length, absence of Transfer-Encoding, closure only on the final
response, exact response count and no trailing bytes. The candidate also passes
all 19 native H1 controls; the fragmented/pipelined metadata control passed
against baseline. Reused probe Clippy evidence covers the identical probe
source, rather than a claim of fresh candidate-product Clippy.

Direct Maximus uses a `Kind` handler and direct Hyper uses its service function;
dynamic Maximus and Axum include routing. The always-ready shared memory wire
exercises H1 futures transport and protocol work. Allocation, memory-wire writes
and output retention occur within timing; input/handler setup and the
output-pointer vector are prepared beforehand. Final output validation and
destruction occur afterward. These are elapsed ns/request measurements, with a
fixed million-response denominator, rather than thread CPU-clock measurements
or allocation counters. There is no H2, native TCP I/O, socket-capacity,
latency-distribution or RSS result.

## All medians and paired changes

Each median spans seven rounds. The final column contains **all seven
candidate/baseline percentage changes in round order**; positive values mean
slower elapsed time. [All trials](head-len-memory-http1-trials.csv) and
[all 84 pairs](head-len-memory-http1-pairs.csv) retain their exact values.

| Engine | Case | Baseline median ns/request | Candidate median ns/request | Paired changes, rounds 0–6 |
| --- | --- | ---: | ---: | --- |
| Direct Maximus | Plain | 979.073 | 1,049.647 | +82.94, +102.16, +0.63, +44.21, +4.53, −3.17, +0.65% |
| Direct Maximus | Static error | 930.635 | 1,059.794 | +157.81, +151.57, +0.44, +31.05, +13.88, +2.85, +2.88% |
| Direct Maximus | Owned error | 895.020 | 972.199 | +146.97, +145.65, −1.00, +33.52, +10.19, +3.25, −3.79% |
| Dynamic Maximus | Plain | 1,100.562 | 1,169.325 | −4.98, +100.94, −6.04, +19.48, +12.31, −1.20, +9.63% |
| Dynamic Maximus | Static error | 1,082.015 | 1,118.223 | +78.37, +110.48, −6.38, +46.69, −8.23, +8.22, +3.98% |
| Dynamic Maximus | Owned error | 1,022.317 | 1,162.173 | +85.46, +128.74, +14.64, +25.82, −1.95, +4.56, +2.06% |
| Hyper control | Plain | 337.416 | 485.450 | +125.55, +25.56, +1.90, +43.87, +14.47, +31.36, +3.97% |
| Hyper control | Static error | 331.445 | 351.125 | +74.40, +37.82, +0.78, +37.80, −0.87, +4.69, +3.56% |
| Hyper control | Owned error | 336.929 | 363.306 | +106.88, +70.35, −0.24, +7.83, +1.74, −0.86, +9.38% |
| Axum control | Plain | 715.240 | 674.226 | +106.41, −59.32, −2.83, +11.31, −5.73, −2.88, +2.18% |
| Axum control | Static error | 656.796 | 675.227 | +22.94, −28.97, −7.20, +29.27, −2.17, +0.79, −0.28% |
| Axum control | Owned error | 656.337 | 671.330 | +18.84, +72.67, +0.38, +2.28, −6.33, −1.37, −0.71% |

Direct static error and unchanged Hyper plain are slower in every pair. Direct
plain is slower in six, direct owned error in five, and dynamic plain/static/
owned cases in four/five/six pairs. These observations do not satisfy the strict
performance gate. They do not prove which source change or scheduling effect
caused the elapsed differences.

One saved host snapshot records compiler activity outside the owned sessions
and high host load during this cohort. It is context only: one snapshot cannot
attribute any case's timing to a process. No observations are removed, no
comparator normalization is performed, and no causal explanation for previous
socket results follows. A separately instrumented thread-CPU-clock fixture is
planned as a new cohort; it does not replace this elapsed-time evidence.

## Matched footprint and layout

| Measure | Baseline | `head_len` candidate | Change |
| --- | ---: | ---: | ---: |
| Hello GNU text aggregate bytes | 827,585 | 827,449 | −136 |
| Hello data bytes | 27,656 | 27,680 | +24 |
| Hello file bytes | 1,279,584 | 1,279,784 | +200 |
| Hello stripped file bytes | 922,400 | 922,400 | 0 |
| Conformance GNU text aggregate bytes | 2,146,608 | 2,146,216 | −392 |
| Conformance data bytes | 59,344 | 59,368 | +24 |
| Conformance file bytes | 2,953,664 | 2,953,848 | +184 |
| Conformance stripped file bytes | 2,233,184 | 2,233,184 | 0 |

GNU text reductions, data growth and unstripped-file growth remain separate
observations. Unchanged stripped lengths do not establish byte identity or
size-gate acceptance. Actual `.text` section sizes are not recorded for this
alternative. Both H1 compat/native future-storage observations remain
4,328/4,352 bytes. The layout probe constructs the serve futures without
polling them; its storage result is separate from CPU behavior.

## Exact identities and reproducibility

| Frozen input | SHA-256 |
| --- | --- |
| Baseline memory probe | `30ff4f5201cfa0b4007f30efdd9e5178d0189a071b2177cf89ec38e7b7bee02d` |
| `head_len` memory probe | `24bec47d4d50fa82943b178499172b60309a05d37af08f2da8f9763ef5d3318e` |
| Unchanged protocol probe source | `c822c3c9549934a8ed4919b0a1c8ea650c779e2168f7fc7f19c012a923f06e21` |
| Candidate H1 product source | `f34b20be1998d49be70b1630c8310342f7da910d6708598e0fbb02a6f1c37208` |
| Baseline manifest | `71c2090f3dac94ab0a6c286b23691f2f2b42986e58a45faca45769c5f0379ac9` |
| Candidate manifest | `ffeb7e51da92cdce6466cb087cdb06057b7f622e01b53d597baf0d991c6ae11a` |

Captured metadata records Rust 1.96.1 on aarch64 Linux, CPU 0 affinity, and
immutable Rust image ID
`sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663`.
All 120 probe dependencies match the frozen comparator lock by name, version,
registry source and checksum; the only extra package is the probe itself.
Hyper 1.12.0 and Axum 0.8.9 implementations/dependencies are unchanged controls.

The [offline analyzer](../analyze-head-len-memory-http1.py) requires all 56
ordered outcomes, the final driver completion marker and confirmed driver exit
zero before export. It checks exact source/build/manifest/driver identities,
one-file scope, commands, order, case/denominator contracts, finite timings,
stderr, footprint/layout controls and dependency parity. Root observed final
exit zero in session 65144. Its
[evidence JSON](head-len-memory-http1-evidence.json) preserves 140 input hashes
and embeds probe source/locks, product manifests/patch, build/run/size drivers,
raw logs, layout source, host snapshot, every row, every pair and summaries.
Raw start/end timestamps and full HTTP output bytes are not separately
archived. No earlier memory, timer, or socket measurements are pooled here.
