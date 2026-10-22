# Shared-path performance and size design

Date: 2026-10-21

Status: approved by the user on 2026-10-21 for planning and implementation
sequentially in this chat. Product implementation has not started.

Baseline: `f49fdd6`, including the retained SSE and gzip improvements.

## Outcome and constraints

Improve the costs shared by many Maximus workloads: connection memory,
body delivery, scheduling, request policy, and compiled application size.
Investigate all five areas. Retain only independently qualified improvements;
a rejected experiment is not an implemented improvement.

The user requires lean, idiomatic Rust, protocol correctness, comparisons
against Hyper and Axum for every core change, and original implementation.
Existing protections, resource limits, feature combinations, and supported
transports must continue to work. Rust 1.96 and edition 2024 remain the floor.
Do not add product dependencies, unsafe code, public configuration, or public
API changes for this campaign. Do not copy competitor implementation code.

Treat correctness and security regressions as unconditional rejection.
Performance, memory, and size tradeoffs require an explicit review of the
measured benefit and cost before retention. Do not infer that encouragement
of valuable tradeoffs authorizes an arbitrary regression budget.

No finite test campaign can prove universal absence of regressions or that
Maximus is the best framework on every workload. Report the measured scope.

## Existing behavior

- `crates/http2/src/lib.rs` reads frame headers and payloads into owned
  vectors and creates a boxed read future for each frame.
- HTTP/2 owns incoming body queues and moves DATA ownership into them.
  Outgoing DATA is copied into a reusable output vector.
- The connection loop writes and flushes output before returning to its
  normal selection of reads, stream work, shutdown, and deadlines.
- Stream polling snapshots IDs from a `BTreeMap`, performs mutable lookups,
  and stops at the configured output budget. Deadline selection scans streams.
- `crates/http1/src/lib.rs` already reuses connection buffers and coalesces
  buffered responses. Do not duplicate or remove those mechanisms blindly.
- `crates/core/src/policy.rs` enforces shared request checks and timeouts.
- `crates/tls/src/lib.rs` provides bounded Compio compatibility buffering.

These observations identify candidate costs, not measured causal attribution.
The current latest campaign has substantial identical-binary variation and
unresolved profile samples. Earlier fixed-array reads, repeated map traversal,
and timer changes include rejected or unqualified experiments. Their results
must inform new probes rather than be relabeled as current acceptance.

## Approach and alternatives

Use incremental private changes with a frozen baseline and one mechanism per
candidate. Keep the existing protocol engines and module boundaries. First
measure allocation and I/O operations, then compare complete servers.

A combined engine rewrite could change ownership and scheduling together,
but would obscure attribution and expand the protocol review burden. Reject
that approach for this campaign.

Continuing only helper-level optimizations would be simpler, but would leave
the shared connection and body path unaddressed. Use helper simplification
only when measurements connect it to the campaign's goals.

## Part 1: connection-owned receive memory

Keep transport reads cancellation-safe and retain submitted owned buffers
until completion. Reuse header and receive storage where doing so removes
measured work without copying an entire body merely to reclaim a buffer.
Prefer existing `Vec`, `Bytes`, and `BytesMut` ownership operations to a pool.

The reader owns transport scratch memory. A delivered body frame owns its
payload independently of future reads. Never mutate bytes still visible to
a handler. Bound retained capacity using existing I/O and protocol limits.
Do not allocate the configured maximum body size per connection.

Choose the exact private reader representation after comparing reuse with
the current move-based path. Do not assume a fixed-array header or persistent
future is beneficial: both executable size and complete-path CPU matter.

Test fragmented headers and payloads, back-to-back frames, padding, EOF in
each read phase, invalid sizes, continuation deadlines, cancellation, and
payload lifetime across subsequent reads. Preserve returned flow-control
credit and body ordering.

## Part 2: bounded output batches

Keep each stream's pending body ownership distinct from the connection's
output storage. Evaluate reducing repeated body copies and producing more
useful DATA work per batch within existing byte and flow-control budgets.
Compare plaintext and TLS separately before selecting a write strategy.

Flush headers before waiting for a streaming producer. Do not delay an SSE
event or control frame indefinitely to fill a batch. Preserve partial-write
handling, error propagation, shutdown, and producer backpressure.

If evidence warrants polling reads while writes are pending, model that as
a separate candidate. Never create overlapping completion writes or drop an
in-flight owned operation. Preserve progress of WINDOW_UPDATE, reset, and
shutdown traffic when the peer constrains the write side.

Test short writes, write and flush errors, delayed producers, small and zero
windows, trailers, empty DATA, concurrent large and small responses, and
shutdown during a blocked write. Validate position-dependent payloads.

## Part 3: stream scheduling and deadlines

Measure useful and redundant polls at 1, 16, and 256 configured streams,
including blocked streams and mixed body sizes. Compare eliminating temporary
storage with tracking readiness only if the latter's extra state is justified.
Do not replace `BTreeMap` merely because another structure is available.

