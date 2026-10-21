# Local Docker and k6 results

**No performance optimization is accepted from these exploratory measurements.**

159 attempts; 159 recorded observations; 0 failed attempts. 68,548,839 measured requests and 15,001,485 warmup requests. Campaign complete: True; raw correctness audit failures: 0.

Docker on this machine: Linux ARM64, 3 VM CPUs, approximately 4 GB RAM. Frozen Rust 1.96.1 builds; Hyper 1.12.0 and Axum 0.8.9 from the benchmark lockfile. One server worker on CPU 0; k6 on CPUs 1 and 2; 64 virtual users, 3 s warmup and 12 s measured runs. Three planned main repetitions; two initial same-executable controls. The host is shared. All five independently built variants run every workload. Raw observations include failures, client CPU, wall-clock CPU intervals and sampled RSS. CPU accounting spans Docker client startup/teardown and sampling, not exactly the k6 duration. RSS is process memory; the sampled peak is not an exact high-water mark. Small-request trials show much higher client CPU utilization than server utilization. Their closed-loop request rates therefore do not establish maximum server capacity or a framework throughput ranking. CPU/request is a local measured cost under the recorded load.

The original reference uses b5bb664 production source with the current shared harness. It retains the known rejected-CONNECT defect and is a valid-workload performance reference only. Separate exact-source correctness checks are documented in [candidate preparation](dedicated-qualification-preparation.md) and [root verification](performance-research-final.json). Hyper and Axum are comparison fixtures, not claims of complete feature parity.

## Corrected baseline versus Hyper and Axum

| Workload | Engine | n/3 | Median requests/s | Median p99 ms | Median CPU µs/request | Median RSS KiB |
|---|---|---:|---:|---:|---:|---:|
| plain /plaintext (0 B) | maximus | 3/3 | 46,514 | 7.654 | 8.54 | 7,304 |
| plain /plaintext (0 B) | hyper | 3/3 | 46,985 | 7.854 | 6.28 | 4,836 |
| plain /plaintext (0 B) | axum | 3/3 | 45,891 | 7.726 | 7.10 | 6,772 |
| plain /error (0 B) | dynamic | 3/3 | 44,115 | 8.215 | 9.50 | 7,232 |
| plain /error (0 B) | hyper | 3/3 | 42,930 | 8.760 | 6.51 | 4,624 |
| plain /error (0 B) | axum | 3/3 | 47,589 | 7.334 | 6.78 | 6,748 |
| plain /users/42 (0 B) | dynamic | 3/3 | 42,859 | 9.140 | 9.45 | 6,512 |
| plain /users/42 (0 B) | hyper | 3/3 | 45,574 | 7.843 | 6.36 | 4,832 |
| plain /users/42 (0 B) | axum | 3/3 | 43,624 | 8.299 | 7.50 | 6,772 |
| plain /echo (1024 B) | dynamic | 3/3 | 38,514 | 9.305 | 10.73 | 6,796 |
| plain /echo (1024 B) | hyper | 3/3 | 39,934 | 9.259 | 7.50 | 5,952 |
| plain /echo (1024 B) | axum | 3/3 | 39,644 | 9.179 | 8.15 | 6,784 |
| h1 /plaintext (0 B) | maximus | 3/3 | 44,634 | 8.004 | 9.85 | 9,296 |
| h1 /plaintext (0 B) | hyper | 3/3 | 42,642 | 8.009 | 7.53 | 7,384 |
| h1 /plaintext (0 B) | axum | 3/3 | 34,731 | 11.873 | 9.47 | 6,252 |
| h2 /error (0 B) | maximus | 3/3 | 33,619 | 10.406 | 13.65 | 8,512 |
| h2 /error (0 B) | hyper | 3/3 | 37,328 | 8.902 | 11.42 | 8,788 |
| h2 /error (0 B) | axum | 3/3 | 38,149 | 9.474 | 12.23 | 8,888 |
| h2 /echo (131072 B) | maximus | 3/3 | 3,231 | 60.741 | 247.97 | 19,184 |
| h2 /echo (131072 B) | hyper | 3/3 | 5,004 | 40.817 | 149.84 | 18,124 |
| h2 /echo (131072 B) | axum | 3/3 | 4,937 | 37.279 | 157.69 | 18,872 |

