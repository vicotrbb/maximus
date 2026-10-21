# Benchmark results

[Approved local Docker/k6 campaign](local-docker-k6.md) records the subsequent
locally authorized comparison of the original reference, corrected baseline,
three isolated candidates, Hyper and Axum. Its generated report states whether
all 159 planned observations are complete. No optimization is accepted from
that exploratory campaign; raw results and adverse observations are retained.

[Dedicated-host preparation](dedicated-qualification-preparation.md) records
three isolated candidate sources and their Linux correctness checks. It supplies
no new performance acceptance.

[Final source verification](performance-research-final.json) records the restored
product state and completed correctness checks. It is not performance acceptance.

**Investigation disposition: archived and performance-unqualified. No
performance optimization from this investigation is retained in root product
code.** The latest [joint wall/thread-CPU H1 control](head-len-cpu-http1.md)
retains every wall/CPU pair and unchanged-control variation. Borrowed Error
conversion, optional H1 completion state and the borrowed H2 timer are restored
to HEAD. The RFC 9931 CONNECT correctness correction and functional regression
coverage remain; the final host, Linux and interoperability checks pass as recorded in
[final verification](performance-research-final.json). Historical
source identities and raw results below are unchanged.

Local Docker/k6 execution is explicitly authorized by the user. Performance
acceptance remains unestablished by the earlier shared VM, memory-transport and
timer cohorts; the subsequent local report records its own measurements and
limits. No cohort establishes that the strict performance goal is achieved.

The [borrowed absolute H2 timer investigation](borrowed-timer.md) records the
archived frozen candidate's completed allocation, correctness, and footprint
checks. Its framework comparison was aborted after severe VM control-plane
stalls; partial raw observations are retained and socket performance remains
unqualified. Earlier owned timer timings do not measure its borrowed future
representation.

The subsequent [same-binary H2 control](post-cache-aa-h2.md) completes eighteen
validated observations, but retains large variation across unchanged engine
repeats. It supplies no candidate performance acceptance or causal cache-cleanup
claim.

The separate [H1 memory-transport control](memory-http1.md) records 168 validated
observations and every one of 84 matched engine/case pairs. Mixed and adverse
timings remain unqualified; it exercises neither H2 nor native TCP I/O.

The [stream-ID traversal model](stream-schedule-model.md) was rejected before
product edits after a repeatable full-budget slowdown. Its standalone model
results are separate from HTTP engine measurements.

The [direct parsed-head H1 experiment](h1-head-direct.md) was rejected for
minimal-artifact code growth. Its smaller conformance text aggregate and
passing native controls remain recorded; no CPU campaign ran for that variant.

The separate [head-length H1 elapsed-time control](head-len-memory-http1.md)
completes 168 observations and 84 pairs, with adverse Maximus medians and large
unchanged-control variation retained. That artifact-only candidate remains
unqualified and is not promoted to product code.

The archived [optional H1 completion-state and borrowed-error investigation](h1-empty.md)
is allocation-validated, with socket performance unqualified under the strict
no-regression requirement. Its primary framework comparison, exact-source
profiles, and separate attribution campaign retain distinct scopes. The
[preceding persistent H2 experiment](core-h2.md) was rejected after adverse
large-response tails. The [initial ownership/registration experiment](ownership-registration.md)
also remains unqualified. These investigations are not accepted throughput
improvements or framework-leadership claims.

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
