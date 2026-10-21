# Borrowed absolute H2 timer investigation

**Status: socket campaign ABORTED; performance remains UNQUALIFIED.** Severe VM
control-plane stalls interrupted the five-second Hyper/Axum comparison.
Completed correctness, footprint, and allocation checks remain valid within
their recorded scope. No HTTP throughput, tail-latency, or strict
non-regression acceptance is claimed.

The candidate preserves borrowed error bodies, uses optional completion state
for empty-framed H1 requests, and closes rejected CONNECT connections. Its
additional H2 change pins `compio::time::sleep_until(deadline)` directly and
borrows the pinned timer through `timer.as_mut().fuse()` in both selection
branches. Router and worker implementations match baseline. No dependency,
public API, configuration, or unsafe code is added.

The absolute deadline avoids the relative-duration reconstruction described in
the [research notes](../../docs/performance-research-2026-10-20.md). The earlier
owned-Fuse variant was rejected for code growth. Its standalone
[mechanism probe](absolute-timer-component.md) does not measure this borrowed
representation. The first borrowed probe overlaps a hashing helper at startup
and remains exploratory. A separate
[clean borrowed mechanism probe](borrowed-timer-component.md) records the
borrowed form directly, without qualifying HTTP socket performance.

## Frozen identities

The baseline product commit is `b5bb664bd6faf9b417cd8af5235ec12e7f0037de`.
Frozen source manifests and completed build logs identify the exact comparison:

| Artifact | SHA-256 |
| --- | --- |
| Baseline comparison executable | `be05e0aea314cfdda839441c4c3c923e4cae9c8100f4836f382d7ca1e703e8f2` |
| Candidate comparison executable | `b3e826754d19d7df1d18fc952e93db2a58e5992d9b043c08f7cbb2523ca86f61` |
| Candidate allocation executable | `669316988f4b65f9b6311713158a94bb7f785bd94a0f9b1bfb03b0cca4cac056` |
| Candidate component executable | `586b1964aca7faea96fc567fb8047caf07533b3b8fdd3879bbe656ca5a43b379` |
| Baseline component executable | `a923b99ed6c7dd7da9675eb2aa19eef1d1f97aea102d1b2491cb411e18afc548` |
| `core-h2-baseline-manifest.json` | `f1ec4b5fd73336f710eca672f134783d76796a6384d5ed7cd008cd1fca01cf16` |
| `absolute-timer-borrowed-manifest.json` | `ab9a4d02b316ec3ea6b95a4c03f75f745cc0f82559d6b22072d34bb6024e3270` |

The frozen roots are `core-h2-baseline-source` and
`absolute-timer-borrowed-source`. The respective completed logs are
`core-h2-baseline-build.log` and `borrowed-timer-build.log`. These files remain
under `artifacts/performance-2026-10-20/`. The analyzer requires the manifest
hashes of the frozen H2 files to match their contents, and requires exactly the
reviewed timer construction and two select substitutions against baseline.

## Fresh allocation observations

Five alternating baseline/candidate pairs emit eighteen rows each. All ten
fresh processes exit zero. The complete connection probes include client and
runtime allocations. Calls include reallocations; requested bytes measure
allocation traffic, rather than retained heap or RSS. Values below are medians
of the five runs.

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
| Complete H1 keepalive | 15.0051 → 14.0069 | 2,628.3256 → 2,605.1464 |
| H2 invalid first frame | 33.4520 → 33.4260 | 14,453.5120 → 14,441.2560 |
| H2 large static | 34.8618 → 34.9151 | 7,915.9808 → 7,940.2856 |
| H2 large owned | 35.9058 → 35.8998 | 40,704.0448 → 40,701.3088 |
| Complete H2 static | 30.0070 → 30.0308 | 6,898.1920 → 6,909.0448 |
| Complete H2 owned | 31.0064 → 31.0194 | 6,910.9184 → 6,916.7178 |