## Candidate paired deltas against the corrected baseline

Percent changes are candidate/reference − 1 for each repetition, then the median. Positive throughput is faster; negative CPU, latency and RSS is lower. These deltas are observations, not accepted gains. The JSON retains every sample, range and comparison against the original reference.

| Candidate | Workload | n/3 | Requests/s Δ | CPU/request Δ | p99 Δ | p99.9 Δ | RSS Δ |
|---|---|---:|---:|---:|---:|---:|---:|
| borrowed-errors | plain /plaintext | 3/3 | +1.24% | -2.63% | -4.56% | -13.17% | -10.76% |
| borrowed-errors | plain /error | 3/3 | +4.24% | -5.30% | -6.31% | -7.51% | +0.55% |
| borrowed-errors | plain /users/42 | 3/3 | +9.36% | -5.40% | -17.44% | -29.67% | +8.13% |
| borrowed-errors | plain /echo | 3/3 | +12.18% | -8.22% | -15.79% | -20.20% | +12.65% |
| borrowed-errors | h1 /plaintext | 3/3 | -6.38% | +8.12% | +5.30% | +3.11% | +13.25% |
| borrowed-errors | h2 /error | 3/3 | +1.12% | -0.72% | -7.01% | -6.91% | +1.92% |
| borrowed-errors | h2 /echo | 3/3 | +0.82% | -0.03% | +3.07% | +7.35% | -2.47% |
| h1-empty-tracking | plain /plaintext | 3/3 | +1.17% | -3.92% | -3.16% | +1.15% | -6.41% |
| h1-empty-tracking | plain /error | 3/3 | +5.95% | -5.43% | -8.78% | -11.33% | -1.26% |
| h1-empty-tracking | plain /users/42 | 3/3 | +7.53% | -6.77% | -16.19% | -21.61% | -6.09% |
| h1-empty-tracking | plain /echo | 3/3 | +3.62% | -0.23% | -10.34% | -12.30% | -3.18% |
| h1-empty-tracking | h1 /plaintext | 3/3 | -7.44% | +9.71% | +14.46% | +9.21% | +0.78% |
| h1-empty-tracking | h2 /error | 3/3 | -7.44% | +3.63% | +12.60% | +13.61% | -7.57% |
| h1-empty-tracking | h2 /echo | 3/3 | +6.59% | -3.58% | -0.19% | -38.08% | +4.87% |
| h2-borrowed-timer | plain /plaintext | 3/3 | +1.12% | +1.12% | -1.11% | -7.08% | -0.05% |
| h2-borrowed-timer | plain /error | 3/3 | -3.80% | -2.55% | +6.84% | +13.08% | -5.53% |
| h2-borrowed-timer | plain /users/42 | 3/3 | +5.11% | -6.26% | -5.52% | +8.08% | +8.72% |
| h2-borrowed-timer | plain /echo | 3/3 | +1.24% | -2.55% | +0.43% | -10.26% | +9.65% |
| h2-borrowed-timer | h1 /plaintext | 3/3 | +2.52% | +2.07% | -6.04% | -6.86% | +10.98% |
| h2-borrowed-timer | h2 /error | 3/3 | -21.12% | +8.30% | +37.43% | +66.51% | -1.13% |
| h2-borrowed-timer | h2 /echo | 3/3 | +10.46% | -6.14% | -27.44% | -50.90% | +4.44% |

## Same-executable control variation

| Case | Engine | Requests/s Δ | CPU/request Δ | p99 Δ | p99.9 Δ |
|---|---|---:|---:|---:|---:|
| 0 | maximus | +0.27% | +2.31% | +0.01% | -10.85% |
| 0 | hyper | -0.85% | +0.11% | +4.70% | +13.45% |
| 0 | axum | +5.15% | -4.04% | -9.92% | -9.30% |
| 1 | maximus | -3.98% | +2.87% | +7.28% | -1.90% |
| 1 | hyper | -0.86% | -1.31% | +0.17% | +4.65% |
| 1 | axum | +6.50% | -2.58% | -11.98% | -18.17% |

