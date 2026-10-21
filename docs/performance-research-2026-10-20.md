# Performance opportunities and protocol constraints

Research date: 2026-10-20. The inspected baseline is
`b5bb664bd6faf9b417cd8af5235ec12e7f0037de`. This note separates source-based
opportunities, measured observations, and experiment dispositions. It makes
no claim that Maximus is the fastest framework.

## Current disposition of this investigation

The user subsequently approved testing locally in Docker with k6. The
[local campaign](../benches/results/local-docker-k6.md) records the original
production-source reference, corrected baseline and three independent candidates
on all seven valid workloads, with Hyper/Axum fixtures and repeated identical
executable controls. Its report records actual completion, raw correctness
audits, allocation diagnostics, linked footprint and all paired observations.
This local authorization supersedes the earlier dedicated-host execution plan;
it does not convert uncertain observations into accepted performance gains.

**No performance optimization evaluated in this investigation is accepted or
retained in root product code.** Borrowed Error conversion, optional H1
completion state, and the borrowed absolute H2 timer match starting revision
`b5bb664bd6faf9b417cd8af5235ec12e7f0037de`.
The archived router, registration, persistent-listener, header-copy and
standalone traversal experiments likewise provide no retained performance
change. Their frozen source identities, raw measurements, favorable results,
adverse results and reports remain preserved.

Root retains the RFC 9931 rejected-CONNECT closure correction and
baseline-compatible functional regression coverage. Inspection of the three
performance source files confirms core/H2 match that starting revision and the H1 diff contains
only the CONNECT correction. Fresh final checks pass: `scripts/check.sh`
records 178 passing test executions, and the Linux suite records 152 passing
executions with one privileged BPF test ignored. Independent H1/TLS/H2/H3 and
WebSocket client checks pass, including 2,062 wire mutations/disconnects,
16 concurrent clients, and post-stress health. The later CONNECT/101 and
nginx metadata controls reproduce their failures before correction and pass
afterward. Exact final source hashes, commands, log hashes, and observed exits
are recorded in [final verification](../benches/results/performance-research-final.json).
These checks establish functional coverage; historical candidate performance
results below are not assigned to the corrected final source.

The subsequently approved local Docker/k6 campaign completed all 159 planned
observations on this machine: 68,548,839 measured requests and 15,001,485 warmup
requests, with no recorded correctness failures or incomplete iterations. No
candidate qualifies: allocation reductions coexist with text growth and mixed
or adverse timing observations. The final unchanged-baseline H2 upload run also
shows a severe latency outlier, retained in every aggregate and pair. Performance
acceptance depends on evidence, rather than authorization alone. Earlier shared VM
campaigns, standalone probes, CPU-clock accounting and functional checks do not
establish acceptance or satisfy the user's strict performance goal.

The earlier [joint wall/thread-CPU H1 control](../benches/results/head-len-cpu-http1.md)
completes all 168 rows and 84 pairs but retains adverse CPU/wall observations
and broad unchanged-control variation. It remains separate from the earlier
elapsed, timer and socket cohorts. The following sections preserve the
chronology of source hypotheses and archived experiments, rather than describe
a currently accepted performance candidate.

## Final review controls