The connection remains the owner of stream lifecycle and flow control.
Retain rotating service order and bounded work per turn. Any additional queue
must survive reset, stream completion, cancellation, and stream-ID reuse
rules without duplicate scheduling or stale references.

Deadline changes must preserve absolute header and continuation deadlines,
idle expiry, handler timeouts, stalled-body timeouts, and bounded draining.
Test wakeups from pending bodies, resumed credit, fairness, and cancellation.

## Part 4: validation ownership

Trace each candidate duplicate check through HTTP/1, HTTP/2, HTTP/3, and
direct uses of shared dispatch. Remove a repeated check only when every
caller retains equivalent boundary validation. Use a private validated value
only if it removes more complexity than it introduces.

Keep ambiguous-path rejection, authority validation, message-length rules,
decoded header budgets, Cookie normalization, and timeouts. Do not weaken
policy to match a lower-level benchmark. Where feasible, add an equivalent
policy peer fixture alongside the existing bare peer references.

Test malformed and duplicate headers, conflicting lengths, direct dispatch,
HEAD, CONNECT, and streaming errors at each affected boundary. Changes that
reach HTTP/3 also require vendored upstream and downstream qualification.

## Part 5: application size

Measure default HTTP/1, minimal SSE, gzip, TLS/HTTP/2 SSE, and full-feature
applications with identical compiler, lockfiles, features, and release settings.
Inspect feature activation, duplicated protocol helpers, generic expansion,
formatting, conversion, and error handling. Prefer deleting redundant work.

Keep benchmark and diagnostic dependencies outside the product graph.
Measure text, data, BSS, stripped file bytes, and unstripped file bytes
separately. Do not describe allocation traffic, live heap, and RSS as the
same metric. An improvement in one is not evidence for another.

## Protocol basis

Use RFC 9110 for HTTP semantics, RFC 9112 for HTTP/1.1, RFC 9113 for HTTP/2,
and RFC 7541 for HPACK. Include RFC 9931's updated CONNECT requirements;
the RFC Editor currently lists RFC 9931 as an update to RFC 9112.
Check current RFC status, updates, and applicable
errata before changing normative behavior. Follow the maintained HTTP/3
and SSE requirements when shared changes affect those features.

Record each altered invariant with a section-specific source and executable
regression check. Existing h2spec policy differences remain visible and must
not be converted into an unsupported full-compliance claim.

Primary references: [HTTP/1.1 and its update](https://www.rfc-editor.org/info/rfc9112/),
[HTTP semantics](https://www.rfc-editor.org/rfc/rfc9110.html),
[HTTP/2](https://www.rfc-editor.org/rfc/rfc9113.html), and
[HPACK](https://www.rfc-editor.org/rfc/rfc7541.html).

## Qualification and acceptance

Reuse `benches/build-frozen.py`, `benches/build-applications.py`, existing
load generators, and allocation fixtures. Extend those tools only where the
new mechanism requires an observable that they do not provide.

For every core candidate, compare baseline Maximus, candidate Maximus,
Hyper, and Axum with identical validated payloads, transport, CPU allocation,
compiler settings, warmup, and load. Retain the bare peer baseline and label
any equivalent-policy variant separately. Actix can supplement plaintext
HTTP/1 comparisons; do not claim that its fixture exercises TLS or HTTP/2.

Screen small text, typed routes, errors, 1 KiB and 128 KiB echo, SSE, and gzip
on each applicable transport. Qualify finalists with fixed offered load and
saturation, single and multiple workers, concurrent streams, connection
churn, and held idle connections. Use at least seven alternating matched
pairs for final baseline/candidate claims and rotating peer order.

Record source and executable hashes, workload hashes, build commands,
environment, all trial results, errors, drops, completed responses, CPU per
request, throughput, latency distributions, allocation traffic, and memory.
Keep diagnostics out of throughput binaries. Keep failed observations.
Run identical-binary controls before assigning small changes to source.

Accept a candidate only when a repeatable improvement appears in its intended
metric, all required correctness checks pass, and the broad suite reveals no
unresolved material regression. Do not hide an adverse workload behind an
aggregate score. A noisy environment cannot qualify a no-regression claim;
preserve the candidate separately and report it as unqualified if necessary.

Run `scripts/check.sh`, `scripts/test-linux.sh`, independent interoperability,
wire mutations, downstream compilation, relevant h2spec checks, and bounded
request, HTTP/1, HPACK, and HTTP/2 fuzz campaigns. Run upstream H3 checks when
shared code affects that path. Explicitly report unavailable privileged tests.

Final output is a lean product diff containing only retained improvements,
focused regression tests, reproducible benchmark tooling, and one report
that separates accepted, rejected, and unqualified candidates. Include
remaining limitations and tradeoffs. Do not claim universal leadership.

## Review decision

Review the private ownership and batching direction, the unchanged public
contracts, the acceptance rules, and the five-part scope before implementation.
The implementation plan will name exact private interfaces after this design
is approved. Execute sequentially in this chat by default; parallel agent
work is unnecessary unless explicitly selected.
