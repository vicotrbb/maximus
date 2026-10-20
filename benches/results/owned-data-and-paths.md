# Owned HTTP/2 bodies and path validation

This experiment evaluates two small runtime changes against commit
`6f0ebd61a7c5664bb1f70104dd20a67bc864e9c6`: move each response DATA frame's
`Bytes` into the HTTP/2 stream's pending buffer, and check decoded path controls
and separators in one character traversal. Neither change adds dependencies,
unsafe code, public APIs, or persistent fields. HTTP/1 runtime code is unchanged.

The measured allocation and focused path-validation benefits are clear. The
initial socket results were mixed; the longer confirmation campaign moved
unfavorably on all three aggregate workloads. This two-change candidate is
**not accepted as a regression-free result**. The binary label `retained` below
identifies that frozen experiment, not final acceptance. The [HTTP/2 ownership follow-up](h2-static-ownership.md) records the later
ablations and the decision to restore the original path validator. Large
HTTP/2 echoes remain behind the Hyper and Axum fixtures.

## Method and evidence

[Individual socket trials](owned-data-and-paths-trials.csv),
[allocation trials](owned-data-and-paths-allocations.csv), and
[samples and source hashes](owned-data-and-paths-evidence.json) preserve the
observations. The ignored local evidence directory
`artifacts/goal-performance-2026-10-19/` contains full logs, frozen source
snapshots, patches, profile data, and experiment drivers. Its `summarize.py`
checks source hashes, exactly 84 socket rows, three samples per combination,
positive and equal request/iteration counters, and zero request failures,
failed checks, dropped iterations, and nonzero measurement exits.

Linux ARM64 ran in a shared Docker VM with three CPUs and 15 other service
containers. Servers used CPU 0; k6 used CPUs 1 and 2. Each trial had 64 virtual
users, a three-second warmup, and a ten-second measurement. Three alternating
baseline/candidate rounds rotated competitor order. Baseline Maximus and retained
Maximus used frozen binaries; Hyper 1.12.0 and Axum 0.8.9 used the maintained
comparison fixture. Cargo lockfiles were unchanged. The trials completed
42,895,073 measured responses with no recorded errors.

CPU cost includes the runner's accounting interval around startup and teardown;
it is not a pure handler CPU measurement. RSS observations are endpoint samples,
not continuous peaks. Shared-host scheduling and client capacity limit inference.
The TLS Hyper fixture uses the existing shared router adapter, including router
cloning; it is not a minimal raw-Hyper lower bound. This is a fixture comparison,
not a ranking of every framework or deployment configuration.

## Allocation result

Five alternating pairs, 10,000 requests per measurement, used identical response
headers for the static and owned payloads. Median calls per request:

| HTTP/2 response | Baseline | Retained |
|---|---:|---:|
| Static payload | 37.0138 | 37.0035 |
| Owned payload | 39.0060 | 38.0032 |

The owned-response difference from the static fixture falls from about two
allocations to one. The remaining allocation owns the payload itself. Median
allocated bytes for the owned fixture fall from 6,971.7360 to 6,946.4592 per
request. Runtime noise affects fractional accounting. This is a complete loopback TCP
HTTP/2 keepalive measurement, not a claim that every application allocates these
totals. The regression assertion uses a relative static/owned budget; baseline
fails it and retained passes in all five pairs.

The implementation uses `Frame::data_mut()` and `mem::take` instead of cloning
`Bytes`. It preserves trailer handling, empty-frame wakeups, stream errors,
flow-control behavior, and the existing pending-buffer behavior.

## Path validation result

Seven alternating repetitions, 100,000 validations per input per repetition,
used black-boxed inputs/results and checked expected acceptance. Median ns/op:

| Input | Baseline | Retained |
|---|---:|---:|
| Root | 5 | 4 |
| `/users/42` | 18 | 17 |
| Long ASCII segment | 185 | 119 |
| Percent-encoded UTF-8 | 40 | 34 |
| Literal Unicode | 24 | 19 |
| Encoded separator | 23 | 22 |
| Dot segment | 13 | 11 |
| Encoded control | 26 | 23 |