Review found that the initial rejected-CONNECT predicate also set the close
flag on a registered 101 protocol transition. Although the callback still ran,
response serialization overwrote `Connection: upgrade` with `close`. The new
200/101 registered-CONNECT wire matrix reproduced that header regression before
the correction and passed afterward. The final predicate closes rejected final
responses while preserving the existing registered switch and its read-ahead.
A 101 transition leaves the original request outstanding under
[RFC 9110 §7.8](https://www.rfc-editor.org/rfc/rfc9110.html#section-7.8);
it is not automatically a final CONNECT rejection.

Earlier frozen candidates contain the initial predicate and were not tested
with this later 101 control. Their recorded passing controls and measurements
remain historical; they do not qualify the corrected final source.

Review also reproduced a runner compatibility regression: nginx-only metadata
collection attempted to hash an unused comparison executable. The runner now
records no comparison hash when every selected engine is nginx. The offline
`benches/test_run.py` controls cover nginx-only, repeated nginx, mixed nginx/
Hyper, and Maximus/Axum selections without launching containers. The original
runner is preserved locally for historical provenance; measured framework
cohorts all selected an engine that uses the comparison executable.

## Prepared dedicated-host qualification

A [portable qualification bundle](../benches/qualification-candidates/README.md)
now separates borrowed-error conversion, optional empty-H1 tracking and the
borrowed H2 timer into independent patches on the corrected CONNECT baseline.
Each patch changes exactly one library and preserves the registered 101 upgrade
control. This is a new source cohort; earlier combined-candidate measurements
do not qualify it.

The exporter verifies a restored source tree for each patch, rejects added or
unexpected paths, records source/candidate hashes, and excludes Git metadata,
build caches, ignored raw measurements and TLS credentials. Its extra-file
control reproduced the initial validator gap and verifies rejection after the
correction. The runbook includes correctness, footprint, allocation and matched
framework stages, explicitly identifies the TLS-H1 performance coverage still
needed, and requires stable same-binary controls before candidate acceptance.
All three independent sources now pass the complete Linux repository checks
(180 passing test executions each, one privileged BPF control ignored) and
independent H1/TLS/H2/H3/WebSocket interoperability with wire mutation controls.
The [preparation report](../benches/results/dedicated-qualification-preparation.md)
records the exact source/commands and remaining performance gates. The bundle
also preserves original production H1 as a separately identified benchmark
reference with the current harness. Final combination acceptance must compare
against both original and corrected references on common valid workloads and
footprint; improvements against a slower corrected baseline cannot hide a net
regression from the starting framework. No new performance optimization is
retained in root.

## Initial selection from the inspected baseline

The best first experiment preserves borrowed error messages in
[`Error::into_response`](../crates/core/src/lib.rs). `Error` already carries
`Cow<'static, str>`, but conversion calls `into_owned()` before constructing the
response. That copies nonempty static messages into a new `String`, although
`Body::from(&'static str)` already represents static payloads without allocation.
Matching the two `Cow` variants can preserve the existing static or owned body
path. Rust documents that
[`Cow::into_owned`](https://doc.rust-lang.org/std/borrow/enum.Cow.html#method.into_owned)
clones borrowed data and moves already owned data. This is an opportunity to
remove a copy, not a predicted throughput result.

The conversion must preserve status, exact body bytes, and
`text/plain; charset=utf-8`. Empty messages, Unicode, dynamically constructed
messages, and nested `Result` conversion are useful controls. Error-heavy
socket workloads matter because an optimization confined to rejection paths
may not affect the usual successful-request benchmark. Compare both an explicit
application error and a malformed HTTP request, since their connection behavior
differs. Measure linked code size as well as allocation traffic.

A second small candidate concerns stored task wakers in
[`Incoming::poll_next` and `Connection::poll`](../crates/http2/src/lib.rs).
Both replace an existing `Option<Waker>` with a fresh clone on repeated polls.
For the `Some` case, Rust recommends
[`Waker::clone_from`](https://doc.rust-lang.org/std/task/struct.Waker.html#method.clone_from)
because it avoids cloning when the existing waker wakes the same task. The
`None` case still needs a clone. This changes reference-count work rather than
necessarily removing a heap allocation. The executor's actual `RawWaker`
implementation determines its cost.

A useful probe counts clone, drop, and wake calls with the same waker and then
with a different waker. Pending upload bodies, resumed handlers, response
backpressure, dropped bodies, reset streams, and idle stream polls need coverage.
The most recent caller must still receive the wakeup. Do not retain the old
waker merely because it looks similar. A failed `will_wake` comparison does not
prove that two wakers refer to different tasks, so the documented `clone_from`
operation is preferable to a custom equivalence assumption.

## Broader candidates that need stronger evidence

[`Connection::poll`](../crates/http2/src/lib.rs) allocates a vector containing
every stream ID before visiting streams in cursor order. Stream removal during
that visit explains the snapshot. The vector allocates even when the output
budget stops polling early. An alternative can compute the next ID through
bounded `BTreeMap::range` lookups, with the original final ID and visit count
limiting each pass. This removes temporary storage but adds map searches.
Measure one active stream, 32 active streams, blocked streams, and mixed small
and large responses before choosing the representation. Reusing a stored vector
instead adds persistent per-connection storage and is a different tradeoff.

Round-robin cursor behavior, stream removal, response byte limits, and wakeups
are existing contracts. Send work must still obey both stream and connection
windows. [RFC 9113 §5.2](https://www.rfc-editor.org/rfc/rfc9113.html#section-5.2)
defines flow control. A faster single-stream result cannot qualify a change
that starves other streams. The separate expiration scan also collects IDs,
but runs on deadline events and deserves independent attribution.

Dynamic registration in [`DynamicRouter::add`](../crates/router/src/lib.rs)
builds a temporary `Vec<&str>` and then joins it into the normalized shape.
A preallocated `String` can construct that same shape directly. This affects
registration cost, not request throughput. Preserve duplicate detection,
literal precedence, renamed capture handling, and HEAD ordering. The existing
registration diagnostic provides a direct comparison; a new routing index
would need larger-route measurements to justify its extra code and state.

Successful dynamic dispatch also clones the registered route pattern for
`Request::set_route`. Literal routes contain no named captures, so those routes
can set the route metadata to an empty borrowed string instead. Clearing the
metadata matters when the incoming request already carries a previous route.
`Request::path` must still reject a missing capture. Test literal GET, fallback
HEAD, explicit HEAD, 405, and requests rerouted after a parameterized match.
Captured routes still need their actual parameter names and preserve the owned
pattern. Eliminating that remaining copy would require changing the ownership
contract between the router and an independently owned request. Leaking strings
or adding shared pointers to every request needs stronger evidence.

## Historical experiments must remain visible

Several attractive changes have already been attempted. Repeating them is
requalification of a known candidate, not a new discovery.

| Candidate | Existing evidence | Consequence |
| --- | --- | --- |
| Reuse the completed boxed H2 read future with `Pin::set` | [Optional I/O report](../benches/results/native-buffers.md) records one fewer allocation per H2 request, but inconclusive performance parity and a runtime revert | Rust permits replacement through [`Pin::set`](https://doc.rust-lang.org/std/pin/struct.Pin.html#method.set), which drops the pinned old value first. Correct pin semantics do not establish speed. |
| Replace H1 Transfer-Encoding and Expect vectors with iterators | [Routing report](../benches/results/lean-routing.md) excludes this candidate after a three-change bundle showed higher H1 echo CPU | The cause was unresolved. Test separately before retention. |
| Borrow Content-Type in JSON and multipart extraction | [Historical ownership investigation](https://github.com/vicotrbb/maximus/blob/d792d98a1cda02e8f65a6d36868f2fceb424c5b5/benches/results/lean-ownership.md) measured one fewer allocation in each extractor | H1 confirmation passed, but the full bundle had unresolved H2 tails and was reduced. It remains a useful isolated candidate. |
| Compact Huffman child indices from `usize` to `u16` | [Owned DATA and paths report](../benches/results/owned-data-and-paths.md#rejected-hpack-storage-experiment) measured node size 24 to 8 bytes, but slower short and long decodes | The reported 513-node trie fits the representation, but the previous timing regression rejects it as an automatic improvement. |
| Fuse path control and separator scans | [H2 ownership follow-up](../benches/results/h2-static-ownership.md#why-the-path-change-was-removed) restores the original validator | Lower focused predicate cost did not qualify the wider bundle. Preserve encoded separators, Unicode controls, and dot-segment policy. |
| Incremental H1 chunk-line CRLF scan | [Owned DATA and paths report](../benches/results/owned-data-and-paths.md#rejected-http1-experiments) records a large fragmented-line gain but a repeatable short-line slowdown | Preserve the current implementation unless a different design passes both cases. |

## Current HTTP requirements

The checked primary references are RFC 9110 for shared semantics, RFC 9112 for
HTTP/1.1, RFC 9113 for HTTP/2, RFC 9114 for HTTP/3, and RFC 7541 for HPACK.
Their publication dates alone do not describe the full current standard.
The [RFC Editor errata records](https://www.rfc-editor.org/errata_search.php?rfc=9112)
identify RFC 9931 as an update to RFC 9112. Reported, held, rejected, and verified
errata have different status and must not be treated interchangeably.

[RFC 9931 §8](https://www.rfc-editor.org/rfc/rfc9931.html#section-8), published
March 2026, requires HTTP/1.1 proxy servers to close the connection after
rejecting CONNECT without processing subsequent requests. The narrowly allowed
optimization requires knowledge that the client waits for success before
forwarding untrusted payloads. Section 5 does not require closing every rejected
Upgrade request.

The inspected H1 connection loop closes on 4xx responses, but its close
predicate does not cover every rejected CONNECT. A generated 501 response for
an unregistered tunnel or an application 3xx/5xx can leave the connection open.
The framework exposes CONNECT tunnel registration. That makes rejected CONNECT
followed by a pipelined ordinary request a concrete regression-test candidate.
Closing all rejected CONNECT requests is a conservative implementation that
does not require identifying compliant proxy clients. This is a correctness
finding, not a throughput improvement.

For H1 framing experiments, preserve rejection of ambiguous Content-Length and
Transfer-Encoding, HTTP/1.0 Transfer-Encoding rejection, exact body boundaries,
and preserved read-ahead bytes. See
[RFC 9112 §6.3](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3).
Removing a temporary collection must not accidentally accept a second header
or comma-separated unsupported coding. Expect handling must retain its current
duplicate and version policy and send 100 Continue only in the established
body-bearing case. The governing expectation semantics are in
[RFC 9110 §10.1.1](https://www.rfc-editor.org/rfc/rfc9110.html#section-10.1.1).

HPACK optimizations must preserve table eviction, decoded byte and field-count
limits, integer overflow checks, and Huffman EOS and padding rejection.
Dynamic references must survive eviction later in the same block. Static
borrowing is already retained. The stateless response encoder's never-indexed
literals avoid introducing dynamic compression state. A dynamic encoder or
Huffman encoder would need its own CPU, wire-byte, memory, and sensitive-field
assessment under [RFC 7541](https://www.rfc-editor.org/rfc/rfc7541.html).

Cookie joining before generic application dispatch remains necessary for H2
and H3. See [RFC 9113 §8.2.3](https://www.rfc-editor.org/rfc/rfc9113.html#section-8.2.3)
and [RFC 9114 §4.2.1](https://www.rfc-editor.org/rfc/rfc9114.html#section-4.2.1).
Saving allocations by forwarding split Cookie fields would change application
semantics.

## Evidence needed for retention

Each candidate needs a frozen baseline, an isolated patch, direct savings,
correctness checks, a release-footprint comparison, and matched socket trials.
The [benchmark workspace](../benches/baselines/Cargo.toml) already contains Hyper
and Axum outside the product dependency graph. Use the same workload and client
validation for every engine. Record differing policy and adapter costs instead
of attributing the whole framework gap to one module.

An error conversion needs an error workload. An extractor optimization needs
JSON and multipart parsing. A scheduler optimization needs concurrent streams,
small flow-control windows, and a large-response fairness workload. Include an
unchanged protocol control because linked code layout can affect other paths.
Throughput, CPU per completed response, allocation traffic, live retained bytes,
RSS, and tail latency answer different questions. The repository's historical
results already demonstrate that reducing allocations alone does not qualify
an implementation under a strict no-regression requirement.

This research uses Maximus source, Rust API contracts, and protocol documents.
It does not import an implementation from a competing framework. Standard HTTP
semantics are shared requirements; an implementation does not become more
correct by inventing a unique interpretation of them.

## Persistent shutdown waiters

A further source inspection identifies repeated shutdown-listener allocation
as an independently testable candidate. This is an unmeasured hypothesis.
The inspected lockfile selects `event-listener` 5.4.2 and `futures-util` 0.3.34.
[`Shutdown::cancelled`](../crates/runtime/src/lib.rs) calls `Event::listen`
before checking the cancellation flag. The locked
[`Event::listen` implementation](https://docs.rs/event-listener/5.4.2/src/event_listener/lib.rs.html)
constructs a `Box::pin(InnerListener)` on each call. Its public documentation
explicitly describes that heap allocation. The event's shared state is separate
from this per-listener box. A future constructed but never polled has not yet
called `listen`, so counting source-level future constructions overstates the
allocation count.

[`worker_loop`](../crates/framework/src/server.rs) constructs and polls a new
fused `shutdown.cancelled()` on every accept or task-completion iteration.
The HTTP/2 `run` function constructs separate stop futures for each output write
and main event selection. The main selection polls stop before its other
branches. The write selection polls the writer first, so an immediately ready
write can finish without constructing an event listener. Source inspection
therefore supports repeated allocations, but does not establish a fixed
number of allocations saved per request or DATA frame.

One stack-pinned fused shutdown future outside each loop can keep its registered
listener across selections. The worker breaks when that future completes.
HTTP/2 can use the same pinned future sequentially in both selects. The
[`Fuse` source](https://docs.rs/futures-util/0.3.34/src/futures_util/future/future/fuse.rs.html)
drops its inner future after completion, reports termination, and returns
`Poll::Pending` on later direct polls. The locked select macro skips terminated
branches. Its [documentation](https://docs.rs/futures-util/0.3.34/futures_util/macro.select.html)
specifically recommends fusing outside loops. Do not recreate a `.fuse()` wrapper
around an already completed unfused shutdown future each time.

The cancellation flag is sticky. Keeping the listener between selections does
not change the flag's Acquire/Release synchronization or the existing
listen-before-check protection against a missed notification. The same future
must never be polled concurrently by two independent tasks. The existing two
H2 selects run sequentially in one connection task, so sharing one future there
does not introduce concurrent polling. Event-listener documents notification
forwarding when a notified listener is dropped. Retaining the listener changes
that drop timing, but shutdown notifies every registered listener and the sticky
flag also covers later registrations.

Three HTTP/2 cases need explicit treatment:

1. Shutdown starts draining an otherwise active connection. The one completed
   stop future must emit one drain action and then remain terminated. Existing
   stream work and the drain timer must continue running.
2. The peer sends GOAWAY or the request cap starts draining before shutdown.
   The current closures suppress stop while draining. An unconditionally polled
   persistent stop future can instead fire once during that existing drain.
   `Connection::drain` checks `draining.is_none()`, so the repeated call neither
   emits another GOAWAY nor resets the deadline. This changes event-selection
   order once and still needs tests. Preserving the explicit suppression through
   a borrowing wrapper is a separate implementation option.
3. Shutdown wins while a response write is pending. Preserve and resume the
   already submitted write future. Recreating the write can lose partial output
   or violate completion-I/O ownership. The existing writer already carries a
   timeout bounded by the original remaining drain budget when draining had
   started earlier. Keeping that inner timeout prevents the nested drain wait
   from extending the original write deadline.

The H2 main select prioritizes stop over its timer. If stop fires after another
cause already started drain, it can delay expiration handling by one loop
iteration. A persistent `Fuse` prevents an indefinitely ready shutdown branch
from starving the timer. Preserve the existing write-first selection order and
the read/work alternation while testing this change. Worker-loop acceptance
must retain its existing submitted accept future across ordinary task
completions. Reusing stop is no reason to rebuild or cancel that accept.

Focused verification needs cancellation before first poll, while idle, after
many successful iterations, during a blocked write, and after peer/request-cap
draining. A response stream that remains pending must terminate by the existing
drain deadline. Repeated `cancel()` calls must not emit repeated GOAWAY or
restart the deadline. A saved task future can alter the enclosing future's
layout, and retaining a listener keeps one box live while ordinary work runs.
Measure future size, per-connection retained bytes, and complete-request
allocation traffic alongside throughput and tails. No previous attempt at this
exact persistent shutdown-waiter change was found in the checked benchmark
reports. No product code or benchmark was changed for this inspection.

## Experiment disposition

The persistent shutdown-waiter candidate was implemented experimentally.
Worker-loop persistence was rejected after a matched minimal hello comparison
showed 831,781 text bytes versus 827,893 in the baseline. The H2-only outcome
was reassessed with matched-feature footprint builds and is selected for
measurement. Its performance remains unqualified.

The earlier `fused-layout-footprint.log` and `h2-persistent-footprint.log`
conformance builds enabled additional JSON, multipart, SSE, and other features.
The original baseline conformance build enabled `tls,http2`. Their text-size
differences therefore cannot establish growth caused by H2 persistence and must
not support a rejection decision. Sequential `matched-size-baseline.log`,
`matched-size-compact.log`, and `matched-size-h2-persistent.log` builds replace
those invalid controls. All three use the same locked compiler, release profile,
and `tls,http2` feature set. The baseline and compact artifact hashes reproduce
their earlier matched builds exactly.

Conformance text bytes are 2,151,396 for baseline, 2,151,180 for compact ownership
changes, and 2,143,776 with H2-only persistence. H2 persistence therefore removes
7,620 text bytes versus baseline and 7,404 versus compact. Its unstripped file
is 2,952,688 bytes, 984 below baseline. All three stripped files remain
2,233,184 bytes. These are footprint observations, not proof of faster execution.
Measured H2 futures were 1,480/2,888 bytes versus 1,576/2,984 for compat/native
respectively. That layout result is separate from linked executable size.
The raw logs, mismatched controls, and candidate hashes remain under
`artifacts/performance-2026-10-20/`. The earlier rejection record must be read
with this feature-mismatch correction. These matched footprint observations
belong to the earlier source that still contained direct shape construction.
They do not identify the later retained snapshot.

The initial complete-H2 allocation guard predicted at most 28 calls and failed.
The isolated baseline, ownership-only, and fused observations recorded 32.0180,
32.0185, and 30.0748 calls per request respectively. Their requested-byte values
were 5,739.0224, 5,740.8008, and 5,827.9898. Other H2 rows moved differently.
These complete-process observations include runtime scheduling. They establish
neither a fixed listener-allocation saving per request nor unchanged requested
bytes. The failed prediction and every emitted row remain preserved. The guard
does not strengthen an allocation-saving or performance claim. No later socket
result qualifies this experimental persistent-future source.

Direct normalized-shape construction was also rejected. Seven CPU-0-pinned
component triples rotated baseline, initial ownership, and selected variants.
Each registration observation measures 10,000 complete 64-route table
operations. Median baseline time is 24,970.130 ns per table, versus 28,859.529 ns
for selected. Every matched selected pair is slower, by +17.1%, +6.0%, +12.2%,
+24.9%, +8.7%, +10.0%, and +26.4%. Removing 64 allocations per table does not
compensate for that consistent timing regression under the acceptance rule.
The [execution outcomes](../artifacts/performance-2026-10-20/component-outcomes.json)
and raw `components-{baseline,initial,selected}-{0..6}.csv` observations remain
preserved. Representative [baseline](../artifacts/performance-2026-10-20/components-baseline-0.csv)
and [selected](../artifacts/performance-2026-10-20/components-selected-0.csv)
CSVs show their complete component rows. The original `Vec` plus `join` shape
construction is restored. The long-capture regression test remains. The
registration allocation guard is removed, while the diagnostic continues to
emit registration observations.

Worker-loop persistence remains reverted. The intermediate `retained-source`
candidate contains borrowed error ownership, constant-time literal
classification through `pattern.len() != shape.len()`, H2-only persistent
shutdown, and rejected-CONNECT closure. The H2 shutdown wire test remains as
regression coverage. The frozen `retained-source` and `retained-baseline-source`
snapshots have identical current allocation, component, and comparison-server
fixtures. Their manifests distinguish this reduced candidate from the preceding
`selected-source`. Its measurements require separate source and tool identities. The
[initial ownership report](../benches/results/ownership-registration.md) and
evidence retain their original scope. The updated runner's
`--reuse-certificate` option allows the next campaign to share one certificate
across all TLS variants. That improvement does not retroactively change the
initial campaign's distinct baseline/candidate certificate keys.

The intermediate retained candidate remains unqualified. Seven matched
`components-final-retained-baseline-{0..6}.csv` and
`components-final-retained-{0..6}.csv` observations show static-error medians of
37.647 → 32.472 ns, with all seven candidate pairs faster. Literal dispatch is
110.817 → 110.440 ns. Captured dispatch is 162.276 → 166.404 ns, with five of
seven candidate pairs slower. Unchanged registration is 23,293.706 → 23,744.198
ns, with mixed pair directions. The five separately named
`allocations-final-retained-baseline-{0..4}.csv` and
`allocations-final-retained-{0..4}.csv` pairs retain static-error 3 → 2 and
literal GET 2 → 1 allocation calls, with registration unchanged at 389. Those
allocation observations do not resolve the captured-dispatch timing result.
No socket campaign has qualified this source.

A distinct `route-shape-source` representation was investigated next.
`Route.pattern` becomes `captures`, which holds the original pattern only for
captured routes and an empty string for literals. The existing normalized
`shape` supplies path matching and both operands of precedence comparison.
Literal routes therefore store their path once. Dispatch unconditionally
clones `captures`, preserving parameter names for captured routes and clearing
prior request metadata for literals without a per-dispatch classification.
The original `Vec` plus `join` shape construction remains. This change is a new
representation candidate, not evidence that the preceding timing concern is
fixed. Baseline and candidate fixtures now both include literal-route
registration allocation and timing cases. New measurements need their own
source identities and must not be pooled with the earlier retained observations.

That representation and a subsequent field-layout variant were rejected.
Seven `shape-components-{baseline,candidate}-{0..6}.csv` pairs show captured
dispatch medians of 164.547 → 171.528 ns, a 4.24% increase, with five of seven
candidate pairs slower. A second variant changed only the route field order to
place matching shape first. Its seven
`layout-components-{baseline,candidate}-{0..6}.csv` pairs show captured dispatch
157.510 → 160.333 ns and 405 dispatch 111.590 → 117.268 ns. Both cases slow in
six of seven pairs. The observed registration and literal-route storage savings
do not qualify these candidates under the strict no-regression requirement.

The router implementation is now restored completely to baseline. Added routing
regression tests remain as controls. All literal allocation guards are removed,
and the current baseline and candidate allocation diagnostics are expected to
exit zero. The reduced `core-h2-source` candidate contains only borrowed error
payload ownership, H2-only persistent shutdown listeners, and rejected-CONNECT
closure. Worker-loop shutdown remains original. The separately frozen
`core-h2-baseline-source` supplies original product code with identical current
fixtures. The baseline comparison binary reproduces SHA-256
`be05e0aea314cfdda839441c4c3c923e4cae9c8100f4836f382d7ca1e703e8f2`.

The planned `core-h2-wrk` HTTP/1 and `core-h2-tls` k6 HTTP/2 cohorts use different
load generators and need separate summaries. No results are claimed here
before their exact-source component, allocation, and socket observations arrive.

The first `core-h2-source` component screen has now completed, with seven
`core-h2-components-{baseline,candidate}-{0..6}.csv` pairs and five allocation
pairs. All allocation diagnostics exit zero. Static-error median time is
37.991 → 31.929 ns, with all seven pairs faster. Captured and 405 dispatch
medians are 152.937 → 151.753 ns and 112.862 → 112.539 ns. Owned-error and
literal controls move adversely, 36.288 → 37.122 ns and 105.377 → 106.370 ns,
with six and five of seven pairs slower respectively. These observations do
not qualify the initial mutable-response implementation as regression-free.

A subsequent `core-h2-tuple-source` candidate composes the existing
`(status, message).into_response()` implementation inside each borrowed/owned
match arm. It replaces the initial response assembly and explicit status
mutation, preserving their intended semantics with two fewer lines. The
`tuple-*` component and allocation observations must remain separate from the
first `core-h2-*` screen. Any network campaign must identify the exact frozen
candidate it executes. The analysis script requires an explicit candidate
manifest selection to prevent assigning tuple-source observations to the
earlier mutable-response source.

The tuple candidate's seven component and five allocation pairs are complete.
All 24 processes in `tuple-component-allocation-outcomes.json` exit zero, with
eight component rows and eighteen allocation rows per run. Static-error median
time is 38.194 → 32.061 ns, with all seven pairs faster. Owned error is
35.573 → 35.705 ns. Literal, captured, and 405 dispatch medians are
105.270 → 108.638 ns, 154.272 → 154.025 ns, and 114.692 → 112.934 ns
respectively. Literal control pair signs are mixed. These are focused timings,
not socket acceptance.

Complete small-H2 allocation medians are 32.0160 → 30.0165 calls and
5,731.9088 → 5,621.6872 requested bytes per request. Large static H2 observations
are 38.9624 → 35.9149 calls and 23,557.8544 → 23,368.1944 bytes. The H1 control
moves adversely, 16.0292 → 16.0891 calls and 1,940.8244 → 2,200.6988 bytes.
That adverse observation remains recorded, with no H1 saving attributed to the
candidate. Complete-process counters include runtime/client work and do not
establish a fixed per-frame saving.

The frozen tuple comparison binary has SHA-256
`c0205e565b99b30488c07de092d1df4b601a89ecac588f49b7f251889e567f18`.
Its final host, Linux, and independent-client checks pass. The tuple conformance
artifact has 2,143,776 text bytes, 2,952,712 file bytes, and 2,233,184 stripped
bytes. That file size belongs to the tuple artifact and differs from the
earlier matched H2-only build. Socket campaigns have begun under separate wrk
and k6 labels. No socket acceptance is claimed while those campaigns run.

## Deferred H1 completion-state experiment

The inspected HTTP/1 `connection` function creates an `Rc<Cell<bool>>` for every
parsed request, including `Framing::Empty`. It already skips the `Incoming`
producer for empty framing. An independently testable representation is
`Option<Rc<Cell<bool>>>`, with `None` meaning that no request body remains and
`Some` holding the existing shared completion state for a framed body. This is
a source-based hypothesis, excluded from the frozen tuple candidate. No
allocation, timing, or footprint result is claimed for it.

The final unread-body check and upgrade guard could test whether an existing
cell is still false. They must not interpret absence as an unread body.
Fixed-length and chunked producers must retain their existing shared cell and
completion transitions. In particular, a chunked request whose decoded body is
empty still needs its producer to consume the terminating chunk and trailers.
Classification must follow parsed framing, not GET/HEAD method assumptions or
an application body hint. [RFC 9112 §6](https://www.rfc-editor.org/rfc/rfc9112.html#section-6)
defines request framing independently of method semantics.

Useful controls include absent Content-Length, Content-Length zero, consumed
and unread fixed bodies, empty and nonempty chunked bodies with trailers,
pipelining after completion, early handler response, Expect handling, and
WebSocket/CONNECT upgrade read-ahead preservation. Future layout, linked code
size, retained bytes, and framed-body branch cost need measurement alongside
the ordinary empty-request case. A representation that helps empty requests
but slows body-bearing requests would fail the same acceptance rule.

No exact previous attempt appeared in the checked benchmark reports or the
targeted H1 Git-history search. The earlier empty-`Body` representation change
and H1 response terminator-buffer reuse address different state. This history
check does not establish that every private or unrecorded experiment has been
exhausted. The current socket campaign remains unchanged.

## Rejection after the primary tuple campaign

The primary tuple campaign completed 84 observations. Its 128 KiB H2 echo
worsened at p99 and p99.9 in all three matched pairs. H2 persistent shutdown
was rejected under the strict no-regression requirement, and the H2 product
implementation is restored exactly to baseline. The frozen tuple source and
all observations remain preserved in the [tuple report](../benches/results/core-h2.md).
This conservative decision does not establish a specific cause for every
observed tail difference.

The separate 56-observation H1 confirmation is still running at this decision.
A planned H2 follow-up script was never launched and will not be used to seek a
favorable result. Exact tuple-source profiling and sanitizer fuzzing were
deliberately not run after rejection. The completed 172 host and 149 Linux
passing test executions, with one privileged BPF skip, retain their original
scope. Earlier profile/fuzz results are not reassigned to the rejected tuple.

The next isolated candidate will test the H1 optional completion state described
above, together with borrowed errors and rejected-CONNECT closure. It will
contain no H2 product change. That future candidate needs separate source
identities and measurements before any acceptance claim.

## Archived H1 candidate evidence

The subsequent H1 completion-state candidate is now measured separately in the
[H1 report](../benches/results/h1-empty.md). It retains borrowed errors and
rejected-CONNECT closure, with H2, router, and worker product implementations
matching baseline. Five fresh allocation pairs validate an approximately
one-call reduction on the complete empty-framed H1 request path. Its primary
84-observation socket cohort remains performance-unqualified because adverse
CPU and tail observations remain unresolved. The rejected persistent-H2 tuple
is a different frozen source and is not reintroduced by these results.

Forced release rebuilds reproduce the baseline and H1 candidate comparison
binaries byte-for-byte. Separate exact-source CPU-clock profiles are complete:
H1 plaintext has 5,100,540 checked responses and 2,955 samples; H2 large echo has
79,223 checked responses and 2,662 samples. Neither loses samples. Restricted
kernel symbols and unresolved libc symbols remain, and CPU-clock samples are
not hardware cycle measurements or paired causal attribution.

The fixed 42-observation isolation campaign is complete and reconciled in a
separate [H2 attribution report](../benches/results/isolate-h2.md). Its 21 pairs
cover seven rounds each of an identical-baseline A/A control, error-only source,
and H1-completion/CONNECT-only source. All 859,622 responses pass validation.
The same A/A binary produces paired RPS changes from −3.16% to +90.16% and
p99.9 changes from −53.45% to +50.13%. Error-only paired p99.9 has a +35.95%
median, while H1-only has −39.53%. Attribution and strict socket qualification
remain unresolved. The analyzer excludes only Docker's random nodename from
kernel identity and preserves all other uname fields and raw strings. These
observations remain separate from the primary Hyper/Axum comparison; A/A
variation is not a proven explanation for that campaign's adverse results.

## Absolute H2 timer experiment

The locked `compio-runtime` 0.12.6 source supports a narrow experiment for
the H2 connection's event timer. The baseline expression is
`sleep(deadline.saturating_duration_since(Instant::now())).fuse()`.
[`sleep`](https://docs.rs/compio-runtime/0.12.6/src/compio_runtime/time/mod.rs.html)
then calls `sleep_until(Instant::now() + duration)`. The existing absolute
deadline can instead be supplied directly to `sleep_until(deadline).fuse()`.
The initial owned-`Fuse` main H2-loop replacement was frozen in
`absolute-timer-source`, identified by `absolute-timer-manifest.json` and
`absolute-timer.patch`. It was subsequently rejected for linked code growth.
A separate borrowed-timer form is implemented and awaiting validation. The
frozen H1 candidate and completed attribution campaign are unchanged.

For a timer that is polled, the direct form removes two explicit clock reads
before the common timer-registration path. For an unpolled timer, only the
current caller's initial clock read occurs, so an exact two-call saving must
not be claimed for every loop iteration. The common registration still reads
the clock. `sleep_until` calls `TimerFuture::try_new`, whose
[`TimerRuntime::insert`](https://docs.rs/compio-runtime/0.12.6/src/compio_runtime/time/runtime.rs.html)
returns no timer when the supplied deadline is at or before now. An expired
deadline therefore remains immediately ready when polled.

The relative conversion also reconstructs a later target. For a future deadline,
subtracting a clock reading and then adding a subsequent reading shifts the
registered target by the elapsed interval between them. That interval can
include scheduling delay. Direct absolute registration preserves the original
target instead. This changes that accidental postponement, so the candidate
needs deadline-behavior checks as well as performance measurements. It does not
remove timer registration or prove an allocation reduction.

Existing H2 wire checks cover `handler_deadline_produces_a_response`,
`first_header_payload_deadline_survives_other_stream_progress`,
`response_progress_refreshes_idle_deadline`, blocked-response shutdown, and
shutdown during an existing request-cap drain. The in-module
`continuation_preserves_the_first_envelope_deadline` check also matters.
A focused timer check should additionally cover already-expired, equal-to-now,
and future deadlines without panic or a renewed relative delay. Preserve the
existing select order, read/work alternation, shutdown behavior, and deadline
selection. Future layout, linked text, and matched H1/H2 controls need checking
because the wrapper's future type changes.

`timeout_at` is outside this narrow experiment. Its locked implementation
routes through `timeout(deadline - Instant::now(), future)`, retaining relative
conversion. An expired deadline alone is not evidence of a panic on this
toolchain. Installed Rust 1.96.0 source and the
[standard `Instant` documentation](https://doc.rust-lang.org/std/time/struct.Instant.html#monotonicity)
show that subtraction currently saturates to zero. Earlier versions panicked,
and the documentation reserves the possibility of future changes. No current
panic claim is made here.

No exact `sleep_until` attempt appeared in the checked benchmark reports or
the targeted H2 Git history. The recorded history search found the original
native-engine introduction, not a prior trial of this replacement.

The [standalone component report](../benches/results/absolute-timer-component.md)
now records seven alternating pairs with 2,000,000 poll-and-drop operations per
mode per round inside an actual `Runtime::block_on` poll context. Relative and
absolute medians are 59.645 and 27.157 ns, with all seven absolute pairs faster.
The measured timer future shrinks from 120 to 96 bytes. Expired and future
deadline controls pass in every mode/round. Probe dependencies match the product
lockfile, including libc 0.2.190, and add no product or benchmark dependency.
These mechanism measurements do not establish HTTP throughput, allocation,
tail-latency, or enclosing connection-future gains. In particular, they measure
the original owned-`Fuse` expression, not the later borrowed timer variant.

The direct owned-`Fuse` product artifact has 2,155,420 conformance text bytes,
4,024 above the original baseline's 2,151,396 bytes. It was rejected despite
the standalone timing result. The raw footprint and layout observation remains
in `absolute-timer-footprint-layout.log`. No socket qualification is assigned
to that rejected artifact.

The borrowed form pins `sleep_until(deadline)` directly and uses
`timer.as_mut().fuse()` in both event-selection branches. Its recorded
conformance artifact has 2,146,608 text bytes, 2,953,664 file bytes, and
2,233,184 stripped bytes. Measured compat/native serve futures are 1,528 and
2,936 bytes on that target. The raw matched artifact observation is
`absolute-timer-borrowed-footprint-layout.log`. This satisfies the observed
footprint screen but does not prove the owned probe's timing applies to this
different future representation.

Borrowed-timer correctness checks completed: 176 host executions and 151 Linux
executions pass, with one BPF-related skip. Independent client/protocol checks
and 2,062 raw-wire mutation/disconnect cases pass; H2 sanitizer fuzzing completes
1,335,300 executions in 61 seconds without failure. Frozen comparison binary
`b3e826754d19d7df1d18fc952e93db2a58e5992d9b043c08f7cbb2523ca86f61`
is identified by `absolute-timer-borrowed-manifest.json` and
`borrowed-timer-build.log`.

Five fresh allocation pairs all exit zero. Complete H1 calls have medians
15.0051 → 14.0069, while complete H2 static calls are 30.0070 → 30.0308 and
requested bytes are 6,898.1920 → 6,909.0448. Adverse fractional H2 observations
are retained; no H2 allocation reduction is claimed. Fourteen prior component
process observations are explicitly reused because both corresponding linked
binaries are byte-identical. They do not measure H2 timer code.

The `borrowed-timer-wrk` five-second campaign was **ABORTED / UNQUALIFIED**
after severe VM control-plane stalls. One CPU snapshot interval reached
45.869681040989235 seconds despite the five-second client setting. Nine
completed invocations and nineteen engine observations remain preserved:
seventeen rows belong to completed invocations and two to the interrupted
tenth invocation. These rows reconcile to 21,654,325 checked responses. The
planned `borrowed-timer-tls` cohort never launched. The full 84-observation
analyzer was not executed, and this partial cohort is never pooled with prior
completed campaigns. Its stop was prompted by stalls, rather than source
acceptance or favorable-result selection.

The [borrowed-timer report](../benches/results/borrowed-timer.md) and
[partial evidence](../benches/results/borrowed-timer-aborted.json) retain all
rows, adverse pairs, raw hashes, missing scope, and observed ranges. The
maximum interval does not establish disk/cache/resource pressure as the cause;
that remains under investigation. Root reports independent static source
review found no actionable findings, limited to source correctness. No socket
acceptance is claimed. Its first dedicated probe overlaps a hashing helper at
startup and remains exploratory. A separate clean borrowed probe then records
seven alternating pairs of two million poll/drop operations per mode, with
median relative/absolute 55.262 → 25.289 ns and all seven pairs favoring the
absolute form. The actual runtime context and expired/future controls are
preserved. [Clean mechanism evidence](../benches/results/borrowed-timer-component.md)
records exact source/executable/raw hashes and all dependency lock versions
and checksums, matching the product lock. Its stored-F sizes 120 → 88 bytes
exclude the borrowed Fuse wrapper and enclosing async frame. Separate observed
compat/native serve futures decrease by 48 bytes each, 1,576 → 1,528 and
2,984 → 2,936. Neither this mechanism evidence nor cache cleanup establishes
HTTP socket performance or the cause of earlier stalls.

The subsequent [same-binary H2 control](../benches/results/post-cache-aa-h2.md)
completes eighteen observations, six repeats each of Maximus/Hyper/Axum from
the original baseline executable. Raw summaries reconcile 510,767 measured
and 292,082 warmup checked responses, with no failed checks or incomplete
iterations. CPU snapshot intervals are 5.43358025001362–6.517835750011727
seconds, while substantial throughput/CPU/tail variation remains. All nine
adjacent same-engine repeat pairs are retained as A/A differences, rather than
candidate gains. The 45-second stall does not recur in that invocation;
stability, cache-cleanup causality, and candidate socket performance remain
unqualified. No completed or partial campaigns are pooled.

The [H1 memory-transport control](../benches/results/memory-http1.md) then
completes all 56 processes and 168 rows, with 84 matched engine/case pairs.
Its source assertions cover 168 million timed and 1.68 million warmup HTTP/1.1
responses, exact framing/body/header counts, and final-connection closure.
Direct Maximus plain medians are 986.928 → 921.100 ns/request with mixed pairs;
static/owned error medians are adverse. Dynamic plain is faster in six of
seven pairs, but other cases and unchanged Hyper/Axum controls retain slower
observations and substantial variation. Strict performance qualification
remains unresolved. This probe exercises H1 shared futures memory transport,
allocation and output retention, with final validation/destruction outside
timing. It provides no H2, native TCP I/O, socket-capacity, latency-distribution,
or RSS result. Exact probe source/lock/build/run drivers and product manifests
are embedded in maintained evidence, with every raw row and pair preserved.

An independent [stream-ID traversal model](../benches/results/stream-schedule-model.md)
was rejected before any product edit. Live BTreeMap lookup avoids the snapshot
in small-budget cases, but the 256-stream full-budget model is slower in all
seven pairs: median 2,012.651 → 2,284.054 ns/poll (+13.48%). All 84 raw rows
and 42 pairs remain preserved, with fixed source/executable identities,
operation denominators and model equivalence assertions. Repeated `range_mut`
initialization/search is a plausible source-based explanation, not a profiled
cause. Actual `Connection::poll` remains unchanged by this experiment; the
model establishes no HTTP, allocation-counter, retained-memory or RSS result.

The artifact-only [direct parsed-head H1 prototype](../benches/results/h1-head-direct.md)
was rejected before a CPU campaign. Its only source change parses the completed
header prefix while borrowed inside the scanner, drains it, and returns `Head`
instead of a temporary header `Vec`. Native controls pass all 19 cases, and
the fragmented/pipelined metadata control passes against baseline. Matched
hello actual `.text` grows 618,328 → 622,168 bytes (+3,840), GNU text grows
827,585 → 831,473, and data grows by 24 bytes. Conformance GNU text decreases
2,146,608 → 2,146,076, while data grows by 24; that improvement does not offset
the minimal-artifact regression. Both stripped file lengths remain unchanged,
and H1 compat/native future sizes remain 4,328/4,352 bytes. Built memory probes
have no timed observations assigned. The `head_len` alternative is separate;
the direct-return result is preserved without a CPU/allocation-saving claim.

Root reports removing only regenerable Cargo debug dependencies (about 22 GB)
and incremental caches (about 13 GB logical, with an earlier cleanup yielding
about 7 GB physical) after the local disk filled. Frozen raw data, executables,
and source snapshots remain preserved. Docker was unavailable while the
OrbStack updater was active; `orbctl start` reported it already running and
Docker later returned ready with server 29.4.0. Those observations establish
neither disk-pressure causality for runtime variation nor updater causality
for Docker's interruption. Subsequent cohorts require fresh environment
captures and remain separate from earlier measurements.

The artifact-only [head-length H1 alternative](../benches/results/head-len-memory-http1.md)
keeps parsing at its original outer site, borrowing the buffered complete
prefix by length before draining it. Its separate elapsed-time memory cohort
completes all 56 processes, 168 rows and 84 pairs with protocol assertions
passing. All six Maximus engine/case medians are adverse; direct static error
is slower in all seven pairs. Unchanged Hyper plain is also slower in all
seven pairs, with broad comparator variation retained. A saved host/compiler
snapshot is context only, not a cause assigned to any measurement. Every
observation remains included, without normalization or pooling. Strict
performance qualification remains unresolved and this artifact is not promoted
to root product code.

Matched `head_len` hello/conformance GNU text decreases by 136/392 bytes,
while data increases by 24 bytes and unstripped file sizes increase by
200/184 bytes; stripped lengths are unchanged. H1 compat/native future storage
remains 4,328/4,352 bytes. The maintained elapsed-time evidence preserves exact
build/source/lock/protocol/footprint/layout provenance and every adverse pair.
A separately instrumented thread-CPU-clock fixture is planned as a distinct
cohort; it does not replace or retroactively qualify these observations.

That separate [joint wall/thread-CPU cohort](../benches/results/head-len-cpu-http1.md)
now completes all 56 processes, 168 rows and 84 matched pairs, with 168 million
timed and 1.68 million warmup responses checked. All CPU-resolution stderr
records report one nanosecond. Direct owned-error CPU median worsens
874.725 → 949.925 ns/request; other cases retain adverse pairs despite favorable
medians, including static error +39.77% and dynamic owned error +28.11%.
Unchanged comparator CPU variation remains broad. Wall and CPU values, every
pair, exact final formatted source/build/dependency identities, and excluded
unmeasured libc drift are preserved separately from earlier elapsed data.
Guest thread CPU accounting is not physical cycles or proof of absent host
contention. None of the experiments earns strict performance acceptance, and
the three remaining performance hunks have been restored; only the CONNECT
correctness correction remains in product source. Final restored-root checks
remain pending separately.