The H1 and static-error differences agree with the removed completion cell and
payload copy. H2 large-static and complete-keepalive medians increase slightly;
these observations remain adverse evidence. No H2 allocation saving is claimed.
Raw files are `borrowed-timer-allocations-{baseline,candidate}-{0..4}.csv`.
Their allocator-instrumented timings do not establish speedups.

Fourteen component observations are explicitly reused from
`tuple-components-{baseline,candidate}-{0..6}.csv`: both component executable
hashes match their earlier counterparts byte for byte. H1 completion and H2
timer code do not contribute to that component executable. The combined
`borrowed-timer-fixture-outcomes.json` identifies all fourteen reused and ten
fresh process outcomes; all twenty-four exit zero. The reused timing medians
are recorded for completeness, without assigning them to the timer change:

| Component | Baseline ns/operation | Candidate ns/operation |
| --- | ---: | ---: |
| Static text | 35.212 | 33.153 |
| Static error | 38.194 | 32.061 |
| Owned error | 35.573 | 35.705 |
| Literal route | 105.270 | 108.638 |
| Captured route | 154.272 | 154.025 |
| Method not allowed | 114.692 | 112.934 |
| Register 64 mixed routes | 22,964.632 | 22,745.415 |
| Register 64 literal routes | 16,265.963 | 16,300.755 |

The static-error case is faster in all seven reused pairs. Owned error, literal
routing, and literal registration retain adverse median differences; unchanged
router code is not credited with a gain.

## Clean borrowed mechanism probe

A separate clean, seven-pair standalone probe records median relative/borrowed
absolute poll-and-drop times of 55.262 → 25.289 ns/operation; all seven pairs
favor the absolute form. It uses the actual runtime poll context, two million
operations per mode per round, alternating order, and expired/future controls.
Its stored-F sizes are 120 → 88 bytes, excluding the borrowed Fuse wrapper and
enclosing async frame. The separate measured HTTP serve-future deltas below
are 48 bytes each. [Exact probe evidence](borrowed-timer-component.json)
records hashes, locked dependency parity, raw rows, command provenance and
limits. This is mechanism evidence, with socket performance still unqualified.

## Footprint and correctness

Matched release observations use the same locked compiler, profile, and
conformance features (`tls,http2`). Independent conformance observations in
`absolute-timer-borrowed-footprint-layout.log` give:

| Artifact measure | Baseline | Borrowed candidate |
| --- | ---: | ---: |
| Comparison executable text bytes | 7,136,205 | 7,122,685 |
| Conformance text bytes | 2,151,396 | 2,146,608 |
| Conformance data bytes | 59,344 | 59,344 |
| Conformance BSS bytes | 2,560 | 2,560 |
| Conformance file bytes | 2,953,672 | 2,953,664 |
| Stripped conformance file bytes | 2,233,184 | 2,233,184 |

The candidate conformance executable hash is
`704b3695c79b20519a2008b59b564a3e0d598025ab48976b952c99a29d3e0263`.
Measured compat/native serve futures are 1,528/2,936 bytes on that target,
versus baseline 1,576/2,984 bytes: each decreases by 48 bytes. These layout
observations do not imply a measured socket speedup. The rejected owned
timer artifact had 2,155,420 conformance text bytes, 4,024 above baseline; it is
not pooled with this borrowed variant.

Candidate checks record 176 passing host test executions and 151 passing Linux
executions with one BPF-related skip. Independent clients pass plain H1,
TLS 1.3 H1/H2, WebSocket, and verified HTTP/3 checks, plus 2,062 raw-wire
mutation/disconnect cases and post-stress health. The candidate H2 sanitizer
fuzzer completes 1,335,300 executions in 61 seconds without failure. Logs are
`borrowed-timer-check.log`, `borrowed-timer-linux.log`,
`borrowed-timer-interop.log`, and `borrowed-timer-fuzz-http2.log`.

## Aborted partial socket campaign

The fixed plan was 84 engine observations across seven existing cases, three
repeats, and five-second measured invocations. H1 used `borrowed-timer-wrk`;
the planned `borrowed-timer-tls` cohort never launched. The maximum recorded
CPU snapshot interval reached **45.869681040989235 seconds** for the dynamic
error candidate despite its five-second client setting. This interval includes
measurement/control overhead and is not the configured client duration.

