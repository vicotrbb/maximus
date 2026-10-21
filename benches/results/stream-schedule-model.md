# Stream-ID traversal model: rejected

**Decision: REJECTED before product edits.** The live BTreeMap traversal is
slower in every pair of the 256-stream full-budget case, with medians
2,012.651 → 2,284.054 ns/model poll (**+13.48%**). Improvements in the other
five cases do not satisfy the strict no-regression requirement. Actual
`Connection::poll` was not modified or benchmarked by this experiment.

This standalone standard-library model compares an ID snapshot followed by
mutable lookups with repeatedly finding the next ID through `range_mut`. Both
wrap around the cursor and mutate map values. Before timing, assertions compare
returned sums, complete maps, and final cursors for stream counts 0, 1, 3, 16,
and 256, every integer cursor from 0 through `2*count+2`, and budgets 1/all:
1,134 one-step model comparisons. Measured runs also assert the total number
of value increments after warmup and timing. These checks validate the model,
rather than real HTTP stream scheduling, mutation, cancellation, or fairness.

Seven rounds alternate mode order for each of six fixed cases. Each mode warms
up 1,000 polls, then measures
`max(1_000_000 / max(min(streams,budget),1), 2000)` polls. The printed
denominator is **polls**, not visited streams. All 84 rows are preserved,
covering 56,929,684 timed model polls and 84,000 warmup polls. Root observed
exit zero in command session 1033; stderr is empty.

| Streams | Budget | Polls per mode/round | Snapshot median ns/poll | Live median ns/poll | All seven live/snapshot pair changes |
| --- | --- | ---: | ---: | ---: | --- |
| 0 | 1 | 1,000,000 | 4.475 | 1.263 | −72.67, −71.43, −70.67, −71.79, −70.88, −74.81, −71.45% |
| 1 | 1 | 1,000,000 | 12.988 | 3.373 | −75.40, −70.32, −73.26, −73.94, −74.34, −75.00, −74.00% |
| 16 | 1 | 1,000,000 | 60.001 | 7.283 | −89.96, −89.21, −88.12, −87.80, −88.14, −87.66, −87.93% |
| 16 | 16 | 62,500 | 97.254 | 88.891 | −0.63, −8.34, −12.83, −8.16, −9.38, −9.09, −10.22% |
| 256 | 1 | 1,000,000 | 489.177 | 10.805 | −97.72, −97.79, −97.77, −97.82, −97.89, −97.68, −97.70% |
| 256 | 256 | 3,906 | 2,012.651 | 2,284.054 | +12.88, +13.47, +13.97, +14.84, +13.22, +14.88, +13.38% |

Repeated `range_mut` initialization/search plausibly contributes to the
full-budget slowdown. That is a source-based inference: there is no profile
or isolated cost measurement demonstrating its cause. Neither allocation
counters nor RSS were measured. This model establishes no HTTP throughput,
socket latency, retained-memory, or engine allocation result.

The executable hash is
`01769a13e115047d7bd56807d0cc464c38cde587b2951dd010157462a2e0a8b8`;
source hash is
`a60fe474a98a13e197051006c4d989b0cbea195666a9964cabce6c659b7db6fd`.
Recorded compiler metadata identifies Rust 1.96.1, LLVM 22.1.2, and
aarch64-unknown-linux-gnu. The artifact uses only the standard library.
Exact compilation flags, execution image and CPU affinity are not independently
recorded in these files. It adds no product or workspace dependency.

[Evidence](stream-schedule-model-evidence.json) embeds the exact source and
compiler/executable metadata, all raw rows, pair changes, summaries and input
hashes. The [offline analyzer](../analyze-stream-schedule-model.py) checks
fixed source/executable identities, all mode/case/round ordering, denominator
formulas, finite timings and stderr. [Trials](stream-schedule-model-trials.csv)
and [pairs](stream-schedule-model-pairs.csv) retain every observation, including
the seven adverse full-budget pairs. This rejected model is separate from the
HTTP memory-transport, timer, and socket cohorts.
