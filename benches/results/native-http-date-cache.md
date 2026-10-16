# Native HTTP Date cache experiment

The candidate adds a private per-second Date HeaderValue cache to the frozen write-batching implementation. It preserves explicit Date headers and refreshes when the clock moves backwards. Only HTTP/1 product `lib.rs` changes. Its SHA-256 is `3f996b23845cb7111d144c4afb9facd7c5adc4d0fae23767719d63b22e6c4677`. The matching binary and allocation-program identities are in `artifacts/native-http/qualification/date-build.log`.

Twelve wrk trials at one worker completed 10,786,504 measured responses with zero failed checks, network errors, or cgroup throttling. Each trial uses 128 connections, a new three-second warmup client and a new eight-second measured client. Engine order rotates across three repeats.

| Path | Batching RPS | Date cache RPS | Change | Batching p99 ms | Date cache p99 ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| /plaintext | 115,524 | 118,211 | +2.33% | 1.715 | 1.623 |
| /echo | 104,712 | 109,150 | +4.24% | 2.047 | 1.795 |

Plaintext RPS ranges overlap, 114,188 to 117,125 versus 116,770 to 118,936. Echo ranges separate, 104,117 to 105,550 versus 108,735 to 109,288. These shared-host results support carrying the cache into final qualification and do not establish gains for every deployment or worker count. This experiment does not retest the four-worker tail behavior from the separate batching matrix.

Complete HTTP allocation medians across three alternating hardware runs fell from 17.9982 allocations and 3,139.1792 requested bytes per request to 15.9983 and 3,081.2248. Static route/body dispatch remained zero; static text response conversion remained two allocations and 656 bytes. Counters include the complete client/server scope under CPU mask `0,8`, not retained heap.

Raw output, exact retained harness, CPU snapshots, acknowledged PMU intervals, source identities, and allocation logs remain under `artifacts/native-http/qualification/`. The completed-campaign resume check retained exactly twelve rows and launched no duplicate trials. Compact median data are in `native-http-date-cache.csv`.

The measured batching control used HTTP/1 `lib.rs` SHA-256 `a3f3a4cff393902d8729d06f58f72c93563361b0d8515f74e224cee4d2298966`. The separately reviewed snapshot has SHA-256 `2a52f9b52a47ad7d6f34017dfec800d14134179eb92f3ecf48670723a1dc662d`: removing exactly one explanatory comment and its `allow(clippy::too_many_arguments)` annotation reproduces the measured hash. These two control snapshots have identical runtime code. The reconstructed measured source and both exact diffs are retained in the qualification artifacts.

CPU and PMU accounting covers the acknowledged control interval around client launch, execution, result observation, and drain. This interval is wider than the client-reported request interval. Cgroup CPU and attached-thread perf task-clock also disagree in the final campaign; the cause is unresolved. Counter totals divided by checked responses are observations under that measurement scope, not exact client-only CPU time or pure application instruction counts.
