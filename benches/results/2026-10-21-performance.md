# SSE and gzip performance investigation

Date: 2026-10-21. Original product source: `df8f279fb7a2a61508527595c9d76e902644b281`.

The local implementation retains SSE encoding and gzip negotiation improvements
based on repeated component timings, allocation counts, native loadable size,
stripped-file checks, and correctness gates. Socket observations remain noisy
and do not establish an overall throughput improvement. Body layout, status
serialization, and HPACK lookup experiments were rejected. No competitor implementation was copied.
The [standards research](2026-10-21-http-research.md) records primary sources and
constraints separately from measurements.

## SSE removes temporary normalization buffers

`Event::encode` previously normalized CRLF and CR through two owned replacement
strings before splitting on LF. It now splits borrowed input at CR, removes one
LF after each CR, and splits each remaining segment at LF. It writes retry
milliseconds directly into the output string. The final output allocation and
streaming backpressure remain.

A field writer adds one ASCII space when data or metadata starts with a space.
This preserves the caller's leading space because an SSE parser removes one
optional space immediately after the colon. Ordinary fields retain their
existing wire bytes. This is a correctness correction under the
[WHATWG event stream interpretation rules](https://html.spec.whatwg.org/multipage/server-sent-events.html#event-stream-interpretation).
The failing-first test records the previous loss of data and metadata spaces.

The line-ending test exhausts 5,461 input strings over `a`, CR, LF, and UTF-8 `é`,
including empty inputs and trailing separators. Other tests cover metadata
injection and retry serialization.

## Gzip negotiation stores only the decision it uses

The compression middleware only chooses whether gzip is acceptable. It does
not rank several output codings. Two optional booleans replace two optional
floating-point weights. Boolean AND preserves the previous minimum-weight
zero-veto rule for duplicate coding entries. Explicit gzip still overrides the
wildcard, including when gzip is forbidden. Unrelated coding entries are skipped
before parsing their parameters.

The existing policy chooses gzip when it is acceptable; it does not honor all
relative preferences against identity. For example, `gzip;q=0.5,identity;q=1`
expresses a preference for identity while still accepting gzip. This limitation
predates the change. RFC 9110 describes the highest nonzero weight as preferred,
but does not require every origin response to maximize that preference.
[Sections 12.1 and 12.5.3](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.5.3)
distinguish preference from acceptability. A future ranking policy needs numeric
weights again.

The decimal parser implements the qvalue grammar in
[RFC 9110 section 12.4.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.4.2).
`1.`, `1.000`, and `0.001` remain valid. Case-insensitive q lookup fixes the
previous acceptance of `gzip;Q=0`.

Malformed weights such as `+0.5`, `.5`, `1e0`, and `0.0001` now make that coding
occurrence unacceptable. The former float parser could accept these values.
This is an explicit recovery-policy change, not an equivalence claim for invalid
input. RFC 9110 permits error recovery. Existing recovery for unknown parameters
and first-q selection is preserved. The implementation does not validate the
entire Accept-Encoding grammar. Tests exhaust all 1,000 three-digit fractions
for both leading digits and preserve multiline, duplicate, and wildcard rules.

## Repeated component measurements

Nine alternating baseline/candidate repetitions run on CPU 0. Times below are
medians; percentage changes are medians of paired ratios, so they need not equal
the ratio of the two median times. Every observation, including peer controls
and adverse repetitions, is retained in [the component data](2026-10-21-components.json).

| Component and input | Baseline ns/op | Candidate ns/op | Paired time change | Slower candidate repetitions |
| --- | ---: | ---: | ---: | ---: |
| negotiation: absent | 3.934 | 3.871 | -3.43% | 3/9 |
| negotiation: gzip | 19.177 | 14.088 | -23.39% | 0/9 |
| negotiation: weighted | 82.322 | 46.300 | -42.59% | 0/9 |
| negotiation: veto | 50.244 | 34.622 | -33.08% | 0/9 |
| negotiation: unknown | 78.687 | 40.876 | -46.69% | 0/9 |
| negotiation: duplicates | 73.342 | 51.097 | -25.98% | 1/9 |
| negotiation: smallest | 30.595 | 22.963 | -22.17% | 0/9 |
| events: empty | 185.302 | 183.795 | -0.74% | 4/9 |
| events: single | 225.798 | 193.337 | -12.24% | 1/9 |
| events: multiline | 239.587 | 219.158 | -8.31% | 1/9 |
| events: crlf | 275.435 | 231.750 | -14.04% | 0/9 |

The event measurement includes construction, response conversion, and complete
body consumption. Hyper uses pre-encoded bytes as a lower-level reference;
Axum constructs native events. Gzip negotiation is a private decision probe
compiled from the actual module, not an exported framework API. Full gzip
middleware is measured separately over sockets against Hyper and Axum.

Allocator instrumentation is a separate diagnostic, not a CPU timing source:

| SSE input | Allocation requests before → after | Requested bytes before → after |
| --- | ---: | ---: |
| Empty | 7 → 7 | 937 → 937 |
| Single line | 11 → 9 | 971 → 958 |
| Multiline | 12 → 10 | 1,024 → 998 |
| Mixed CR/LF | 14 → 11 | 1,102 → 1,064 |

These counts cover the complete in-process event response, not the network
stack. They are allocation requests, not retained heap size. Instrumented timing
is deliberately excluded: allocator counters distort execution. The identical
Body executable exhibits a +5.72% paired median in one control case, illustrating
why small timing differences cannot be assigned to source changes.

The supplementary [metadata and retry probe](2026-10-21-event-fields.json)
also uses nine alternating repetitions and checked wire output. Hyper remains
a pre-encoded reference; Axum constructs the same logical fields through its API.

| SSE fields | Baseline ns/op | Candidate ns/op | Paired time change | Slower repetitions |
| --- | ---: | ---: | ---: | ---: |
| retry | 248.733 | 222.806 | -12.89% | 1/9 |
| metadata | 247.412 | 227.855 | -8.44% | 0/9 |
| combined | 283.293 | 252.562 | -11.00% | 1/9 |

The combined metadata/retry case reduces allocation requests from 16 to 13
and requested bytes from 1,088 to 1,061. These instrumented CPU times are also
excluded from performance claims.

## Response-checked socket comparisons

The main campaign completed 96 measured rows and 93,807,252 requests with zero
status, body, protocol, network-error, failure, dropped-iteration, or completion
mismatches. There are three independent timed trials per source and workload,
not millions of independent framework comparisons. Each trial has a three-second
warmup and eight measured seconds. Source order alternates and peer order rotates.
All raw rows, including adverse observations, are in [the socket data](2026-10-21-sockets.json).

These are closed-loop saturation observations. RPS and latency medians below
are computed across the three timed trials. They need not match the median of
paired percentage changes. The control and follow-up findings below bound interpretation of the adverse rows.

| Protocol and workload | Maximus baseline RPS | Maximus candidate RPS | Hyper RPS | Axum RPS | Maximus p99 ms before → after |
| --- | ---: | ---: | ---: | ---: | ---: |
| plain /plaintext | 260,500 | 267,661 | 343,271 | 295,809 | 2.13 → 1.85 |
| plain /users/42 | 212,648 | 258,690 | 305,607 | 258,004 | 2.92 → 2.30 |
| plain /error | 266,632 | 223,043 | 349,948 | 305,210 | 2.23 → 3.84 |
| plain /sse | 39,956 | 41,144 | 44,401 | 43,513 | 8.37 → 8.41 |
| h1 /sse | 34,237 | 38,555 | 41,866 | 40,468 | 9.97 → 8.52 |
| h2 /sse | 39,195 | 36,252 | 42,425 | 40,808 | 7.76 → 8.65 |
| h2 /echo (128 KiB) | 4,047 | 4,239 | 5,449 | 5,924 | 35.87 → 33.68 |
| plain /compress | 23,741 | 24,640 | 25,836 | 24,677 | 14.34 → 13.35 |

The unchanged error route has a slower candidate RPS median. HTTP/2 SSE also
has adverse throughput and p99 observations. Neither is hidden by the faster
component results. The three paired error-route RPS changes are -18.79%,
+25.43%, and -16.35%. The untouched typed-route changes are +21.65%, +24.70%,
and +2.93%. The same-binary controls below show why these large, reversing
observations cannot be assigned to a source change from this cohort alone.
Hyper and Axum are faster in several workloads. This campaign does not establish best-in-class throughput.

### Identical-binary controls and the HTTP/2 follow-up

Both control variants execute the same candidate binary. All 36 control rows
pass every response and completion check. The signed medians below are real
observations of measurement variation, not performance effects of source edits.
Full rows and individual ratios are in [the control data](2026-10-21-controls.json).

| Identical-binary workload | Paired RPS change | Paired p99 change | Paired CPU/request change |
| --- | ---: | ---: | ---: |
| plain /plaintext | +2.64% | -9.29% | -1.72% |
| plain /users/42 | -3.02% | +55.87% | +2.37% |
| plain /error | -17.01% | +29.43% | +18.46% |
| plain /sse | +13.61% | -14.11% | -11.83% |
| h2 /sse | -0.31% | +10.11% | +2.33% |
| plain /compress | -0.11% | -3.89% | +0.58% |

The unchanged error route spans -24.36% to +13.82% paired RPS. Its paired p99
spans -32.82% to +44.48%. This reproduces the scale and direction of adverse
source-cohort observations without changing code. HTTP/2 SSE controls span
-7.65% to -0.24% RPS and -4.53% to +11.11% p99. These data are displayed
separately; no control subtraction or normalization creates a claimed gain.

The initial HTTP/2 SSE tail result triggers a separate nine-pair alternating
baseline/candidate follow-up, preserving the original cohort. All 18 rows pass.
Its paired medians are -1.80% RPS, +1.07% p99, and -0.79% CPU per request.
The larger original tail shift does not persist across this follow-up.
[All nine pairs](2026-10-21-h2-followup.json) remain available, including slower
candidate repetitions. This supports neither a reliable whole-server gain nor
a universal proof of no regression. The retained performance claims concern
the repeated component CPU and allocation improvements; socket screening does
not isolate an adverse source effect beyond the observed environment variation.

## Native linked size

These are `size` totals for text, data, and BSS in clean Linux aarch64 release
builds. They are not process RSS or a claim about all possible applications.
Both sources use identical corrected dependencies and build settings.

| Maximus application | Baseline bytes | Candidate bytes | Difference |
| --- | ---: | ---: | ---: |
| Minimal hello | 858,049 | 858,049 | 0 |
| Minimal SSE | 841,193 | 840,465 | -728 |
| Minimal gzip | 917,992 | 904,744 | -13,248 |
| TLS/HTTP/2 SSE | 2,214,828 | 2,201,764 | -13,064 |
| Existing full-feature application | 3,715,198 | 3,697,858 | -17,340 |

File bytes are recorded separately in [the footprint data](2026-10-21-footprints.json).
The same `strip --strip-all` operation is applied to copied binaries on both
sides. Original profiling binaries remain untouched.

| Application | Unstripped file bytes before → after | Stripped file bytes before → after |
| --- | ---: | ---: |
| Hello | 1,279,584 → 1,279,584 | 922,400 → 922,400 |
| SSE | 1,201,480 → 1,201,976 | 856,808 → 856,808 |
| Gzip | 1,277,232 → 1,275,760 | 922,344 → 922,344 |
| TLS/HTTP/2 SSE | 2,949,176 → 2,949,464 | 2,233,128 → 2,233,128 |
| Full features | 4,932,128 → 4,931,328 | 3,744,152 → 3,744,152 |

The unstripped SSE files grow by 496 and 288 bytes. This limitation is explicit:
the change does not reduce every symbol-bearing executable's file length.
Stripped file lengths stay equal because ELF segment alignment and padding
absorb the section reductions. Loadable section totals shrink, but this is not
a claim of smaller deployment downloads in these fixtures.

The hello executables are byte-for-byte identical. The gzip baseline retains
seven float-parser symbols, including the power-of-five table. Those symbols
are absent from the candidate gzip application.

The isolated mixed-framework executable grows by 3,312 bytes, from 4,850,023 to
4,853,335. That diagnostic remains visible. It contains competitors and test
paths outside the product dependency graph. The dedicated gzip comparison
server shrinks by 17,288 bytes. The event component executable shrinks by 720
bytes. Body and status control executables are byte-for-byte identical.

## Rejected experiments

| Experiment | Useful observation | Rejection reason |
| --- | --- | --- |
| H1 `StatusCode::as_str` serialization | Removes general formatting at the call site | Retains a digit table and grows minimal applications by about 3 KiB |
| H1 direct digit arithmetic | Simple byte serialization | Minimal hello grows by 204 bytes and SSE by 8,060 bytes |
| H1 shared digit helper | Avoids duplicated arithmetic | Minimal hello grows by 524 bytes and SSE by 172 bytes |
| HPACK stop after matching static-name group | Literal lookups improve about 38–39% | Repeated exact-match regressions across the 61-entry table |
| HPACK find-first then scan group | Similar literal gains | Exact-match regressions remain |
| SSE delimiter-index iterator | Removes owned normalization | Component executable grows by 2,032 bytes |
| SSE capacity reservation | Fewer output growth steps in selected cases | Can retain more capacity and lacks a general win |
| Compact Body enum | `Body` shrinks from 40 to 32 bytes | Streaming cases slow by roughly 5–10% in repeated measurements |
| Compact Body with early-success return | Cleaner success/error convergence | Still grows minimal hello by 184 bytes and SSE by 160 bytes |

Production Body, H1 serialization, and HPACK lookup code remain at the original
implementation. Added functional tests cover all 900 supported status values
for both HTTP/1.0 and HTTP/1.1 and the static HPACK table's name/index invariants.
The benchmark probes and local rejected snapshots preserve the investigation.

## Build provenance and security baseline

Initial source snapshots exposed a Cargo timestamp-cache problem: older restored
files at the same guest path could reuse stale objects. Those initial builds
are disqualified. `build-frozen.py` cleans every local package, touches local
Rust sources using the guest clock, serializes target-volume writers, and checks
source-content hashes before and after building. A repaired baseline rebuild
produced identical executable hashes. Final timing uses later clean builds with
recorded source and binary identities.

The common baseline patches rustls-webpki from 0.103.15 to 0.103.16 for
[RUSTSEC-2026-0382](https://rustsec.org/advisories/RUSTSEC-2026-0382.html).
Product and vendored-H3 lockfiles receive that patch. The plaintext-only Actix
fixture disables unused default features, removing its unused vulnerable h2
0.3.27 dependency. Benchmark pruning is not counted as a product-size gain.
These dependency corrections are security maintenance, not measured performance
wins against the former dependency versions.

## Measurement limits

The Docker VM exposes three virtual CPUs and about 3.9 GiB of memory. Existing
user containers remain running. The server uses CPU 0 and clients use CPUs 1
and 2. Repeated alternating trials reduce ordering bias but cannot remove all
host interference. No result establishes a universal absence of regressions,
production acceptance, or that Maximus is the fastest available framework.

Hyper is a lower-level reference. Its SSE fixture returns pre-encoded bytes;
Axum uses its event API. Axum rejects CR data and omits the empty-data component
case, so unsupported component cells are excluded explicitly. The gzip fixture
uses Maximus middleware and Tower's public compression API for Hyper and Axum,
requesting numerical level 1. Backend, synchronous-versus-streamed compression,
header policy, and middleware work can differ. Decompressed payload and gzip
encoding are checked; compressed bytes need not match.

## Correctness and profiling

The [verification record](2026-10-21-verification.json) preserves test summaries
and log hashes. The host check includes formatting, Clippy with warnings
denied, workspace tests, no-default-feature tests, feature combinations, fuzz
target compilation, and documentation with warnings denied. Linux tests and
independent clients pass H1, TLS, H2, H3, SSE, and WebSockets. Interoperability
also covers 16 concurrent uploads and 2,062 wire mutations and disconnects.
Vendored-H3 upstream tests and downstream compilation pass. Product, benchmark,
and vendored lockfile vulnerability audits pass.

Four final fuzz targets run for 61 seconds each without a crash: request
8,583,878 executions, HTTP/1 1,076,742, HPACK 1,008,885, and HTTP/2 1,525,926.
These are bounded crash-finding runs, not proof that no bug exists. SSE and
qvalue exhaustive tests cover the newly changed encoding and negotiation paths.

Response-checked SSE software CPU-clock profiles use the final frozen binaries:
1,938 baseline samples and 1,926 candidate samples over 20 seconds. About 58%
of folded profile weight remains unresolved, largely with restricted kernel
symbols. The PMU reports cycles and instructions as unsupported. No hardware
counter result or precise hotspot conclusion is inferred. The local raw
profiles and flamegraphs remain under `artifacts/performance-2026-10-21/`.
`profile-frozen.sh` reproduces the private-network sampling procedure.

## Reproduction

Use Linux Docker with the recorded Rust image, release thin LTO, and one codegen
unit. The [build manifests](2026-10-21-builds.json) preserve the commands, image
identity, every source fingerprint, and executable hashes. Competitors resolve
to Hyper 1.12.0, Axum 0.8.9, and Tower HTTP 0.7.1 in the isolated lockfile.

The local frozen exports are
`artifacts/performance-2026-10-21/final-baseline-source` and
`artifacts/performance-2026-10-21/final-candidate-source`. To recreate the
baseline elsewhere, export the original commit, apply the common lockfile
corrections, and copy the final isolated benchmark fixtures and lockfile into
both exports. Apply only the SSE and compression source changes to the candidate.
The supplementary metadata/retry exports are `fields-baseline-source` and
`fields-candidate-source` in the same local artifact directory. Set
`BASELINE_FIELDS_SOURCE` and `CANDIDATE_FIELDS_SOURCE` to those exports.
Use new names and output directories; the builders refuse to overwrite manifests.

```sh
python3 benches/build-frozen.py \
  --source "$BASELINE_SOURCE" --name rerun-baseline --out artifacts/rerun-builds
python3 benches/build-frozen.py \
  --source "$CANDIDATE_SOURCE" --name rerun-candidate --out artifacts/rerun-builds
python3 benches/build-applications.py \
  --source "$BASELINE_SOURCE" --name rerun-baseline --out artifacts/rerun-size
python3 benches/build-applications.py \
  --source "$CANDIDATE_SOURCE" --name rerun-candidate --out artifacts/rerun-size
python3 benches/build-frozen.py \
  --source "$BASELINE_FIELDS_SOURCE" --name rerun-fields-baseline \
  --bins event-fields events-allocations --out artifacts/rerun-builds
python3 benches/build-frozen.py \
  --source "$CANDIDATE_FIELDS_SOURCE" --name rerun-fields-candidate \
  --bins event-fields events-allocations --out artifacts/rerun-builds
python3 benches/compare-final.py \
  --baseline /target/2026-10-21/rerun-baseline \
  --candidate /target/2026-10-21/rerun-candidate \
  --label rerun --duration 8s --repeats 3
python3 benches/report-final.py \
  --campaign artifacts/bench/rerun-campaign.json \
  --output artifacts/bench/rerun-report.json
```

Run components by executing the frozen `events`, `negotiation`, `bodies`, and
`event-fields` binaries on CPU 0 in alternating baseline/candidate order for nine
repetitions. `events-allocations` and `allocations` are separate instrumentation
runs. `analyze-hotpaths.py` contains the paired-ratio summary functions. Raw
observations are retained locally under `artifacts/performance-2026-10-21/` and
`artifacts/bench/`. Never run compilation, fuzzing, profiles, or another timing
campaign alongside the measurements.

The socket reporter requires all planned cases, repetitions, successful trials,
and unique baseline/candidate response pairs. Interrupted or failed campaigns
preserve their raw trials and diagnostics but produce no qualification statistics.
`--allow-incomplete` marks live inspection as unqualified. Same-binary controls
use the candidate directory for both variants, `--no-peers`, and selected cases.
Six reporting regression tests exercise complete, failed, interrupted, duplicate,
missing-response, and symmetric missing-path inputs. The runner and reporter
share the case contract in `workloads.py`.
