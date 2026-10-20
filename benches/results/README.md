# Benchmark results

The [HTTP/1 terminator buffer report](lean-ownership.md) records the retained allocation reduction, focused controls and reduced-build socket qualification; the earlier four-change investigation remains linked in history. The [static routing and trie-capacity report](lean-routing.md) records the current retained pair and rejected experiments. The [HPACK lookup report](lean-io.md) records the retained single-pass lookup and its verification; the earlier direct-encoding candidate was removed. The [HTTP/2 receive-work report](h2-receive.md) records the preceding credit and payload-ownership changes, their ablations, controls and measurement limits. The [optional I/O cleanup report](native-buffers.md) records the preceding feature-scope pass and rejected allocation experiments. The [lean core and routing report](lean-core.md) records the preceding simplification pass. The [native hot-path experiments](native-hot-paths.md) cover the preceding protocol optimizations, with [trial data](native-hot-path-trials.csv), [allocation data](native-hot-path-allocations.csv) and [source/build identities](native-hot-path-evidence.json). The earlier [native HTTP comparison](native-http-comparison.md) records the engine replacement. It includes the previous Maximus engine, Hyper, Axum and Actix, with failed load targets and measurement limitations retained.

- [Primary trials](native-http-trials.csv), [median summaries](native-http-comparison.csv), [phase map](native-http-phases.json) and [scaling chart](native-scaling.svg).
- [Buffer sweep](native-http-buffer-sweep.csv), [allocations](native-http-allocations.csv), [connection churn](native-http-churn.csv) and [sustained run](native-http-soak.csv).
- [Write batching](native-http-write-batching.md), [Date caching](native-http-date-cache.md) and [other optimization experiments](native-opportunities.md).
- [Correctness qualification](../../docs/native-http-qualification.md) and [evidence manifest](native-qualification-evidence.json).

See [benchmark instructions](../README.md) to run a new campaign. Raw logs, binaries and profiler dumps remain local under ignored `artifacts/`; this repository contains compact measured results, not the full raw archive. Older reports and their tools remain in Git history at `9cb096a`.

The [owned HTTP/2 DATA and path-validation investigation](owned-data-and-paths.md)
records the rejected preliminary two-change candidate, matched allocation counts, focused
path timings, 84 socket trials, rejected HTTP/1 and HPACK experiments, and the
remaining performance qualification limits.

The [static HPACK and response-ownership investigation](static-hpack-and-response-ownership.md)
records the rejected four-change candidate, five matched allocation pairs,
release footprints, focused decode probes, correctness qualification, and a fresh
private-loopback comparison with Hyper and Axum. It keeps the preliminary
candidate and rejected experiments separate.

The [HTTP/2 static fields and DATA ownership report](h2-static-ownership.md)
records the retained three HTTP/2 changes after restoring the original path
validator, its ablation, fresh correctness checks, allocation measurements and
frozen-binary comparison and fixed-count memory diagnosis. It states the
remaining throughput and memory uncertainties.
