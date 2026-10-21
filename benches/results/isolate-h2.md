# H2 attribution and identical-binary controls

**Decision: attribution and strict socket qualification remain unresolved.**
This separate diagnostic completes 42 observations, 21 matched pairs, and
859,622 checked responses. An A/A group compares the exact same executable
with itself and still varies widely. That observation does not prove the cause
of adverse results in the [primary H1 candidate campaign](h1-empty.md), and
does not erase or qualify those results.

All groups exercise the static Maximus TLS/H2 128 KiB echo with k6, 64 constant
virtual users, a separate three-second warmup, and a five-second measured stage.
Each group has seven rounds. Baseline/candidate order alternates by round,
and group order rotates. There are no new Hyper/Axum observations in this
diagnostic. The primary comparator dataset remains separate.

## Frozen sources and reconciliation

The baseline source is `b5bb664bd6faf9b417cd8af5235ec12e7f0037de`. Its frozen
`core-h2-baseline-source` comparison SHA-256 is
`be05e0aea314cfdda839441c4c3c923e4cae9c8100f4836f382d7ca1e703e8f2`.
Both sides of every A/A pair use that same binary path and hash.

The `error-only-source` changes only `crates/core/src/lib.rs`, preserving
borrowed error payloads. Its comparison SHA-256 is
`945f8c4a7b31ac340d8dfd00ae5b81e946849206722dcaf6ec118da1c65e8f47`.
The `h1-only-source` changes only `crates/http1/src/lib.rs`, introducing optional
completion state and rejected-CONNECT closure. Its SHA-256 is
`20fb46a906b5d16430b17202938bbc07b4ed7b35e5f1d1cbe7763cfecd594472`.
H2 implementation source remains baseline in all three builds. These selective
sources test attribution across linked executables; their changed branches
are not the normal successful H2 large-echo handler work.

The [evidence JSON](isolate-h2-evidence.json) identifies frozen source manifests,
build logs, binary paths/hashes, plan, runner, workload, images, certificate,
environment observations, and raw file hashes. It reconciles all outcomes,
measured/warmup JSON, positive equal request/start/completion counts, body and
protocol checks, zero HTTP/network/check failures, zero drops, and exit zero.
The [trial CSV](isolate-h2-trials.csv) retains every observation, and the
[pair CSV](isolate-h2-pairs.csv) retains every numeric change.

The analyzer's first exact `uname -a` comparison rejected Docker's different
container nodenames. Its correction removes only the second nodename field
after validating the expected Linux structure. It preserves the operating
system, complete release, version/build text, architecture, and `GNU/Linux`
suffix byte-for-byte. Raw `uname -a` values remain recorded. No kernel version,
image identity, load setting, executable identity, or other environment field
is silently ignored by this correction.

## Every matched pair

Values are percentage changes from baseline to the second execution, in round
order. Positive CPU and latency changes are adverse. A/A's second execution is
labelled candidate only to use the same paired schema; it has no source change.

| Group | Round | RPS | CPU/response | p99 | p99.9 |
| --- | ---: | ---: | ---: | ---: | ---: |
| A/A | 0 | −3.16 | +4.28 | +4.05 | +9.40 |
| A/A | 1 | +3.95 | −3.75 | −1.57 | −4.21 |
| A/A | 2 | +9.46 | −9.47 | −7.78 | −20.99 |
| A/A | 3 | +56.31 | −35.56 | −48.02 | +50.13 |
| A/A | 4 | −1.49 | +1.10 | +2.42 | +38.34 |
| A/A | 5 | +31.34 | −18.48 | −22.09 | −44.15 |
| A/A | 6 | +90.16 | −32.93 | −61.81 | −53.45 |
| Error only | 0 | +0.69 | +0.27 | −5.68 | −13.25 |
| Error only | 1 | +0.18 | −1.97 | +3.76 | +59.62 |
| Error only | 2 | +11.09 | −8.26 | −3.36 | −8.69 |
| Error only | 3 | −9.79 | +9.18 | +38.54 | +57.57 |
| Error only | 4 | −33.00 | +38.42 | +47.91 | +35.95 |
| Error only | 5 | +19.25 | −7.96 | −59.53 | −58.39 |
| Error only | 6 | −14.43 | +10.20 | +39.25 | +56.82 |
| H1 only | 0 | −0.63 | −0.27 | +6.85 | −44.80 |
| H1 only | 1 | +1.94 | −0.87 | −1.19 | +25.56 |
| H1 only | 2 | +3.75 | −2.52 | −12.35 | +0.73 |
| H1 only | 3 | +0.65 | +11.21 | −21.12 | −39.53 |
| H1 only | 4 | −1.41 | −0.80 | +4.21 | +6.96 |
| H1 only | 5 | +75.71 | −33.39 | −54.99 | −71.18 |
| H1 only | 6 | +10.61 | −0.73 | −16.64 | −50.04 |

Median paired changes summarize those seven pair percentages, rather than the
ratio of cross-trial medians:

| Group | RPS | CPU/response | p99 | p99.9 |
| --- | ---: | ---: | ---: | ---: |
| A/A | +9.46% | −9.47% | −7.78% | −4.21% |
| Error only | +0.18% | +0.27% | +3.76% | +35.95% |
| H1 only | +1.94% | −0.80% | −12.35% | −39.53% |

A/A RPS spans −3.16% to +90.16%, CPU/response −35.56% to +4.28%, and p99.9
−53.45% to +50.13%. This is observed same-executable variability. It cannot be
subtracted from the other groups as a correction or treated as an additional
candidate improvement. The adverse error-only p99.9 median and favorable
H1-only median do not establish a causal ranking or regression-free source.

## Limits and reproduction

One server worker runs on CPU 0 and clients on CPUs 1–2 of the shared three-CPU
Linux ARM64 VM. Environment snapshots include unrelated Positron containers
as well as four lifeing services. Container inventory does not establish their
continuous CPU use or prove that any particular workload caused a tail spike.
The certificate, Rust/client image IDs, request body, protocol checks, and
runner configuration stay matched.

These five-second closed-loop trials are short and may be client-limited.
They do not establish production capacity or near-capacity tail guarantees.
Server CPU accounting includes startup/teardown observation intervals of
5.448–8.467 seconds. Raw server CPU snapshots were not retained for this
diagnostic; the analyzer reconciles CSV arithmetic, not an independently
reconstructed cgroup delta. RSS is an endpoint observation, not a continuous
peak or retained heap. Build logs and manifests document provenance, while
hashes alone cannot prove which source produced a binary.

Seven rounds and an identical-binary control do not turn unresolved primary
regressions into passes. This diagnostic is never pooled with the primary
five-second or earlier fifteen-second framework comparisons. No strict socket
acceptance, production readiness, or fastest-framework claim is made.

The fixed plan and source identities remain under
`artifacts/performance-2026-10-20/isolate-h2.py` and
`isolate-h2-identities.json`. Reconcile the retained data with
`python3 benches/analyze-isolate-h2.py`. That analyzer performs no workload.
New executions require fresh labels and must preserve the existing trial,
pair, raw environment, and client records.
