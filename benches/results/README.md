# Benchmark results

The [optional I/O cleanup report](native-buffers.md) records the latest feature-scope pass and rejected allocation experiments. The [lean core and routing report](lean-core.md) records the preceding simplification pass. The [native hot-path experiments](native-hot-paths.md) cover the preceding protocol optimizations, with [trial data](native-hot-path-trials.csv), [allocation data](native-hot-path-allocations.csv) and [source/build identities](native-hot-path-evidence.json). The earlier [native HTTP comparison](native-http-comparison.md) records the engine replacement. It includes the previous Maximus engine, Hyper, Axum and Actix, with failed load targets and measurement limitations retained.

- [Primary trials](native-http-trials.csv), [median summaries](native-http-comparison.csv), [phase map](native-http-phases.json) and [scaling chart](native-scaling.svg).
- [Buffer sweep](native-http-buffer-sweep.csv), [allocations](native-http-allocations.csv), [connection churn](native-http-churn.csv) and [sustained run](native-http-soak.csv).
- [Write batching](native-http-write-batching.md), [Date caching](native-http-date-cache.md) and [other optimization experiments](native-opportunities.md).
- [Correctness qualification](../../docs/native-http-qualification.md) and [evidence manifest](native-qualification-evidence.json).

See [benchmark instructions](../README.md) to run a new campaign. Raw logs, binaries and profiler dumps remain local under ignored `artifacts/`; this repository contains compact measured results, not the full raw archive. Older reports and their tools remain in Git history at `9cb096a`.
