# Native HTTP write batching experiment

Both variants use the same native product snapshot; the candidate changes only HTTP/1 `lib.rs` and `io.rs`. It bounds and reuses the response buffer, coalesces fixed headers/body, and flushes streaming headers before polling the body. The matching source/binary SHA-256 values are in `artifacts/native-http/qualification/candidate-build.log`. The per-second Date cache is a separate experiment. Both strategy variants use the configured 16 KiB I/O buffer, a 13-byte plaintext response, and a 1 KiB echo payload. This strategy comparison uses plaintext HTTP/1; the separate size sweep on the final writer covers 4/16/64 KiB buffers, plaintext and TLS H1, and 1/128 KiB echo bodies. It does not isolate the batching strategy under TLS or measure streaming throughput.

Thirty-six validated wrk trials, each with three-second warmup and eight-second measurement at 128 connections, completed 50,446,670 measured responses with zero failed checks, network errors or cgroup throttling. The same shared Ryzen hardware, four-CPU container quotas, compiler and disjoint server/client core sets are described in the opportunity report. Entries below are three-trial medians. Engine order rotates by repeat.

| Path | Workers | Before RPS | Batching RPS | Change | Before / batching p99 ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| /plaintext | 1 | 72,141 | 115,529 | +60.14% | 2.925 / 1.658 |
| /plaintext | 2 | 134,999 | 209,706 | +55.34% | 1.386 / 1.020 |
| /plaintext | 4 | 239,719 | 353,251 | +47.36% | 0.975 / 3.131 |
| /echo | 1 | 56,911 | 104,292 | +83.25% | 3.121 / 1.936 |
| /echo | 2 | 105,435 | 195,357 | +85.29% | 1.934 / 1.183 |
| /echo | 4 | 184,212 | 333,834 | +81.22% | 1.327 / 2.265 |

Four-worker p99 medians worsen, and individual tails vary substantially. These saturated closed-loop runs complete different request rates; they do not compare latency under the same offered load. Final fixed-arrival measurements must retain this distinction. The throughput gain supports carrying batching into qualification, while the tail results prevent a blanket latency-improvement claim.

Complete HTTP allocation medians across three alternating runs fell from 25.9987 allocations / 4,008.4072 requested bytes per request to 17.9973 / 3,138.7688. Static route/body remains zero allocations and static text conversion remains two allocations / 656 bytes. The two-thread complete-path scope uses process mask `0,8` and includes client allocation traffic, so it differs from prior ARM measurements. Native allocation program wall times include roughly five seconds of shutdown drain after request measurement and are not used as throughput results.

Raw validated JSON, acknowledged task-attached PMU results, both-container CPU snapshots, exact frozen harness, binary identities, and allocation output remain under `artifacts/native-http/qualification/`. The compact median data are in `native-http-write-batching.csv`. Product policy remains enabled. No result establishes universal performance leadership.

## Four-worker fixed-arrival control

A separate twelve-trial k6 comparison offers 5,000 requests per second with 512 preallocated VUs. It completes 599,991 checked responses with zero failed checks, HTTP failures, dropped iterations, or cgroup throttling. Each entry is the median of three ten-second trials. These k6 request-duration percentiles have different semantics from the wrk corrected histogram and must not be pooled.

| Path | Before p50 ms | Batching p50 ms | Before p99 ms | Batching p99 ms | Before p99.9 ms | Batching p99.9 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| /plaintext | 0.0869 | 0.0769 | 0.924 | 1.097 | 2.857 | 6.570 |
| /echo | 0.0910 | 0.0818 | 1.122 | 1.326 | 3.865 | 12.197 |

Median latency improves while both tail percentiles worsen. The allocation and throughput gains therefore carry a measured tail-latency tradeoff, including at this low offered load. Shared workloads, power management, interrupt sharing, and scheduler variation remain confounders; no specific harness or placement defect was found. The 5,000-per-second workload is far below saturation and cannot resolve the near-capacity p99 regression. The installed Debian wrk 4.1.0 has no rate-control option; this campaign adds no new load tool.

The compact fixed-arrival data are in `native-http-batch-arrival.csv`, with raw trials under the distinct `native-batch-arrival-20261016-01` label. No selective rerun replaces these results.

CPU and PMU accounting covers the acknowledged control interval around client launch, execution, result observation, and drain. This interval is wider than the client-reported request interval. Cgroup CPU and attached-thread perf task-clock also disagree in the final campaign; the cause is unresolved. Counter totals divided by checked responses are observations under that measurement scope, not exact client-only CPU time or pure application instruction counts.