These integer nanosecond measurements show local predicate cost, not predicted
application throughput. The new predicate rejects the same Rust Unicode control
characters and slash/backslash characters; NUL is covered by `is_control()`.
Regression tests cover percent-encoded code points through U+009F, representative
higher Unicode values, malformed escapes, and dot segments. Strict rejection of
encoded separators is this framework's policy. RFC 3986 distinguishes reserved
characters and their percent-encoded forms; it does not mandate every aspect of
this policy. See [RFC 3986 §2.2](https://www.rfc-editor.org/rfc/rfc3986.html#section-2.2)
and [RFC 9110 §4.2.3](https://www.rfc-editor.org/rfc/rfc9110.html#section-4.2.3).

## Socket medians

Requests/second, three observations per cell:

| Protocol/workload | Baseline Maximus | Retained Maximus | Hyper | Axum |
|---|---:|---:|---:|---:|
| HTTP/1 plaintext | 63,101 | 62,682 | 62,600 | 62,481 |
| HTTP/1 1 KiB echo | 55,385 | 55,676 | 55,092 | 55,756 |
| HTTP/1 parameter route | 60,811 | 60,154 | 61,718 | 63,591 |
| HTTP/1 JSON | 59,346 | 62,684 | 62,456 | 60,979 |
| TLS HTTP/2 plaintext | 61,618 | 63,350 | 62,938 | 61,921 |
| TLS HTTP/2 1 KiB echo | 50,632 | 52,415 | 52,795 | 53,283 |
| TLS HTTP/2 128 KiB echo | 5,401 | 5,370 | 7,256 | 7,186 |

Latency, CPU, memory, and every sample are in the evidence JSON and trial CSV.
For example, large-echo p99 changes from 25.078 to 25.405 ms and CPU accounting
from 161.471 to 162.856 µs/request. HTTP/1 plaintext and parameter-route medians
also move unfavorably. Three shared-VM repetitions cannot classify these small
shifts as either noise or causal regression. Do not interpret this table as a
no-regression certificate.

## Footprint

Matched release builds used the same container path and toolchain. No data/BSS
growth was measured:

| Binary | Baseline file bytes | Retained file bytes | Text-byte change |
|---|---:|---:|---:|
| Hello example | 1,279,584 | 1,279,584 | −168 |
| Conformance server | 2,953,672 | 2,953,488 | −8,420 |
| Comparison server | 9,594,080 | 9,594,064 | −164 |

These are exact artifacts, not portable size promises. The larger conformance
text change reflects code generation in that build. Earlier scanner variants'
size changes must not be attributed to these retained changes.

## Rejected HTTP/1 experiments

Incrementally scanning CRLF across fragmented reads greatly accelerated long
lines. The first implementation grew the line and incoming-body futures by eight
bytes and was rejected. A second implementation restored all measured future
sizes but slowed the 32-byte, one-byte-fragment case in every one of seven pinned
pairs: median 854 → 982 ns. Its 8,190-byte case improved from 8,930,682 → 134,188 ns,
but the short-input regression disqualified it. Both production variants were
reverted. Boundary tests remain for split CRLF, exact limits, truncation, empty
lines, and preservation of the next line.

The interrupted earlier socket campaign preserves 75 completed rows separately;
none is merged into the retained campaign. Failed builds, invalid stale-binary
probes, and rejected designs remain in the local evidence directory.

## Correctness and protocol qualification

The final retained source passed `scripts/check.sh`, including feature variants,
formatting, Clippy with warnings denied, documentation, and tests. Linux
all-features tests and independent HTTP/1, TLS 1.3, HTTP/2, HTTP/3, WebSocket,
JSON, multipart, SSE, gzip, CORS, HEAD and concurrent-upload clients passed.
Wire stress covered 2,062 mutations/disconnects plus concurrent clients and a
post-stress health check. The privileged eBPF test was skipped, not passed.

The h2spec policy gate passed on plaintext and TLS: raw results were 139/147,
with five recorded RFC 7540/RFC 9113 policy differences and three closure-race
cases passing held-response reruns. This is not a claim of 147/147 raw success
or full RFC certification. Response ownership changes must preserve
[RFC 9113 §8.1](https://www.rfc-editor.org/rfc/rfc9113.html#section-8.1)
message framing; the test boundary also retains
[RFC 9112 §7.1.1](https://www.rfc-editor.org/rfc/rfc9112.html#section-7.1.1)
chunk-line behavior.

A fresh baseline CPU-clock profile collected 765 samples with none lost.
Restricted kernel symbols limit kernel attribution. User-space samples included
clock calls and allocation/free activity; this is supporting profiling evidence,
not a hardware-PMU measurement or proof that all sampled allocation cost is
removed by the change.

Fresh sanitizer fuzzing completed without a reported failure: request parsing
9,813,249 executions, HTTP/1 1,343,538 executions, and HTTP/2 1,630,077 executions,
61 seconds each. These bounded runs do not exhaust the input space. They ran
after all timed socket trials, so compilation and fuzz load did not compete with
those measurements.

## Rejected HPACK storage experiment

The compiled RFC 7541 Huffman table generates 513 trie nodes with maximum child
index 512. A checked `u16` child-index prototype reduced Rust's measured node
size from 24 to 8 bytes on this target, reducing the shared vector's nominal
node storage from 12,312 to 4,104 bytes. Seven alternating, CPU-pinned probes
showed unfavorable decode costs: the long valid input median rose from 4,797
to 5,352 ns and the short input from 13 to 17 ns. The authority example improved
from 98 to 88 ns. The prototype was rejected; production retains `usize`
indices. The probes include output allocation and full result comparisons, not
just trie traversal. The first long fixture had invalid padding and failed
before collection; it was corrected to a valid length and its failure retained
separately. No data from that failed trial enters these medians.

This experiment changed only the internal representation, using the project's
existing table from [RFC 7541 Appendix B](https://www.rfc-editor.org/rfc/rfc7541.html#appendix-B).
It did not borrow another framework's implementation.

## Longer confirmation: candidate not accepted

[Seven longer pairs](owned-data-and-paths-confirmation.csv) used twenty-second
measurements on HTTP/1 plaintext, HTTP/1 parameter routing, and the 128 KiB TLS
HTTP/2 echo. All 42 rows had matching positive request/iteration counters and
zero failed requests, checks, dropped iterations, or nonzero exits, totaling
34,923,600 responses. Aggregate medians:

| Workload | Baseline RPS | Candidate RPS | Baseline p99 ms | Candidate p99 ms | Baseline CPU µs/request | Candidate CPU µs/request |
|---|---:|---:|---:|---:|---:|---:|
| HTTP/1 plaintext | 61,267.709 | 58,401.233 | 5.440 | 5.570 | 7.627 | 7.967 |
| HTTP/1 parameter | 63,028.810 | 59,588.499 | 5.241 | 5.531 | 7.564 | 7.602 |
| Large TLS HTTP/2 echo | 5,201.800 | 4,821.590 | 24.848 | 29.310 | 177.845 | 186.424 |

Paired throughput directions vary: the candidate is slower in four of seven
plaintext pairs, four of seven parameter pairs, and five of seven large-echo
pairs. The shared environment remains variable; these observations do not
identify which source change, code-generation effect, or external load caused
the differences. They do prevent claiming this candidate satisfies the requested
no-regression gate. No production changes have been committed or merged.