## Linked footprint and allocation diagnostics

These measurements ran before k6, in separate processes. Text/data/BSS are GNU `size` aggregates; file bytes and stripped bytes are retained in JSON. Every candidate increases text in at least one linked artifact versus the corrected baseline. All three stripped artifact file sizes remain unchanged across variants; ELF layout granularity means a smaller text aggregate need not shrink the executable file.

| Source | Minimal hello text B | Conformance text B | Comparison text B |
|---|---:|---:|---:|
| original | 827,893 | 2,151,396 | 7,136,205 |
| baseline | 827,905 | 2,147,364 | 7,136,205 |
| borrowed-errors | 831,565 | 2,147,084 | 7,122,977 |
| h1-empty-tracking | 827,957 | 2,147,364 | 7,140,433 |
| h2-borrowed-timer | 827,905 | 2,150,920 | 7,135,789 |

| Source | Static error allocations | Static error bytes | Owned error allocations | H1 keepalive allocations/request |
|---|---:|---:|---:|---:|
| original | 3.0000 | 665.0000 | 3.0000 | 15.1708 |
| baseline | 3.0000 | 665.0000 | 3.0000 | 15.0242 |
| borrowed-errors | 2.0000 | 656.0000 | 3.0000 | 15.0106 |
| h1-empty-tracking | 3.0000 | 665.0000 | 3.0000 | 14.0136 |
| h2-borrowed-timer | 3.0000 | 665.0000 | 3.0000 | 15.0163 |

The borrowed-error candidate removes one allocation and 9 allocated bytes for the static error fixture; owned errors remain at three allocations. The empty-H1 candidate removes approximately one allocation in the full keepalive diagnostic. Fractional network counts include process-wide runtime activity; allocation reduction alone is not throughput acceptance. Diagnostic elapsed times are retained as raw data and are not treated as a timing campaign.

## Disposition and final-run anomaly

All 159 expected unique observations are present, with three samples in every main cell. The raw correctness audit records 0 failures. No performance candidate is promoted: concrete allocation reductions coexist with linked-text growth and adverse or inconsistent local timing/resource observations.

Examples of paired median observations versus corrected baseline are: borrowed-errors on h1 /plaintext: requests/s -6.38%, CPU/request +8.12%, p99.9 +3.11%; h1-empty-tracking on h1 /plaintext: requests/s -7.44%, CPU/request +9.71%, p99.9 +9.21%; h2-borrowed-timer on h2 /error: requests/s -21.12%, CPU/request +8.30%, p99.9 +66.51%. These are observed differences, not established causal effects of the patches. The full tables and JSON retain favorable observations as well.

The final unchanged corrected-baseline H2 upload trial recorded 411.34 requests/s, p99 3442.12 ms, p99.9 3845.36 ms and a 23.34 s CPU-accounting interval. Every request still passed the response and iteration checks. This severe outlier is retained in all aggregates and pairs; it inflates some apparent candidate upload gains. The observations do not establish its cause. Together with the same-executable variation and high client CPU, it prevents strong capacity or universal performance conclusions.


The campaign uses partially balanced source order and three samples per main cell. It cannot establish universal absence of regressions. Repeatable adverse observations, unstable controls or contradictory workload outcomes prevent acceptance. A candidate needs a fresh, balanced confirmation campaign before promotion. No adverse result is discarded or normalized using competitor variation.

Review the committed report, JSON and trial CSV without the original execution environment. Rerunning the full historical audit with `python3 benches/analyze-local-k6.py` requires the locally retained artifact collection, which is ignored by Git and has no published restoration location. A fresh checkout contains the compact evidence, not those audit inputs. The predeclared driver, source hashes, build logs and frozen executable hashes are under `artifacts/performance-2026-10-20/local-k6-approved/`; raw k6 summaries are under `artifacts/bench/local-k6-*`. TLS private keys are excluded from reports.
