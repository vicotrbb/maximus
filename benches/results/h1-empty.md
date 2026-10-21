# Optional H1 completion state and borrowed error bodies

**Decision: ALLOCATION-VALIDATED; SOCKET PERFORMANCE UNQUALIFIED under the user's
strict no-regression requirement.** The frozen candidate removes a repeatable
allocation from empty-framed H1 requests and preserves static error-body bytes.
Its five-second socket campaign has unresolved adverse CPU and latency
observations. No fastest-framework, universal speedup, or performance-acceptance
claim follows from these results.

The baseline is `b5bb664bd6faf9b417cd8af5235ec12e7f0037de`, comparison binary
`be05e0aea314cfdda839441c4c3c923e4cae9c8100f4836f382d7ca1e703e8f2`.
The candidate is frozen by `h1-empty-manifest.json`, comparison binary
`6ea0cb75fc30f8caff4895b2a56f7508e0e6d8d36126a21f295c2a08e165528a`.
It contains three product changes:

- H1 completion state is `None` for parsed empty framing. Fixed and chunked
  bodies retain their existing shared completion cell. Unread-body closure and
  upgrade checks test that optional cell.
- Error conversion preserves borrowed static bytes and delegates each ownership
  variant to the existing tuple response implementation.
- Rejected CONNECT closes the H1 connection, preserving the requirement in
  [RFC 9931 §8](https://www.rfc-editor.org/rfc/rfc9931.html#section-8).

H2, router, and worker-loop implementations match baseline. The preceding
persistent-H2 candidate remains [rejected](core-h2.md); none of its allocation
or socket results is attributed to this source. No dependency, public API,
configuration, or unsafe code is added.

## Allocation and component evidence

Five fresh baseline/candidate allocation pairs emit eighteen cases each. The
complete-loopback measurements include client and runtime allocations. Values
are medians. Calls include reallocations, and requested bytes are allocation
traffic rather than retained heap or RSS.

| Case | Calls/operation, baseline → candidate | Requested bytes/operation |
| --- | ---: | ---: |
| Static GET, HEAD, POST, PUT | 0 → 0 | 0 → 0 |
| Static text with Content-Type | 2 → 2 | 656 → 656 |
| Static error | 3 → 2 | 665 → 656 |
| Owned error | 3 → 3 | 665 → 665 |
| Register mixed 64-route table | 389 → 389 | 20,280 → 20,280 |
| Register literal 64-route table | 389 → 389 | 18,552 → 18,552 |
| Dynamic GET and POST | 2 → 2 | 609 → 609 |
| Dynamic 405 | 5 → 5 | 698 → 698 |
| Complete H1 keepalive | 15.0221 → 14.0230 | 2,636.0776 → 2,612.4880 |
| H2 invalid first frame control | 33.4410 → 33.4520 | 14,447.6960 → 14,453.5120 |
| H2 large static control | 34.9308 → 34.8985 | 7,947.4048 → 7,932.7160 |
| H2 large owned control | 35.9104 → 35.9099 | 40,706.1424 → 40,705.7858 |
| Complete H2 static control | 30.0093 → 30.0176 | 6,899.2408 → 6,903.0256 |
| Complete H2 owned control | 31.0097 → 31.0155 | 6,912.4232 → 6,915.0680 |

The H1 allocation difference agrees with removing the completion-state
allocation on the empty-framing branch. Static-error conversion removes its
payload copy. Unchanged H2 controls retain their fractional differences; no H2
allocation saving is claimed. The
[allocation CSV](h1-empty-allocations.csv) contains all 180 rows, including
instrumented timings that do not establish component speedups.

The fourteen earlier component process observations are reused because both
baseline and candidate component binaries are byte-identical to their earlier
counterparts. The candidate component SHA-256 is
`586b1964aca7faea96fc567fb8047caf07533b3b8fdd3879bbe656ca5a43b379`.
H1 completion-state code and the reverted H2 code are absent from that linked
component artifact. Reuse is explicit in the combined fixture outcomes. These
are not fourteen new timing executions and do not measure the H1 optimization.
Together with ten fresh allocation processes, all 24 recorded exits are zero.

Reused component medians, ns/operation:

| Component | Baseline | Candidate artifact |
| --- | ---: | ---: |
| Static text | 35.212 | 33.153 |
| Static error | 38.194 | 32.061 |
| Owned error | 35.573 | 35.705 |
| Literal route | 105.270 | 108.638 |
| Captured route | 154.272 | 154.025 |
| 405 | 114.692 | 112.934 |
| Mixed 64-route registration | 22,964.632 | 22,745.415 |
| Literal 64-route registration | 16,265.963 | 16,300.755 |

All seven static-error pairs are faster. The
[component CSV](h1-empty-components.csv) preserves every reused observation and
its identity. The allocation diagnostic candidate is a different executable,
SHA-256 `c196635dbc1c05ede3fe96f2473579898c41ec071f42f566b75ca6b526a723c1`.

## Five-second socket cohort

The [trial CSV](h1-empty-trials.csv) contains 84 observations and 76,058,290
checked responses. Sixty wrk observations complete 72,066,199 responses;
twenty-four k6 observations complete 3,992,091. Every row has positive counts,
zero response-check failures, zero network failures, zero dropped requests,
and exit zero. k6 iteration starts, requests, and completions match. wrk checked
responses match its request count. Warmup responses are excluded from these
totals.

The [evidence JSON](h1-empty-evidence.json) reconciles measured/warmup JSON,
CSV rows, outcomes, source manifests, expected build hashes, image identities,
workload settings, fixture row counts, and the shared TLS certificate. It
retains all 21 matched pairs and raw artifact hashes. An initial export read a
stale top-level binary; that error was caught before measurement. The stale
build log remains preserved, and these trials use the corrected frozen hash.
Subsequent forced release rebuilds from the frozen sources reproduce both
baseline and candidate comparison binaries byte-for-byte. The retained
`verify-baseline-build.log` and `verify-h1-empty-build.log` record those hashes.
A development-profile clean had previously left release timestamps intact;
that build-control pitfall was caught before the isolation campaign. No primary
trial is invalidated by either corrected build-control error.

Each cell below is a median of three five-second trials. H1 uses wrk, two client
threads, and 64 connections:

| H1 workload | Baseline RPS | Candidate RPS | Hyper 1.12.0 | Axum 0.8.9 |
| --- | ---: | ---: | ---: | ---: |
| Dynamic plaintext | 243,282 | 252,272 | 362,494 | 291,461 |
| Captured parameter | 280,443 | 283,868 | 320,117 | 306,242 |
| Application error | 134,790 | 174,790 | 364,715 | 248,683 |
| 1 KiB echo | 124,821 | 143,321 | 234,115 | 243,545 |
| Static plaintext control | 93,145 | 330,705 | 410,978 | 272,030 |

TLS/H2 uses k6 with 64 constant virtual users and remains a separate generator
cohort:

| TLS/H2 workload | Baseline RPS | Candidate RPS | Hyper 1.12.0 | Axum 0.8.9 |
| --- | ---: | ---: | ---: | ---: |
| Application error | 64,227 | 64,957 | 62,307 | 63,686 |
| 128 KiB echo | 5,766 | 5,730 | 8,095 | 6,813 |

CPU and latency medians, baseline → candidate:

| Workload | Server CPU µs/response | p99 ms | p99.9 ms |
| --- | ---: | ---: | ---: |
| H1 dynamic plaintext | 4.029 → 3.919 | 1.834 → 0.934 | 18.923 → 4.406 |
| H1 captured parameter | 3.495 → 3.495 | 2.189 → 1.662 | 39.820 → 37.086 |
| H1 application error | 3.684 → 3.692 | 7.216 → 13.174 | 49.919 → 32.269 |
| H1 1 KiB echo | 4.058 → 4.063 | 7.793 → 2.966 | 44.805 → 11.644 |
| H1 static plaintext | 3.429 → 2.981 | 6.987 → 6.036 | 44.273 → 31.425 |
| H2 application error | 8.452 → 8.321 | 4.510 → 4.521 | 8.528 → 9.920 |
| H2 128 KiB echo | 157.012 → 158.859 | 21.992 → 23.612 | 33.899 → 51.125 |

All three pair changes appear below in round order. Positive CPU and latency
changes are adverse:

| Workload | RPS changes, % | CPU/response changes, % | p99 changes, % | p99.9 changes, % |
| --- | --- | --- | --- | --- |
| H1 dynamic plaintext | +0.28, +3.70, +109.72 | +1.34, −2.74, −43.67 | −62.00, −38.19, −87.01 | −89.51, −64.57, −76.42 |
| H1 captured parameter | +1.22, +2.65, +4.40 | +0.00, −2.12, −7.40 | −65.37, −12.34, +97.64 | −93.02, +65.84, +31.67 |
| H1 application error | +1.03, +29.68, −27.05 | −1.11, −0.40, +1.00 | −64.13, +82.57, +56.91 | −68.48, −35.36, +52.46 |
| H1 1 KiB echo | −2.76, +14.82, +7.89 | +1.82, −17.84, +0.11 | +2.84, −88.40, +229.05 | −26.86, −77.87, +69.89 |
| H1 static plaintext | −3.77, −14.61, +288.14 | +3.11, +5.92, −15.31 | +1015.71, +634.12, −94.79 | +1498.42, +53.50, −95.27 |
| H2 application error | +1.23, +10.42, +1.14 | −1.55, −4.02, −0.91 | −2.88, −14.92, +4.07 | −4.95, −22.35, +16.50 |
| H2 128 KiB echo | −5.51, +0.88, −1.27 | +1.02, +1.72, −1.18 | +50.74, −8.38, +8.01 | +166.61, −22.72, +51.47 |

The large H2 echo has higher median CPU and tails, with two slower throughput
pairs and two adverse p99.9 pairs. H2 source is unchanged, but that does not
permit declaring the observation irrelevant or the bundle regression-free.
Code layout, runtime interactions, and shared-host effects remain possible;
none is established as the cause.

The static plaintext median suggests a large improvement, yet two of three
paired throughputs are lower and both corresponding CPU and tail observations
are worse. The third pair's +288.14% throughput dominates the cross-trial
picture. These mixed results must not be presented as a repeatable speedup.
Small adverse H1 CPU medians and the application-error p99 increase also remain
visible. Neither allocation savings nor another favorable endpoint offsets
unresolved regressions under the stated gate.

## Environment, footprint, and limits

One server worker uses CPU 0 in a shared three-CPU Linux ARM64 Docker VM;
clients use CPUs 1–2. Four lifeing service containers remain active. There is no
exclusive CPU reservation. No build or fuzz load from this task runs during the
timed campaign. Each five-second stage follows a separate three-second client
warmup. Order alternates baseline/candidate by round and rotates competitor
order. The H2 cohort shares one verified local certificate across all variants,
requires TLS 1.3, and checks H2 on every response.

These are short closed-loop observations. They do not measure production
capacity, fixed-arrival overload behavior, or stable near-capacity tails.
Server CPU is cgroup accounting over surrounding intervals of 5.365–6.514
seconds, not exact handler cost. RSS and `peak_rss_kib` are endpoint samples,
not continuous peaks. wrk and k6 percentiles have different collection semantics
and are not pooled. The Hyper TLS fixture retains the shared router-clone
adapter; policy costs differ across frameworks. This cohort is not pooled with
the earlier fifteen-second tuple or five-second H1 confirmation datasets.

Matched Linux release footprint observations:

| Artifact | File bytes, baseline → candidate | Text bytes, baseline → candidate |
| --- | ---: | ---: |
| Comparison executable | 9,752,760 → 9,752,536 | 7,136,205 → 7,127,261 |
| Minimal hello | 1,279,584 → 1,279,584 | 827,893 → 827,585 |
| Conformance, `tls,http2` | 2,953,672 → 2,953,664 | 2,151,396 → 2,147,148 |

Conformance data/BSS remain 59,344/2,560 bytes. Stripped hello and conformance
remain 922,400 and 2,233,184 bytes respectively. Equal sizes do not mean
byte-identical files. The comparison executable includes competing frameworks.
The release profile keeps thin LTO and one codegen unit. Smaller text and the
eight-byte conformance file reduction do not establish socket performance.

## Correctness and remaining evidence

Exact-source checks record 176 host test executions, 151 Linux passes, and one
privileged BPF skip. Independent HTTP/1 plaintext/TLS, HTTP/2, verified QUIC
HTTP/3, WebSockets, feature flows, concurrent uploads, and 2,062 wire mutations
and disconnects with post-stress health pass. Empty, zero-length, fixed,
chunked, unread-body, pipeline, and upgrade behavior are covered by the source's
regression checks. Request framing remains independent of request method, as
specified by [RFC 9112 §6](https://www.rfc-editor.org/rfc/rfc9112.html#section-6).

Final-source sanitizer fuzzing completes 9,017,091 request, 1,291,814 H1, and
1,599,769 H2 executions, 61 seconds each, without a reported failure. These are
finite checks, not exhaustive safety or RFC-conformance proof.

The [exact-source profile summary](h1-empty-profile.json) validates the candidate
binary hash for two separate twenty-second CPU-clock recordings. H1 plaintext
traffic completes 5,100,540 checked responses and yields 2,955 samples with none
lost. Large H2 echo completes 79,223 checked responses and yields 2,662 samples
with none lost. Each profile's client runs fifteen seconds, outside every timed
comparison cohort. H1 samples include timers, clock calls, parsing, and writes;
H2 samples include AES-GCM, timers, and transport work. Restricted kernel symbols
and unresolved libc symbols limit attribution. CPU-clock samples are not
hardware cycles or a paired causal breakdown of the allocation saving.

A separate fixed [H2 attribution diagnostic](isolate-h2.md) completes 42
observations across 21 pairs, seven rounds for each of an identical-baseline
A/A control, borrowed-error-only source, and H1-completion/CONNECT-only source.
Its 859,622 responses reconcile, but attribution remains unresolved. The same
A/A executable spans −3.16% to +90.16% paired RPS and −53.45% to +50.13% p99.9.
That variability is not established as the cause of primary adverse results.
Error-only paired p99.9 has a +35.95% median, versus −39.53% for H1-only. These
separate observations neither replace the primary Hyper/Axum comparison nor
qualify its unresolved socket performance.

Raw source snapshots, logs, stale-export evidence, binaries, and measurement
artifacts remain under `artifacts/performance-2026-10-20/` and `artifacts/bench/`.
The evidence JSON embeds the analysis script, exact manifest/build-log choices,
fixture outcomes, and raw hashes. Reproduction uses the frozen baseline and
candidate with `benches/compare-ownership.py`, three repeats, five-second stages,
`--generator wrk --protocol plain` for `h1-empty-wrk`, and
`--generator k6 --protocol h2` for `h1-empty-tls`. Use fresh labels to preserve
every original observation.