The parent campaign received SIGINT and exited 130. Root reports removing its
sole orphan server, `maximus-bench-55518-2`, after verifying the command was
Axum from the borrowed candidate executable. The stop was prompted by the
control-plane stalls, rather than source acceptance or selection of favorable
observations. Disk/cache/resource pressure remains under investigation; no
specific cause is established here.

[Partial evidence](borrowed-timer-aborted.json) and
[all recorded rows](borrowed-timer-aborted-trials.csv) preserve nineteen engine
observations and 21,654,325 checked responses. Nine invocations completed with
passing outcomes, supplying seventeen rows. The interrupted tenth invocation
has two recorded rows (Maximus and Hyper), no completed invocation outcome,
and no Axum result row; its existing Axum log is preserved. No TLS observation
exists. The full 84-observation analyzer was not executed.

The separate [offline partial reconciler](../analyze-borrowed-timer-aborted.py)
checks every recorded CSV count and primary latency/RPS metric against its raw
JSON, compares all nineteen rows to the preserved abort snapshot, and hashes
all existing cohort files. It records the missing/partial scope explicitly.
It does not qualify the incomplete campaign. Summary ranges across all engines
and cases describe only the recorded observations:

| Measure | Recorded minimum | Recorded maximum |
| --- | ---: | ---: |
| RPS | 93,438.818151 | 350,852.319582 |
| CPU microseconds/request | 2.669608 | 4.626065 |
| p99 milliseconds | 2.094 | 152.252 |
| p99.9 milliseconds | 7.306 | 222.755 |
| CPU snapshot interval seconds | 5.438416291959584 | 45.869681040989235 |

All available Maximus pairs are retained below. Values are candidate changes
relative to baseline, in percent. Each has only the first repeat. The static
plaintext pair includes a row from the interrupted candidate invocation.

| Case | RPS | CPU/request | p99 | p99.9 |
| --- | ---: | ---: | ---: | ---: |
| Dynamic plaintext | −29.36% | +8.58% | +127.57% | +136.45% |
| Captured route | −10.10% | +5.47% | −64.01% | −81.28% |
| Dynamic error | −28.77% | +4.62% | +5.66% | −7.52% |
| Dynamic 1 KiB echo | +33.04% | −12.38% | −50.33% | −59.19% |
| Static plaintext, partial invocation | −1.31% | −1.22% | +79.94% | +223.03% |

Adverse rows are preserved. The observed stalls neither prove a source-level
regression nor explain adverse observations in earlier campaigns. No samples
are pooled with previous completed cohorts. Raw server CPU snapshots were not
retained, so this partial reconciliation does not independently reconstruct
the CSV's server CPU arithmetic.

The shared three-CPU Linux ARM VM and short repeated observations limit
performance inference. Earlier [H1 findings](h1-empty.md),
[rejected persistent-H2 findings](core-h2.md), and the separate
[attribution diagnostic](isolate-h2.md) remain intact. Neither those campaigns
nor these completed allocation checks qualify this borrowed-timer socket
candidate. Root reports independent agent static source review found no
actionable findings; that result is limited to source correctness and supplies
no performance qualification. The clean standalone timer mechanism result
does not change the aborted campaign's unqualified socket status.

A subsequent [same-binary H2 control](post-cache-aa-h2.md) completes all eighteen
observations with passing validation and shorter CPU snapshot intervals. Its
unchanged executable still shows large throughput and tail variation, so host
stability remains unestablished. It is kept separate and establishes neither
the effect of cache cleanup nor this candidate's socket performance.

The subsequent [H1 memory-transport control](memory-http1.md) validates all 168
rows and 84 matched engine/case pairs. Its timing evidence remains mixed,
including slower Maximus samples and large variations in unchanged comparator
controls. It exercises H1 shared futures protocol work, allocation and output
retention; it supplies no H2 or native TCP I/O result and leaves strict
performance qualification unresolved.
