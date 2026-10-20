# Static HPACK fields and response ownership

Four local changes reduce ownership work against baseline commit
`6f0ebd61a7c5664bb1f70104dd20a67bc864e9c6`:

- Decode indexed static HPACK names and values as borrowed `Cow` slices. Literal strings
  and dynamic-table data stay owned. A private named `Field` keeps name/value
  access explicit and occupies the same measured 48 bytes as the old tuple.
- Move response DATA bytes out of their `Frame` instead of cloning them.
- Copy a borrowed pending-data prefix directly into the output batch, then
  advance the buffer. Clear an exhausted `Bytes` immediately so its owner is
  released. The old temporary `Bytes` owner existed only to perform this copy.
- Check decoded path controls and separators in one character traversal.

No public API, dependency, unsafe code, persistent connection field, protocol
setting, or byte budget is added. HTTP/1 reader runtime code is unchanged; its
new CRLF boundary tests remain from a rejected experiment. Changes were written
against this implementation; no competitor implementation was imported.

The selected-source comparison completed 84 measurements and 41,050,736
validated responses with zero recorded failures, failed checks or dropped
iterations. A separate seven-pair confirmation completed another 45,799,892
validated responses. Plain 1 KiB echo was slower in six of seven pairs, with
higher CPU/request and observed RSS. This four-change combination is **not
accepted**. The completed ablation and the decision to restore the original
path validator are recorded in the [HTTP/2-only follow-up](h2-static-ownership.md).
This report remains the record of the four-change combination.

## Ownership and bounds

