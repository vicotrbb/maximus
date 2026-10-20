# HTTP/2 static fields and DATA ownership

This change retains three private HTTP/2 optimizations against
`6f0ebd61a7c5664bb1f70104dd20a67bc864e9c6`:

- Borrow indexed HPACK static-table names/values from their compiled bytes.
  Literal strings and dynamic references remain owned; decoded-list limits are
  checked before copying or insertion. A named 48-byte `Field` replaces the
  previous 48-byte tuple.
- Move DATA `Bytes` out of a response `Frame` rather than cloning its owner.
- Copy the pending prefix synchronously into the output batch, advance the
  buffer, and explicitly release an exhausted owner. This avoids a temporary
  `Bytes` owner used solely for copying.

The original core path validator and HTTP/1 runtime remain unchanged. No public
API, dependency, unsafe code, persistent connection field, protocol setting or
byte budget is added. These changes were developed against this implementation;
no competitor implementation was imported.

**Decision: retain the three HTTP/2 changes for their measured allocation and
release-footprint benefits after correctness qualification.** No reproducible
source-level retention regression was identified. Socket throughput remains
small and mixed, and large-response memory measurements retain uncertainty.
This result does not establish an absolute no-regression guarantee or universal
performance superiority.

## Ownership and protocol contracts

Borrowed fields originate only from the compiled
[RFC 7541 static table](https://www.rfc-editor.org/rfc/rfc7541.html#section-2.3.1).
Dynamic results survive subsequent eviction because they are owned. Static
borrowing still counts every decoded byte and the per-field overhead against
the configured header-list budget. Tests exercise those ownership cases,
including eviction later in the same header block.

The send changes preserve content-length checks, connection/stream flow control,
END_STREAM, response trailers and empty-frame wakeups. Copying finishes before
advancing the source; setting an exhausted buffer to `Bytes::new()` prevents
retaining its backing storage. A 1 MiB small-window wire test verifies a
position-dependent byte pattern through repeated splits and window updates.
These contracts follow [RFC 9113 message framing](https://www.rfc-editor.org/rfc/rfc9113.html#section-8.1).

## Why the path change was removed

The [preceding four-change experiment](static-hpack-and-response-ownership.md)
records the full candidate, 84 competitor-comparison observations and 42 longer
confirmation observations. The confirmation showed lower plain-echo throughput
in six of seven pairs, higher CPU/request and higher endpoint RSS. That
combination failed to demonstrate the requested no-regression outcome.

A separate rotated-triple ablation compared original baseline, that candidate,
and this HTTP/2-only build. Seven 20-second observations per build completed
23,661,554 validated responses with zero recorded failures. Median results:

| Metric, plain 1 KiB echo | Baseline | Four-change candidate | HTTP/2-only |
|---|---:|---:|---:|
| Completed requests/s | 53,305 | 57,094 | 58,180 |
| CPU microseconds/request | 8.580 | 8.008 | 8.103 |
| p99 milliseconds | 6.184 | 5.721 | 5.548 |
| Observed peak RSS KiB | 7,048 | 7,136 | 7,040 |

HTTP/2-only throughput is lower in three of seven baseline pairs; the
four-change candidate is lower in two. This cohort does not reproduce the
previous plain-echo throughput ordering. It does not prove that the path loop
caused the earlier result. Both host scheduling and compiler/layout effects
remain possible. The order patterns repeat modulo three; the four-change
candidate is the middle variant in five of seven triples, another limitation
for causal inference. We remove the marginal path change rather than use the
favorable new cohort to erase the earlier unfavorable evidence.

[All ablation observations](static-hpack-and-response-ownership-path-ablation.csv)
and [complete metric arrays](static-hpack-and-response-ownership-path-ablation.json)
retain the individual samples and ordering.

## Evidence carried by the unchanged HTTP/2 components

The HTTP/2 implementation is byte-identical to the HTTP/2 files in the preceding
candidate. Its focused, pinned seven-pair probes include output creation and
destruction: static decoding median 72 to 28 ns, indexed-name request 92 to
37 ns, literal decoding 33 to 33 ns, dynamic insertion/reference 89 to 86 ns
(overlapping samples), budget rejection 4 to 4 ns. These are component costs,
not application-throughput predictions. Field 48 bytes, connection 280 bytes,
and the probed serve future 1,576 bytes are unchanged.

The prior candidate's tests include independent HTTP/1/TLS/HTTP2/HTTP3,
WebSocket, streaming and upload clients, 2,062 raw-wire stress cases, downstream
compilation, and 14,858,775 sanitizer-fuzz executions across four targets.
Fresh final-combination checks and measurements are recorded below.
Prior results are not relabeled as new binary measurements.

## Final allocations and footprint

Five alternating matched pairs reproduce the following allocation medians:

| Completed response fixture | Baseline allocations/request | Final allocations/request |
|---|---:|---:|
| Small static string | 37.0107 | 30.0086 |
| Small owned string | 39.0137 | 31.0056 |
| 32 KiB static bytes | 41.9353 | 34.9346 |
| 32 KiB owned bytes | 43.9476 | 35.9541 |

The reduction is about 17–21% across these fixtures. All five final runs pass
the relative owned/static gates; the baseline fails those gates as expected,
after preserving every fixture's measurements. The invalid-first-frame control
retains the original GOAWAY runtime; its process-wide accounting includes
asynchronous teardown noise.

Matched Rust 1.96 Linux ARM64 ThinLTO builds use one codegen unit, unchanged
lockfiles and the same `/work` source path. Values below describe unstripped ELF
files and GNU `size` text totals (which include read-only code/data sections).
Data and BSS totals are unchanged.

| Artifact | Baseline file bytes | Final file bytes | Baseline text bytes | Final text bytes |
|---|---:|---:|---:|---:|
| Minimal example | 1,279,584 | 1,279,584 | 827,893 | 827,893 |
| TLS/HTTP2 conformance | 2,953,672 | 2,953,672 | 2,155,256 | 2,151,396 |
| Unified comparison | 9,594,080 | 9,528,400 | 6,988,089 | 6,980,069 |

The unified comparison contains every fixture engine; its size is not a
per-framework ranking. Savings vary with application, features and compiler.

## Fresh final-combination correctness

`scripts/check.sh` completed 166 passing test executions across feature/config
variants, formatting, Clippy with warnings denied, fuzz-target compilation and
documentation. Linux all-features tests passed 144 executions with one
privileged eBPF test skipped. Independent clients passed HTTP/1, verified TLS
1.3, HTTP/2, HTTP/3 QUIC, WebSocket text/binary, JSON, multipart, SSE, gzip,
CORS, HEAD and concurrent uploads. Wire stress passed 2,062 mutations/disconnects,
16 concurrent clients and post-stress health. The downstream consumer compiled.

Fresh plaintext/TLS h2spec policy gates passed. Each legacy raw suite is
139/147, with five documented RFC 7540/RFC 9113 policy differences and three
response-closure races. All three closure cases pass held-response reruns.
This is not a raw 147/147 result or full RFC certification.

## Final socket comparison

All 36 observations completed, with 14,050,021 validated responses and zero
recorded request failures, failed checks, dropped iterations or nonzero client
exits. Three repetitions per configuration give these medians:

| TLS HTTP/2 workload | Baseline Maximus RPS | Final Maximus RPS | Hyper RPS | Axum RPS |
|---|---:|---:|---:|---:|
| Plaintext | 60,176 | 60,934 | 60,570 | 59,937 |
| 1 KiB echo | 51,130 | 50,366 | 50,926 | 50,753 |
| 128 KiB echo | 5,178 | 5,205 | 7,379 | 7,314 |

Final throughput changes are +1.3%, -1.5% and +0.5%. CPU/request medians change
from 9.553 to 9.278 microseconds, 12.730 to 12.759, and 175.296 to 173.389.
The small echo difference and overlapping samples do not establish either a
causal slowdown or universal non-regression. This shared-host comparison does
not demonstrate higher application throughput across workloads.

Endpoint RSS medians increase for plaintext and large echo and decrease slightly
for small echo. Large-echo final RSS ranges from 18,064 to 20,804 KiB versus
19,440 to 19,616 for baseline. These unfavorable observations remain in the
[trials](h2-static-ownership-trials.csv) and
[full metric arrays and identities](h2-static-ownership-evidence.json).
Unchanged field sizes do not establish unchanged process memory. The separate
fixed-count, open-connection memory diagnosis below narrows that uncertainty.

Large-body Maximus remains behind Hyper and Axum in this fixture. The TLS Hyper
fixture uses the existing shared-router adapter, including cloning; it is not a
minimal raw-Hyper lower bound. Client CPU limits and shared scheduling also
constrain capacity conclusions. CPU accounting includes snapshots around
client startup/teardown, rather than isolated handler CPU. Reported peak RSS is
the maximum sampled value, not a continuous peak.

## Fixed-count memory diagnosis

Three alternating pairs per workload compare fresh processes after identical
completed request counts, with exactly 64 established connections verified at
each comparison point. Each connection completes 100 warmups followed by
10,000 plaintext requests or 1,000 large echoes. Eight-second holds after warmup
and midpoint let the observer sample equal volumes. After completion, the client
keeps connections open for 12 seconds; samples at completion and two, four and
six seconds later separate quiescent retention from endpoint activity.

All 12 runs completed 4,300,800 validated responses with zero request/check
failures. `/proc/1/smaps_rollup` gives these final quiescent medians in KiB:

| Workload and metric | Baseline | Final |
|---|---:|---:|
| Plaintext RSS | 8,008 | 7,976 |
| Plaintext anonymous | 3,108 | 3,108 |
| Large echo RSS | 19,492 | 19,692 |
| Large echo anonymous | 14,688 | 14,952 |

Large-echo paired anonymous differences are +120, -224 and +920 KiB. File-backed
residency also varies substantially. The final 264 KiB anonymous median increase
is about 1.8%; it is not hidden by the equal plaintext medians. Quiescent values
remain stable through the holds. These observations do not establish a
consistent source-level retention regression or unchanged memory, and they do
not replace the earlier unfavorable endpoint RSS observations. Three pairs on
a shared VM leave uncertainty about small memory differences.

[Memory observations](h2-static-ownership-memory.csv) and
[phase samples and methodology](h2-static-ownership-memory.json) preserve both
anonymous and file-backed categories. Periodic samples during traffic remain in
the raw archive; those are not equal-count comparisons or continuous peaks.
The observer's first startup attempt missed the server's stderr readiness line;
no traffic or memory result was collected. That diagnostic failure is preserved
separately and excluded from these 12 successful runs.

## Exact final-binary profile

A separate CPU-clock profile of the frozen final binary contains 2,679 samples
at 199 Hz, zero reported lost samples, and 76,739 validated 128 KiB echo responses.
The recording lasts 20 seconds around a 15-second verified-TLS HTTP/2 client run.
Its throughput is excluded from socket measurements. AES-GCM encrypt/decrypt
accounts for about 21% of samples; timer polling and clock access remain visible.
The largest libc address is unresolved and kernel symbols are restricted, so
neither is assigned an invented function name. This is sampled CPU attribution,
not hardware-cycle accounting or proof of a single cause for the competitor gap.
The [profile summary](h2-static-ownership-profile.json) records its binary hash,
sample count and leading entries; raw stacks and the flamegraph remain local.

## Reproduction and limits

Fresh comparison runs use the frozen original baseline and HTTP/2-only binaries,
Hyper 1.12.0 and Axum 0.8.9, verified TLS 1.3, 64 virtual users, a three-second
warmup and ten-second measurements. The shared Linux ARM64 Docker VM has three
CPUs: server CPU 0, client CPUs 1 and 2. Each server has a private network
namespace and its client shares that namespace, preserving loopback without a
published host port. Other user services remain running. No compiler, fuzzer or
profiler runs concurrently with measurements.

Allocation counters are process-wide requested allocations, not live heap or
RSS. Successful fixtures use 10,000 requests after 100 warmups. They use `/`,
static method/scheme/path indices and a literal authority with indexed name;
other header mixes have different savings. Owned/static response conversions
and headers match. Large fixtures use 32 KiB `Bytes` and frame splitting. The
relative gates execute after every fixture has emitted its observations.
Instrumented wall times are not used as production speedup evidence.

Raw logs, frozen binaries, source snapshots, manifests, profiler stacks,
rejected experiments and drivers remain in ignored
`artifacts/goal-performance-2026-10-19/`. Compact reports and observations are
retained here. No measurement establishes universal performance superiority or
absence of every possible regression.
