# Borrowed errors and persistent H2 shutdown experiment

**Decision: reject H2 persistent shutdown from the frozen tuple candidate under
the no-regression requirement.** It reduces measured static-error and H2 allocation work and linked
code size, but the primary socket campaign retains adverse observations. The
128 KiB H2 echo worsens at p99.9 in all three pairs. H1 captured-route CPU and
echo tails also remain unresolved. The H2 implementation has been restored
exactly to baseline. The frozen tuple source and observations remain unchanged.
This is a conservative rejection based on the recorded adverse results, not
proof of a particular source-level cause. This report does not describe
accepted final product code.

The baseline is `b5bb664bd6faf9b417cd8af5235ec12e7f0037de`. The candidate is the
frozen `core-h2-tuple-source` snapshot, identified by
`core-h2-tuple-manifest.json`. Its comparison binary SHA-256 is
`c0205e565b99b30488c07de092d1df4b601a89ecac588f49b7f251889e567f18`.
The baseline binary reproduces
`be05e0aea314cfdda839441c4c3c923e4cae9c8100f4836f382d7ca1e703e8f2`.

Three product changes enter this candidate:

- `Error::into_response` matches borrowed/owned messages and delegates each to
  the existing `(status, message).into_response()` implementation.
- H2 keeps one fused shutdown waiter across connection-loop selects. The
  worker-loop implementation remains original.
