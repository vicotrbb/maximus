# Borrowed errors and dynamic-route allocation experiment

**Decision: the initial candidate is not qualified as regression-free.** It
removes repeatable allocation work and some linked code, but the dynamic HTTP/1
error workload has a large adverse throughput and tail-latency result. Other
favorable observations do not override that result. Its cause is unresolved.

This report covers baseline `b5bb664bd6faf9b417cd8af5235ec12e7f0037de` and the
frozen `final-reviewed` candidate. That candidate contains three performance
changes: preserve borrowed `Error` payloads, omit capture metadata copies for
literal dynamic routes using `pattern.contains('{')`, and construct normalized
route shapes directly into a preallocated string with a preliminary length
scan. It also closes HTTP/1 connections after rejected CONNECT. It does not
contain the later persistent shutdown-future or constant-time route-shape
classification experiments. The label `final-reviewed` identifies this frozen
build, not final acceptance.

The [research note](../../docs/performance-research-2026-10-20.md) explains the
Rust ownership contracts and [RFC 9931 §8](https://www.rfc-editor.org/rfc/rfc9931.html#section-8)
requirement behind CONNECT closure. Tests verify that optimistic tunnel bytes
following rejected CONNECT do not dispatch a subsequent request. The closure
fix is a correctness change, not an attributed performance gain.

## Direct allocation observations

Five alternating baseline/candidate pairs use the same allocation diagnostic
and 10,000 operations per dispatch or conversion case. Registration measures
100 complete 64-route tables, including destruction. The counts below are
identical across all five observations for each variant.

| Operation | Allocation/reallocation calls, baseline → candidate | Requested bytes, baseline → candidate |
| --- | ---: | ---: |
| Static error conversion | 3 → 2 | 665 → 656 |
| Owned error conversion control | 3 → 3 | 665 → 665 |
| Register and destroy 64 dynamic routes | 389 → 325 | 20,280 → 16,184 |
| Literal dynamic GET | 2 → 1 | 609 → 608 |
| Literal dynamic POST | 2 → 1 | 609 → 608 |
| Dynamic 405 control | 5 → 5 | 698 → 698 |
| Static GET, HEAD, POST, PUT controls | 0 → 0 | 0 → 0 |

Literal dispatch still allocates its boxed handler future. A static error body
keeps its compiled bytes, while an owned error keeps its original payload
allocation. Registration removes one temporary segment vector per route.
The complete-loopback H1/H2 rows remain in the
[allocation CSV](ownership-registration-allocations.csv); their fractional
counts include runtime and client activity and show no comparable direct saving
for the unchanged successful-handler fixtures.

All five baseline diagnostic processes exit 101 at the new final assertion,
`route registration must not allocate temporary segment lists`. They emit all
17 measurement rows before that expected baseline failure. All five candidate
processes exit zero. The baseline is deliberately tested against the candidate's
guard. These are recorded baseline failures, not passing executions. The
instrumented timings include allocator counters, so this report makes no
component-speedup claim from their timing column. Requested allocation bytes
are traffic, not retained heap or RSS.

## Socket comparison

The [trial CSV](ownership-registration-trials.csv) contains 84 measurements and
54,324,333 checked responses. Every row has positive equal request, started,
and completed iteration counts, zero HTTP failures, zero failed checks, zero
drops, and exit zero. Independent analysis reconciles every row with its raw
trial CSV and client JSON. Separate warmup responses are excluded from that
total. The [evidence JSON](ownership-registration-evidence.json) retains all
samples, ranges, and 21 baseline/candidate pair changes.

Each cell is a median of three trials. Responses per second:

| Workload | Baseline Maximus | Candidate Maximus | Hyper 1.12.0 | Axum 0.8.9 |
| --- | ---: | ---: | ---: | ---: |
| Dynamic H1 plaintext | 58,823 | 59,941 | 56,034 | 55,743 |
| Dynamic H1 captured parameter | 55,799 | 57,806 | 40,972 | 50,363 |
| Dynamic H1 application error | 54,930 | 27,186 | 51,670 | 56,479 |
| Dynamic H1 1 KiB echo | 30,929 | 46,585 | 38,537 | 42,366 |
| Static TLS/H2 application error | 46,980 | 53,063 | 50,286 | 51,327 |
| Static TLS/H2 128 KiB echo control | 4,246 | 4,619 | 6,343 | 6,350 |
| Static H1 plaintext control | 53,880 | 56,071 | 46,888 | 55,242 |

The two plaintext rows use distinct runs and separately retain their competitor
samples. They are not pooled. The large-body H2 control's product source is
unchanged by these optimizations. Its apparent improvement cannot be assigned
to the route or error changes. It still trails both comparator fixtures.

Median CPU and latency observations, baseline → candidate:

| Workload | Server CPU µs/response | p99 ms | p99.9 ms |
| --- | ---: | ---: | ---: |
| Dynamic H1 plaintext | 8.046 → 7.722 | 5.446 → 5.538 | 10.503 → 9.892 |
| Dynamic H1 captured parameter | 8.931 → 8.619 | 6.064 → 5.722 | 12.169 → 10.419 |
| Dynamic H1 application error | 8.539 → 8.538 | 6.273 → 15.278 | 13.458 → 34.785 |
| Dynamic H1 1 KiB echo | 12.141 → 10.889 | 11.601 → 7.119 | 22.900 → 12.676 |
| Static TLS/H2 application error | 13.031 → 10.722 | 7.969 → 7.009 | 14.720 → 13.563 |
| Static TLS/H2 128 KiB echo control | 195.568 → 190.194 | 35.487 → 28.745 | 101.332 → 38.853 |
| Static H1 plaintext control | 8.802 → 8.436 | 6.437 → 5.844 | 13.990 → 10.948 |

The dynamic H1 error throughput changes are −3.99%, −51.11%, and −50.82% in
the three matched pairs. Its baseline range is 40,403–55,277 responses/s, versus
19,755–52,736 for the candidate. One candidate p99.9 is 429.714 ms. CPU/request
changes in those same pairs are +5.31%, −15.71%, and +33.52%. The nearly equal
cross-trial CPU medians conceal that variation. They do not explain away the
throughput or tail result.

Source inspection finds the intended same GET endpoint, status 500, and exact
`handler error` body on both revisions. The added CONNECT close predicate does
not apply to that GET. This does not isolate the slowdown's cause. No faulty
component, host event, or benchmark artifact has been demonstrated as the
explanation. The bundle remains unqualified.

## Measurement scope

The server runs one worker on VM CPU 0. k6 1.6.0 uses CPUs 1–2 in the same
three-CPU Linux ARM64 Docker VM. Each trial uses 64 constant virtual users, a
separate three-second warmup, and a fifteen-second measured client process.
Baseline precedes candidate in rounds zero and two, with reversed order in
round one. Candidate, Hyper, and Axum order rotates by round. Source snapshots
have identical comparison-server code and locked dependencies. Both variants
use the same measured workload and runner hashes recorded in the evidence.
Release builds use thin LTO and one codegen unit.
Maximus explicitly selects io_uring and raises the admission memory budget to
4 GiB in the fixture. Competitor policies and adapters differ. In particular,
the TLS Hyper fixture retains the shared router-clone adapter. These are
comparisons of the maintained fixtures, not identical-policy engine isolation.

TLS verifies a generated local CA certificate and restricts negotiation to
TLS 1.3 with H2 checked on every response. Each runner invocation generates a
new RSA-2048 key and certificate. Candidate and competitor engines within one
invocation share that certificate. Its paired baseline invocation uses a
different key and certificate with the same TLS configuration. The campaign
does not use byte-identical baseline/candidate certificates.

Four lifeing service containers remain active. An unrelated Positron cargo-test
process was observed on the host. The environment is shared, without exclusive
CPU reservation. Those facts limit attribution but do not invalidate selected
unfavorable rows or establish the cause of the error result.

Server CPU/request divides the cgroup usage delta by completed responses across
the broader before/after observation interval. Those intervals range from
15.566 to 73.372 seconds despite the fifteen-second client stage. They include
control and observation delay and are not exact handler CPU measurements.
Client CPU covers the measured client's container lifetime. RSS is an endpoint
sample. `peak_rss_kib` is the maximum of the before/after samples because no
periodic sampling was configured. It is not a continuous peak. Three shared-VM
pairs and closed-loop client load do not establish server capacity, framework
rankings, near-capacity tails, or a no-regression guarantee.

## Footprint, profiling, and correctness

| Linux ARM64 release artifact | File bytes, baseline → candidate | Text bytes, baseline → candidate |
| --- | ---: | ---: |
| Comparison executable | 9,752,760 → 9,686,504 | 7,136,205 → 7,113,533 |
| Hello example | 1,279,584 → 1,279,584 | 827,893 → 827,549 |
| Conformance example | 2,953,672 → 2,953,728 | 2,151,396 → 2,151,180 |

The conformance file grows 56 bytes despite smaller text. Its stripped file
remains 2,233,184 bytes; stripped hello remains 922,400 bytes. Equal stripped
sizes do not mean byte-identical binaries. The comparison executable includes
other frameworks and is not the standalone Maximus library size.

A [baseline CPU-clock profile](ownership-registration-profile.json) records
2,740 samples with none lost during a separate TLS/H2 128 KiB echo run. It
includes AES-GCM, timer, allocation/free, unresolved libc, and kernel activity.
Its binary predates the reviewed fixture build and has a separate recorded hash.
No profile traffic enters the 84 measurements. CPU-clock samples with restricted
kernel symbols are not hardware cycle counts or a causal breakdown of the
retained allocation savings.

The reviewed source passes the check script with 171 test executions and the
Linux suite with 148 passes and one privileged BPF test ignored. Refreshed
independent clients pass HTTP/1 plain/TLS, HTTP/2, verified QUIC HTTP/3,
WebSockets, feature flows, concurrent uploads, and 2,062 wire mutations and
disconnects with post-stress health. Request and H1 sanitizer fuzzing complete
8,206,347 and 1,198,013 executions respectively, 61 seconds each, without a
reported failure. These finite checks qualify observed correctness, not the
bundle's performance or universal RFC conformance.

## Reproduction and evidence

The evidence JSON embeds the analysis script. Save that script under
`artifacts/performance-2026-10-20/analyze-initial.py` and run it from this
repository to reconcile retained raw CSVs and client JSON, regenerate summaries,
and check source identities. It performs no benchmark. The raw archive is under
ignored `artifacts/performance-2026-10-20/` and `artifacts/bench/`.

The campaign command is
`python3 benches/compare-ownership.py --baseline /target/oct20-baseline-reviewed/maximus-comparison --candidate /target/oct20-final-reviewed/maximus-comparison --label ownership-registration --repeats 3 --duration 15s`.
Use a fresh label to avoid overwriting measurements. Rebuild the frozen
snapshots and verify the evidence's binary and tool hashes before comparison.
Later candidates need their own source identities, measurements, and decision.