Only the compiled [RFC 7541 static table](https://www.rfc-editor.org/rfc/rfc7541.html#section-2.3.1)
provides borrowed data. Dynamic references are copied before table mutation or
eviction. Indexed-field and indexed-name lengths are checked before those
copies. Borrowing does not waive decoded header-list accounting. Tests check
borrowed static fields, owned literal/dynamic data, and a dynamic result after
its source entry is evicted later in the same block.

Moving DATA and advancing its suffix preserve content-length checks, flow-control
accounting, END_STREAM decisions, response trailers, and empty-frame wakeups.
The copy completes synchronously before the source buffer is advanced. Merely
advancing to zero length can retain backing storage; explicit clearing preserves
prompt release. Existing exact-window, zero-window, trailer and frame-splitting
tests pass. The small-window test now checks a position-dependent byte pattern
through a 1 MiB response, so repeated prefixes and skipped suffixes are detected.
Message framing remains governed by [RFC 9113 §8.1](https://www.rfc-editor.org/rfc/rfc9113.html#section-8.1).

Path validation preserves the previous control/separator policy, including NUL,
ASCII controls, Unicode C1 controls, slash and backslash. Tests cover encoded
code points through U+009F, representative higher Unicode characters, malformed
escapes and dot segments. The focused path experiment uses this exact predicate;
its samples are recorded in the [earlier investigation](owned-data-and-paths.md).
This strict separator policy is a framework choice, not a claim that
[RFC 3986 §2.2](https://www.rfc-editor.org/rfc/rfc3986.html#section-2.2)
mandates every rejection.

## Allocation experiment

Five alternating baseline/selected pairs use the same stack-buffer wire client
and response conversions. Each successful keepalive fixture measures 10,000
requests after 100 warmups. Small static/owned responses both use the string
response conversion and identical text headers. Large static/owned responses
both use the `Bytes` response conversion and a 32,768-byte payload, forcing frame
splitting while fitting the stream window. Connection credit covers the entire
measurement; timed WINDOW_UPDATEs are unnecessary.

Counters include process-wide server/runtime allocation activity. They measure
requested allocation bytes, not allocator bin sizes, live heap, or RSS. All
response bodies, status, stream IDs and completion are checked. The relative
owned/static allocation gates run after every fixture has emitted its results,
so a failing baseline still preserves all observations. Baseline exits 101 as
expected; selected exits zero in all five pairs. There is no platform-specific
absolute total-allocation gate.

Median allocations per completed request across the five pairs:

| Response fixture | Baseline | Selected |
|---|---:|---:|
| 13-byte static string | 37.0233 | 30.0102 |
| 13-byte owned string | 39.0295 | 31.0145 |
| 32 KiB static bytes | 41.8846 | 34.8708 |
| 32 KiB owned bytes | 43.9285 | 35.9024 |

These requests use `/` and a fixed literal authority, with static indexed
method/scheme/path fields and an indexed authority name. Other header mixes,
including dynamically indexed application paths, have different savings; these
numbers are not a universal seven-allocation reduction for every HTTP/2 request.

The invalid-first-frame fixture separately records process-wide rejection-path
activity. It verifies the GOAWAY payload and EOF. EOF proves peer closure, not
completion of every asynchronous cleanup allocation. Runtime accounting noise
therefore remains in this control. The rejected stack-array GOAWAY experiment
is not part of the selected runtime.

Allocation-instrumented wall times are not used to claim production speedups:
the counters themselves add work to allocation calls. Uninstrumented focused
probes and socket runs provide separate timing evidence.

## Focused decoding and live state

Seven alternating, CPU-pinned repetitions use 10,000,000 decodes per input per
repetition. Inputs and results are black-boxed; probes include output allocation
and destruction rather than isolated table lookup. Median ns/op:

| Header block | Baseline | Selected |
|---|---:|---:|
| Static indexed fields | 72 | 28 |
| Request with indexed names | 92 | 37 |
| Literal field | 33 | 33 |
| Dynamic insertion/reference | 89 | 86 |
| Header-budget rejection | 4 | 4 |

The common static cases improve strongly. Literal and error medians are
unchanged; dynamic samples overlap and do not establish a meaningful gain.
These are local component costs on a shared VM, not throughput predictions.

Measured layouts are unchanged: decoded field 48 bytes, connection 280 bytes,
and the probed `serve_futures` future 1,576 bytes. Dynamic-table entry storage is
unchanged. No additional owner or buffer survives a suspension point.

## Release footprint

Matched Linux ARM64 builds use Rust 1.96, ThinLTO, one codegen unit, unchanged
lockfiles, and the same `/work` source path. Values below are complete unstripped
ELF files and GNU `size` text-column totals; the text column includes read-only
code/data sections. Data and BSS totals are unchanged.

| Artifact | Baseline file bytes | Selected file bytes | Baseline text bytes | Selected text bytes |
|---|---:|---:|---:|---:|
| Minimal example | 1,279,584 | 1,279,584 | 827,893 | 827,725 |
| TLS/HTTP2 conformance server | 2,953,672 | 2,953,664 | 2,155,256 | 2,147,148 |
| Unified comparison server | 9,594,080 | 9,528,384 | 6,988,089 | 6,979,917 |

The comparison executable contains all fixture engines; its size is not a
per-framework size ranking. These exact artifacts do not promise identical
byte savings for every application, compiler or feature set.

## Qualification

The selected Rust source passed `scripts/check.sh`: 166 test executions across
feature/configuration variants, formatting, warnings-denied Clippy, fuzz-target
compilation and documentation. Linux all-features tests passed 144 executions
with one privileged eBPF test skipped. Independent clients passed HTTP/1,
verified TLS 1.3, HTTP/2, HTTP/3 QUIC, WebSocket text/binary, JSON, multipart, SSE,
gzip, CORS, HEAD and concurrent uploads. Wire stress passed 2,062
mutations/disconnects, concurrent clients and a post-stress health check. The
downstream consumer check passed.

Fresh sanitizer fuzzing completed without a reported failure: request parsing
10,729,224 executions, HTTP/1 1,239,328, HTTP/2 1,596,849, and HPACK 1,293,374,
61 seconds per target. These bounded runs do not exhaust the input space.

Plaintext and TLS h2spec policy gates passed. Each raw legacy suite is 139/147:
five documented RFC 7540/RFC 9113 policy differences and three response-closure
races, with all three passing held-response reruns. This is not 147/147 raw
success or a full RFC certification claim.

## Socket methodology and startup isolation

The campaign alternates frozen baseline and selected Maximus binaries and
rotates Hyper/Axum order. Each workload has three repetitions, a separate
three-second warmup, a ten-second measurement, and 64 virtual users. The server
uses CPU 0 and k6 CPUs 1 and 2 in a shared three-CPU Linux ARM64 Docker VM.
Other user services remain running, so this is not an exclusive-host capacity
measurement. The fixtures pin Hyper 1.12.0 and Axum 0.8.9.

The first selected campaign exited before any measured row: another local
project had started using host port 8080. Running the original baseline directly
reproduced `AddrInUse`. The runner now gives each server a private network
namespace and joins its client to that namespace. Traffic remains loopback,
without Docker bridge forwarding or a published host port. The environment
records the mode. Eleven smoke cases covered plain Maximus/Hyper/Axum/Actix/Epoll
and TLS HTTP/1/HTTP2 Maximus/Hyper/Axum, with 132,162 validated responses and zero
errors. This cohort must not be pooled with older host-network measurements.

CPU accounting spans the runner's snapshots around client startup and teardown;
it is not isolated handler CPU. RSS values are observations, not continuous
peaks. Client limits and shared-host scheduling constrain throughput inference.
The TLS Hyper fixture uses the existing shared router adapter, including router
cloning, and is not a minimal raw-Hyper lower bound.

## Socket observations

Median completed requests/s from three repetitions per configuration:

| Protocol/workload | Baseline Maximus | Selected Maximus | Hyper | Axum |
|---|---:|---:|---:|---:|
| plain /plaintext (0 body bytes) | 65,420 | 64,212 | 58,060 | 36,054 |
| plain /echo (1024 body bytes) | 56,422 | 55,042 | 56,974 | 59,073 |
| plain /users/42 (0 body bytes) | 64,510 | 65,234 | 64,066 | 64,877 |
| plain /json (0 body bytes) | 64,311 | 64,998 | 64,543 | 62,018 |
| h2 /plaintext (0 body bytes) | 63,928 | 64,878 | 53,364 | 54,680 |
| h2 /echo (1024 body bytes) | 51,333 | 48,985 | 48,376 | 51,655 |
| h2 /echo (131072 body bytes) | 5,150 | 5,157 | 7,322 | 7,858 |

The selected medians change by −1.8% for plain plaintext, −2.4% for plain
1 KiB echo, and −4.6% for HTTP/2 1 KiB echo. The other four throughput medians
increase by 0.1–1.5%. HTTP/2 large-echo p99.9 is 48.334 versus 52.934 ms and
observed RSS 19,244 versus 19,760 KiB. Those unfavorable observations remain in
the data. Baseline HTTP/2 plaintext spans 32,273–65,935 requests/s; sample ranges
overlap widely. Three repetitions on this shared VM cannot resolve the smaller
differences or establish universal non-regression. Large HTTP/2 echo also remains
slower than both comparison fixtures.

[Individual trials](static-hpack-and-response-ownership-trials.csv),
[allocation observations](static-hpack-and-response-ownership-allocations.csv),
and [source identities and full metric arrays](static-hpack-and-response-ownership-evidence.json)
preserve the lower results as well as the higher ones.

## Longer paired confirmation

Seven alternating pairs use 20-second measurements for the three initially
lower-throughput workloads. All 42 observations completed, with 45,799,892
validated responses and zero recorded failures, failed checks or drops.

| Workload | Baseline RPS | Selected RPS | Selected lower-throughput pairs |
|---|---:|---:|---:|
| Plain plaintext | 64,357 | 64,732 | 2/7 |
| Plain 1 KiB echo | 60,252 | 58,004 | 6/7 |
| TLS HTTP/2 1 KiB echo | 53,530 | 53,796 | 3/7 |

Plain echo median CPU/request rises from 7.544 to 7.974 microseconds and
observed RSS from 6,356 to 7,344 KiB. Selected RSS is higher in all seven pairs.
Despite shared-host variability, these adverse observations prevent accepting
the combination under the requested no-regression rule. This does not prove
that the path predicate causes a 3.7% throughput regression: environmental
fluctuations are much larger, CPU/request shares its request-count denominator,
and endpoint RSS does not isolate live heap. The HTTP/2 throughput
difference from the shorter cohort did not reproduce.

[Confirmation trials](static-hpack-and-response-ownership-confirmation.csv)
and [paired metric arrays](static-hpack-and-response-ownership-confirmation.json)
remain separate from the shorter competitor comparison.

## CPU-clock profiles

Separate post-campaign Linux `perf` recordings compare the frozen baseline,
selected Maximus, and Hyper binaries on verified-TLS HTTP/2 128 KiB echo. The
20-second recordings sampled at 199 Hz during a 15-second client run, with
64 virtual users and the same private-loopback CPU placement. They contain
2,687, 2,676, and 2,582 samples respectively and zero reported lost samples.
Profiled throughput is excluded from the socket campaign.

AES-GCM encrypt/decrypt accounts for about 22% of sampled Maximus CPU and 35%
of Hyper CPU in these recordings. Maximus timer polling/drop and clock access
are also visible. These sample shares identify investigation targets; they do
not assign the throughput gap to a single cause, and percentages across engines
are not absolute CPU-cost comparisons. Kernel symbols are restricted and the
largest libc address is unresolved in the report, so its cost is not given an
invented function name. The profiles do not justify removing timeout policy or
changing cryptographic settings. Inspection of the pinned Compio 0.12.6 source
shows that polling a new sleep inserts a timer and dropping it cancels that
registration. The HTTP/2 event loop constructs a sleep per selection. Any timer
reuse experiment must preserve changing stream, header, settings, idle and drain
deadlines and measure persistent-future size; it is not included in this change. Full stacks, recording diagnostics, build IDs,
verified response summaries and SVG flamegraphs remain in the raw archive.
The [compact profile record](static-hpack-and-response-ownership-profiles.json)
retains binary hashes, sample totals and the leading flat entries.

## Rejected work and evidence boundaries

The [earlier report](owned-data-and-paths.md) preserves the initial two-change
candidate and its unfavorable longer socket confirmation. Its 84 primary and
42 confirmation rows are separate from this selected-source campaign. It also
records rejected HTTP/1 scanners: one grew futures by eight bytes; another
restored layouts but slowed a short fragmented line in all seven pairs.

A compact HPACK Huffman trie reduced each trie node from 24 to 8 bytes but slowed the long
probe by about 12%, so it was rejected. Tuple-Cow and early send combinations
introduced small ELF-file growth despite smaller text totals. A fixed-array
GOAWAY implementation passed tests but increased the standalone text total and
was rejected. The selected named field and linear send retain the ownership
benefits without the observed artifact growth. No GOAWAY runtime change remains.

All raw logs, rejected snapshots, frozen source manifests, patches, profiler data
and experiment drivers remain under ignored
`artifacts/goal-performance-2026-10-19/`. The linked evidence records the release source/binary identities and
individual selected-source trials. The
startup failure and preliminary smoke runs remain separate from primary trials.