- HTTP/1 closes after rejected CONNECT, as required for proxy-server behavior
  by [RFC 9931 §8](https://www.rfc-editor.org/rfc/rfc9931.html#section-8).

The router implementation is unchanged from baseline. Earlier routing and
registration experiments were removed after adverse observations. Their
[initial report](ownership-registration.md) and the
[research disposition](../../docs/performance-research-2026-10-20.md) remain
separate. No new dependency, unsafe code, public API, or configuration enters
this product candidate.

## Focused measurements

Seven alternating component pairs run the same frozen diagnostic on CPU 0,
without allocation counters. Values are medians in nanoseconds per operation.
Dispatch/conversion cases use 2,000,000 operations per run; registration uses
10,000 complete 64-route tables, including destruction.

| Component | Baseline | Tuple candidate |
| --- | ---: | ---: |
| Static text control | 35.212 | 33.153 |
| Static error | 38.194 | 32.061 |
| Owned error control | 35.573 | 35.705 |
| Literal dynamic route control | 105.270 | 108.638 |
| Captured dynamic route control | 154.272 | 154.025 |
| Dynamic 405 control | 114.692 | 112.934 |
| Register mixed 64-route table | 22,964.632 | 22,745.415 |
| Register literal 64-route table | 16,265.963 | 16,300.755 |

All seven static-error pairs are faster. Literal control pair directions are
mixed. Unchanged controls moving in either direction do not establish a causal
effect of the candidate on their implementation. The
[component CSV](core-h2-components.csv) retains all 112 observations.

Five separate allocation pairs emit eighteen cases each. All 24 component and
allocation processes exit zero. Counts include allocation and reallocation
calls. Requested bytes measure allocation traffic, not retained memory.

| Allocation case | Calls/request, baseline → candidate | Requested bytes/request, baseline → candidate |
| --- | ---: | ---: |
| Static GET, HEAD, POST, PUT | 0 → 0 | 0 → 0 |
| Static text with Content-Type | 2 → 2 | 656 → 656 |
| Static error | 3 → 2 | 665 → 656 |
| Owned error | 3 → 3 | 665 → 665 |
| Register mixed 64-route table | 389 → 389 | 20,280 → 20,280 |
| Register literal 64-route table | 389 → 389 | 18,552 → 18,552 |
| Dynamic GET and POST | 2 → 2 | 609 → 609 |
| Dynamic 405 | 5 → 5 | 698 → 698 |
| Complete H1 keepalive | 16.0292 → 16.0891 | 1,940.8244 → 2,200.6988 |
| H2 invalid first frame | 40.7230 → 40.6440 | 29,240.2080 → 28,754.9680 |
| H2 large static response | 38.9624 → 35.9149 | 23,557.8544 → 23,368.1944 |
| H2 large owned response | 39.9836 → 36.9909 | 56,335.5216 → 56,170.7218 |
| Complete H2 static keepalive | 32.0160 → 30.0165 | 5,731.9088 → 5,621.6872 |
| Complete H2 owned keepalive | 33.0168 → 31.0170 | 5,746.7444 → 5,636.4656 |

The direct static-error saving is one payload copy. Complete-loopback counters
include the client and runtime. H2 observations do not establish a fixed number
of listener allocations saved per frame. The adverse H1 allocation and byte
observations remain in the [allocation CSV](core-h2-allocations.csv); no H1
allocation saving is attributed to this candidate. Timings in that instrumented
CSV do not support component-speed claims.

## Primary socket campaign

The [trial CSV](core-h2-trials.csv) has 84 observations and 273,547,347 checked
responses. Sixty wrk observations account for 260,920,186 responses. Twenty-four
k6 observations account for 12,627,161. Each row has positive response counts,
zero failed response checks, zero network failures, zero drops, and exit zero.
k6 starts, requests, and completions match. wrk's checked-response count matches
its request count; no k6 iteration count is invented for wrk.

The [evidence JSON](core-h2-evidence.json) reconciles every measured and warmup
JSON with the CSV and outcome records. It checks the expected binary identities,
stable workload settings and container images, shared TLS certificate identity,
identical diagnostic sources, all fixture exits, and raw artifact hashes. Its
embedded script preserves all observations and 21 matched-pair changes.

H1 uses wrk with two client threads and 64 connections. RPS values are medians
of three fifteen-second trials per cell:

| H1 workload | Baseline | Tuple candidate | Hyper 1.12.0 | Axum 0.8.9 |
| --- | ---: | ---: | ---: | ---: |
| Dynamic plaintext | 270,112 | 306,794 | 340,749 | 365,095 |
| Dynamic captured parameter | 294,656 | 283,113 | 300,639 | 171,707 |
| Dynamic application error | 245,688 | 316,647 | 348,765 | 307,102 |
| Dynamic 1 KiB echo | 267,276 | 274,036 | 290,914 | 267,606 |
| Static plaintext control | 325,086 | 332,727 | 344,856 | 338,157 |

TLS/H2 uses k6 with 64 constant virtual users. These results form a separate
generator cohort and are not pooled with wrk:

| TLS/H2 workload | Baseline | Tuple candidate | Hyper 1.12.0 | Axum 0.8.9 |
| --- | ---: | ---: | ---: | ---: |
| Application error | 66,461 | 66,843 | 66,467 | 64,334 |
| 128 KiB echo | 5,713 | 5,633 | 7,997 | 7,578 |

Maximus remains behind the competitor fixtures on large H2 echo. Neither table
establishes framework leadership. The Hyper TLS fixture retains its shared
router-clone adapter, and framework policy costs differ.

Candidate latency and CPU medians, baseline → candidate:

| Workload | Server CPU µs/response | p99 ms | p99.9 ms |
| --- | ---: | ---: | ---: |
| H1 dynamic plaintext | 3.648 → 3.235 | 1.339 → 0.779 | 16.382 → 13.335 |
| H1 captured parameter | 3.268 → 3.503 | 1.195 → 0.793 | 8.855 → 3.722 |
| H1 application error | 4.024 → 3.131 | 0.880 → 0.607 | 6.999 → 3.158 |
| H1 1 KiB echo | 3.713 → 3.618 | 0.788 → 0.733 | 4.407 → 6.962 |
| H1 static plaintext | 3.055 → 2.988 | 0.551 → 0.484 | 8.993 → 6.198 |
| H2 application error | 9.063 → 9.206 | 4.254 → 4.167 | 8.237 → 8.442 |
| H2 128 KiB echo | 159.417 → 157.863 | 21.121 → 26.028 | 28.351 → 40.533 |

Pair direction matters. Each sequence below follows campaign rounds zero,
one, and two. Positive CPU or latency changes are adverse:

| Workload | Paired RPS changes, % | Paired CPU/response changes, % | Paired p99.9 changes, % |
| --- | --- | --- | --- |
| H1 dynamic plaintext | −2.09, +9.08, +16.51 | +2.19, −8.66, −13.75 | +201.15, +0.57, −65.05 |
| H1 captured parameter | +3.16, −9.03, −3.92 | −3.05, +10.32, +7.18 | −27.39, −28.37, −89.91 |
| H1 application error | −10.10, +4.41, +28.88 | +11.46, −4.25, −22.20 | −65.35, −35.88, −49.14 |
| H1 1 KiB echo | −7.63, +4.76, −1.04 | +7.95, −4.54, +0.90 | +57.98, +291.35, −40.18 |
| H1 static plaintext | +1.69, +2.35, −0.45 | −1.84, −2.20, +0.47 | −3.83, −45.96, +146.05 |
| H2 application error | +0.47, −28.21, +2.87 | −3.20, +1.58, +0.40 | +2.48, +154.90, −1.30 |
| H2 128 KiB echo | +1.07, −4.29, −13.28 | −0.98, +1.03, +8.34 | +20.98, +63.74, +42.97 |

Large H2 echo also worsens at p99 in every pair, by +0.57%, +22.48%, and
+34.02%. Its lower cross-trial median CPU does not cancel these adverse tails
or its two slower throughput pairs. The H2 error trial with −28.21% throughput
also has +101.93% p99. The evidence retains every p99 pair as well. Shared-host
variation is a limitation, not an established explanation for those results.

A separately planned H1 confirmation runs twelve five-second alternating pairs
for capture and echo, plus four plaintext control pairs. Its 56 observations
are complete, totaling 58,481,357 checked responses. All outcomes and raw client
counts pass the separate reconciliation in
[confirmation evidence](core-h2-confirm-evidence.json) and
[trial rows](core-h2-confirm-trials.csv). Medians for this five-second cohort:

| H1 confirmation | Pairs | RPS, baseline → tuple | CPU µs/response | p99 ms | p99.9 ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Captured parameter | 12 | 205,188 → 224,422 | 4.953 → 4.256 | 1.763 → 4.017 | 19.045 → 17.776 |
| 1 KiB echo | 12 | 236,428 → 240,564 | 4.168 → 4.121 | 1.439 → 1.364 | 9.299 → 10.725 |
| Plaintext control | 4 | 299,165 → 263,461 | 3.306 → 3.767 | 1.749 → 0.852 | 7.558 → 3.705 |

Paired directions remain mixed. Captured throughput and CPU are adverse in
seven of twelve pairs each, despite their favorable cross-trial medians. Echo
throughput and CPU are adverse in six of twelve pairs each. Plaintext control
throughput and CPU are adverse in three of four pairs each. Captured p99 and
p99.9 are adverse in six of twelve pairs each; echo tails are adverse in five
of twelve pairs each. The evidence retains all ranges and per-pair values.

These short confirmation trials remain separate from the primary
fifteen-second Hyper/Axum dataset. They compare only frozen Maximus baseline and
tuple binaries on the same shared host and retain wide variation. They do not
establish performance parity, explain the earlier differences, or qualify H2.
They cannot erase or relabel an unfavorable primary result. A separate H2
follow-up script was prepared but never launched. It will not be run to seek a
favorable replacement result. No completed H2 confirmation or source-level
attribution of the H2 tail result is claimed.

## Environment and footprint

The shared Linux ARM64 VM has three CPUs. One Maximus worker uses CPU 0;
clients use CPUs 1–2. Four lifeing service containers remain active. There is
no exclusive host reservation. No compilation or fuzzing from this task runs
during timed campaigns. Each trial uses a separate three-second warmup client
before measurement. Baseline/candidate order alternates by round, and the
candidate/Hyper/Axum order rotates.

TLS uses one SHA-256-identified certificate across all twelve baseline and
candidate runner invocations and their competitor engines. The client verifies
the local certificate and requires TLS 1.3/H2. This improves the matching over
the separate certificates in the earlier ownership campaign without changing
that campaign's evidence.

Server CPU/request uses cgroup usage across the broader observation interval,
which ranges from 15.402 to 32.210 seconds for the fifteen-second client stages.
It is not exact handler CPU time. RSS and `peak_rss_kib` are endpoint samples,
not continuous peaks. k6 error-workload client CPU medians are near 178–182%
of two assigned CPUs, so these runs do not isolate maximum server capacity.
wrk percentiles and k6 request-duration percentiles have different collection
semantics. Their rates and tails cannot be pooled.

Matched Linux release builds use the same locked dependencies, thin LTO, and
one codegen unit. Example features match their baseline controls:

| Artifact | File bytes, baseline → tuple | Text bytes, baseline → tuple |
| --- | ---: | ---: |
| Comparison executable | 9,752,760 → 9,685,736 | 7,136,205 → 7,109,649 |
| Minimal hello | 1,279,584 → 1,279,584 | 827,893 → 827,549 |
| Conformance, `tls,http2` | 2,953,672 → 2,952,712 | 2,151,396 → 2,143,776 |

Conformance data falls from 59,344 to 59,296 bytes; BSS remains 2,560 bytes.
Stripped hello stays 922,400 bytes and stripped conformance stays 2,233,184.
Equal stripped sizes do not mean identical binaries. The comparison executable
contains other frameworks. Earlier conformance probes with additional optional
features were invalid controls and are excluded from this table. Footprint
savings do not qualify runtime performance.

## Correctness and outstanding work

The exact tuple host check records 172 passing test executions. Linux records
149 passes and one privileged BPF test ignored. Independent clients pass
HTTP/1 plaintext/TLS, HTTP/2, verified QUIC HTTP/3, WebSockets, feature flows,
concurrent uploads, and 2,062 raw-wire mutations/disconnects with post-stress
health. The added H2 test covers shutdown during an existing request-cap drain,
retaining one response and one GOAWAY. Existing blocked-write shutdown checks
remain. These finite checks do not establish complete RFC conformance.

**Exact tuple-source profiling and sanitizer fuzzing were deliberately not run
after this candidate was rejected.** The older baseline profile and the earlier
ownership candidate's fuzz counts are not reassigned to this tuple source.
The completed 172 host and 149 Linux passing test executions remain valid
evidence for their recorded source and scope. They do not override the
performance rejection.

The next isolated candidate will test optional H1 completion state together
with borrowed errors and rejected-CONNECT closure. It contains no H2 product
change and requires separate source identities and measurements. No outcome
for that candidate is assigned to the tuple report.

Raw artifacts are retained under `artifacts/performance-2026-10-20/` and
`artifacts/bench/`. The evidence embeds the analysis-only script and identifies
the frozen manifests, build log, fixture outcomes, and raw JSON hashes.
Reproduction uses `benches/compare-ownership.py` with the frozen baseline/tuple
binary paths, `--label core-h2-wrk --generator wrk --protocol plain` or
`--label core-h2-tls --generator k6 --protocol h2`, three repeats, and fifteen
seconds. Use fresh labels when rerunning. All earlier rejected experiments
and failed predictions remain separate and preserved.
