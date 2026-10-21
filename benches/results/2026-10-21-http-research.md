# HTTP optimization constraints and experiment shortlist

Research date: 2026-10-21. Inspected source: `df8f279fb7a2a61508527595c9d76e902644b281`.
This is source research, not benchmark evidence or acceptance of a product change.
No implementation was taken from Hyper, Axum, or another framework.

## Standards checked

The RFC Editor identifies [RFC 9110](https://www.rfc-editor.org/info/rfc9110/)
as HTTP semantics, [RFC 9112](https://www.rfc-editor.org/info/rfc9112/) as
HTTP/1.1, [RFC 9113](https://www.rfc-editor.org/info/rfc9113/) as HTTP/2,
and [RFC 7541](https://www.rfc-editor.org/info/rfc7541/) as HPACK. RFC 9113
obsoletes RFC 7540 and RFC 8740. RFC 7541 remains the HPACK reference; its
older publication date is not a reason to substitute a different wire format.

The current RFC 9112 information page lists
[RFC 9931](https://www.rfc-editor.org/rfc/rfc9931.html), published March 2026,
as an update. Its section 8 requires proxy servers to close the underlying
connection after rejecting CONNECT without processing another request.
Section 8 describes a narrowly qualified exception when the proxy knows that
the client waits for success before transmitting untrusted data. Retain the
existing conservative rejection behavior and registered 101/2xx transition
controls in every H1 candidate. This is separate from a blanket rejection of
all ordinary Upgrade requests.

These pages were checked during this investigation. This is not an exhaustive
audit of every extension or erratum. RFC text is immutable; its update and
errata records can change. Reported errata are not automatically approved
corrections. Review a relevant erratum's status before assigning normative
weight. The RFC Editor's
[verified-errata rendering](https://www.rfc-editor.org/rfc/inline-errata/rfc9112.html)
explicitly describes itself as informative.

## Contracts that optimizations must preserve

* H1 parsing operates on octets. Converting the whole input to Unicode before
  framing can create security differences. Preserve CRLF handling, rejection of
  malformed fields, duplicate/framing policy, header/body limits, and exact
  pipelined read-ahead. The current strict rejection policy must not silently
  become a tolerant parser during an optimization.
  [RFC 9112 sections 2.2 and 6.3](https://www.rfc-editor.org/rfc/rfc9112.html#section-2.2)
  define octet parsing and framing precedence. TE plus CL requires closure
  after the response even when the server elects to process using TE.

* Response serialization must preserve body suppression for HEAD and the
  status-specific cases, exact content length, chunk boundaries, trailers,
  HTTP/1.0 closure, and protocol transitions. Avoid replacing general framing
  with a fast path that assumes an ordinary 200 response.
  [RFC 9112 section 6.3](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3).

* Keep required Date generation. An origin server with a clock must generate
  Date on 2xx, 3xx, and 4xx responses; it may generate it on 1xx and 5xx.
  Content-Length has additional restrictions for HEAD, 304, 1xx, 204, and
  successful CONNECT. Existing caching or serialization work cannot assume
  the same header/body behavior for all statuses.
  [RFC 9110 sections 6.6.1 and 8.6](https://www.rfc-editor.org/rfc/rfc9110.html#section-6.6.1).

* H2 sends must obey both connection and stream windows. Non-DATA control
  frames do not consume those windows. Continue reading available control
  frames to avoid WINDOW_UPDATE deadlock. Window-update algorithms are
  implementation choices, but window accounting is a protocol obligation.
  [RFC 9113 sections 5.2 and 6.9](https://www.rfc-editor.org/rfc/rfc9113.html#section-5.2).
  H2 field names must be lowercase; field and pseudo-header validation belongs
  outside an HPACK table lookup optimization.
  [RFC 9113 section 8.2](https://www.rfc-editor.org/rfc/rfc9113.html#section-8.2).

* HPACK table order and indices are fixed. Preserve dynamic FIFO eviction,
  size limits, integer-overflow checks, header order, duplicates, opaque
  bytes, and decoded byte/count limits. Huffman EOS in content, padding longer
  than seven bits, and padding that is not an EOS prefix are decoding errors.
  [RFC 7541 sections 2.3, 4, 5.2 and Appendix A](https://www.rfc-editor.org/rfc/rfc7541.html#section-5.2).
  Borrowed dynamic output must remain valid after subsequent table eviction.

* URI decoding must preserve reserved separators and decode each input once.
  [RFC 3986 sections 2.2 and 2.4](https://www.rfc-editor.org/rfc/rfc3986.html#section-2.4)
  distinguish reserved and unreserved characters and explain why repeated
  decoding changes interpretation. The framework's stricter separator,
  control-character, UTF-8, and dot-segment rejection is an existing product
  contract, not a requirement to decode every path before routing.

## Three distinct experiments worth screening

These are hypotheses from Maximus source, not measured improvements. Screen
them independently for linked size before expensive end-to-end qualification.
Reject a candidate when its full evidence violates the user's performance,
size, or quality constraints.

| Direction | Source observation and minimal experiment | Required discriminating cases |
| --- | --- | --- |
| Use the existing status-code byte representation in H1 serialization | `crates/http1/src/lib.rs::write_response` formats `as_u16()` through `write!`. The locked `http` crate exposes `StatusCode::as_str()`. Compare composing the same protocol/status/reason/CRLF bytes through existing buffer operations. This uses a dependency API and adds no custom integer formatter. | All supported status values, unknown reason phrase, HTTP/1.0 and 1.1, HEAD, bodyless statuses, streaming/chunked responses, small output buffers, and registered transitions. Check minimal hello and full conformance linked text and data, because `as_str()` can retain a lookup table. |
| Stop HPACK static lookup after a matching name's contiguous group | `crates/http2/src/hpack.rs::encode` already combines exact/name-only searches in one pass, but still visits the rest of the table after a name-only match. Appendix A groups repeated names. Test ending the scan once that group ends, while preserving the first matching name index and exact-value precedence. Validate the grouping property rather than silently relying on it. | Every one of 61 table positions, exact and literal values, unknown names, empty/binary/long values, duplicates, prefixed output, all RFC examples, and random differential header lists. Unknown-name and late-table cases must not regress. Measure both codec CPU and H2 socket headers/echo with Hyper and Axum. |
| Decode percent hex digits through byte operations | `crates/core/src/request.rs::decode` converts each hexadecimal octet to `char` and calls `to_digit(16)`. A small ASCII-only byte conversion could remove generic character work while retaining the current borrowed path and output representation. Do not rewrite `validate_path` or its scan order in this experiment. | Exhaustive pairs of input octets after `%`, mixed-case hex, truncation, invalid encodings, encoded Unicode, controls, `%25`, `%252f`, `+` with both form modes, and duplicate query parameters. Include unescaped paths/queries as unchanged controls; encoded routes need an end-to-end workload to expose the actual affected path. |

The status-code API is documented in
[`http::StatusCode::as_str`](https://docs.rs/http/latest/http/status/struct.StatusCode.html#method.as_str).
The installed `http` source implements it using a compiled digit table; no
table or implementation needs copying into Maximus. The percent-decoder
candidate follows URI grammar rather than another framework's decoder.

## Avoid repeating disqualified work

The repository already records extensive negative results. The
[2026-10-20 investigation](../../docs/performance-research-2026-10-20.md)
rejects or leaves unqualified borrowed errors, empty-H1 completion state,
absolute timers, direct H1 parsed-head return/head-length borrowing,
persistent listeners, dynamic-router registration/metadata changes,
fused path validation, and live BTreeMap stream traversal. The
[owned DATA/path report](owned-data-and-paths.md) additionally rejects compact
Huffman nodes and incremental H1 chunk-line scanning. The
[HPACK lookup report](lean-io.md) already retains a single combined scan and
removes an earlier direct header-buffer encoder. Reimplementing these is
requalification of an existing idea, not a fresh discovery.

Historical reports describe their own exact source cohorts. Their passing
checks or favorable component numbers do not qualify a new binary. Keep
unchanged-binary controls, all adverse pairs, build identities, allocation
traffic, retained memory, linked text/data, and socket measurements separate.
Component timing can screen mechanisms; it cannot establish HTTP throughput,
tail latency, or a universal framework ranking. Hyper/Axum fixtures must use
the same workload, response semantics, transport, runtime configuration,
connection policy, and measurement boundaries where those can be matched;
remaining policy differences must be recorded explicitly.

## SSE candidate review

The [WHATWG SSE parsing and interpretation rules](https://html.spec.whatwg.org/multipage/server-sent-events.html#parsing-an-event-stream)
were checked on 2026-10-21 (page reports its last update as 2026-10-20).
This is a living standard. Relevant requirements:

* Streams are UTF-8. CRLF, lone CR, and lone LF each terminate one line.
* Each `data` field appends its value and LF. Dispatch removes the final LF;
  an empty data field still creates an empty-string event. A trailing input
  newline therefore needs another empty data field to preserve it. A final
  blank line is required; EOF alone does not dispatch.
* Parsing removes one ASCII space immediately after the field colon.
* An `id` containing NUL is ignored; empty `id` resets the remembered ID.
  CR/LF delimit lines. `event` has no corresponding NUL prohibition. Empty
  event type selects `message`.
* `retry` accepts ASCII decimal digits only and sets milliseconds.

Source review of `crates/core/src/sse.rs::Event::encode` finds that the current
candidate's `match_indices` loop skips LF already consumed with CR, handles
trailing separators and empty input, and preserves UTF-8 slice boundaries.
Its direct `writeln!` retains the previous integer `Duration::as_millis()`
representation. Rejecting NUL in event metadata remains stricter public API
policy. No new correctness defect was identified in those changed operations.
This is source review, not executed conformance or performance acceptance.

There is a separate pre-existing leading-space limitation: encoding `" hello"`
as `data: hello` makes the client receive `"hello"`. The same applies to
leading-space id/event values. The old encoder produces the same bytes.
Existing line-ending equivalence tests do not address browser interpretation
of those values; do not describe wire equivalence as complete SSE semantic
validation. This observation does not attribute a regression to the candidate.

The candidate removes normalized-data temporaries and the retry `format!`
temporary, but output still grows as a `String`. It neither guarantees one
allocation nor establishes lower retained memory. The installed, locked
`bytes` 1.12.1 conversion source (`src/bytes.rs`, `From<String>` and
`From<Vec<u8>>`, version fixed by [Cargo.lock](../../Cargo.lock))
transfers a String through Vec into Bytes. When length differs from capacity,
it preserves that allocation and allocates shared-owner metadata. Output growth
can therefore affect both allocation count and capacity retained while a slow
consumer holds the frame. Check short, large, newline-dense, retry, metadata,
and backpressured streams, including encoded bytes retained concurrently.
The input String also remains live while output is built. Removing intermediate
copies is a mechanism improvement; its timing and total-memory consequences
still need the matched measurements required above.

## Gzip negotiation: replace numeric ranking with acceptance state

Read-only review target: `crates/framework/src/compression.rs::accepts_gzip`.
No peer-framework implementation was consulted; no product build or benchmark
was run for this review.

The primary grammar is
[RFC 9110 section 12.4.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.4.2),
with field semantics in
[section 12.5.3](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.5.3).
Qvalues allow `0` or `1`, optionally followed by a decimal point and up to
three digits; digits following `1.` must be zero. Thus `0.` and `1.` are valid.
Omitted weight means one; zero excludes a coding. Coding and q parameter names
are case-insensitive. Explicit coding entries override wildcard applicability.
The grammar has one optional weight, not arbitrary or repeated parameters.
Error recovery for invalid syntax remains permitted by
[section 2.4](https://www.rfc-editor.org/rfc/rfc9110.html#section-2.4).

The current consumer never ranks two compressed representations: it only asks
whether the minimum stored gzip/wildcard weight is positive. For valid qvalues,
`min(a, b) > 0` is equivalent to `(a > 0) && (b > 0)`. Consequently two
`Option<bool>` states can replace two `Option<f32>` states without changing
the current duplicate-coding decision. This minimum/zero-veto duplicate policy
is an existing implementation choice; the cited RFC text does not prescribe
that particular tie-breaking algorithm for repeated instances of one coding.

Minimal proposal:

1. Identify gzip or wildcard before parsing its weight. Ignore unrelated
   codings as today. Consume all field lines and comma elements.
2. Parse only a small byte grammar: first byte `0` or `1`; optionally `.`
   followed by zero through three ASCII digits. The `1` branch permits only
   zero digits. Return positive for every valid `1` form and for a `0` form
   containing any nonzero fractional digit. No float or scaled integer is
   needed. Missing weight remains positive.
3. Combine repeated entries using boolean AND; retain `None` versus
   `Some(false)`. Finish with `gzip.or(wildcard).unwrap_or(false)` to preserve
   current absent-header/empty-header compression policy. Never use boolean
   OR between explicit and wildcard states. A wildcard acceptance cannot be
   returned early because a later explicit gzip entry can veto it.
4. Compare `q` case-insensitively. Keep whitespace handling deliberate: the
   weight grammar allows OWS around the semicolon, not between `q` and `=`.
   Treat invalid syntax separately from valid-input equivalence. A small
   conservative policy is to make that coding occurrence unacceptable when
   its weight or parameter structure is malformed, including unsupported or
   duplicate parameters; document that as a policy change from today's
   permissive skip/find behavior, not a performance-preserving identity.

| Input | Current source behavior | Proposed strict-parser result |
| --- | --- | --- |
| `gzip`, `GZip;q=1`, `gzip;q=1.000`, `gzip;q=1.` | Accept | Accept |
| `gzip;q=0`, `gzip;q=0.`, `gzip;q=0.000` | Reject | Reject |
| `gzip;q=0.001`, `gzip;q=0.1` | Accept | Accept |
| `gzip;Q=0` | Accepts because lowercase-prefix search misses the parameter | Reject; concrete valid-input correction |
| `gzip;q=0, *;q=1` | Reject | Reject |
| `*;q=0, gzip;q=0.1` | Accept | Accept |
| `gzip;q=1, gzip;q=0` across either one or multiple field lines | Reject | Reject under preserved zero-veto policy |
| `gzip;q=+0.5`, `gzip;q=.5`, `gzip;q=1e0`, `gzip;q=0.0001` | Float syntax can accept | Malformed; reject occurrence under proposed policy |
| `gzip;q=NaN`, `gzip;q=inf`, `gzip;q=-1`, `gzip;q=1.001` | Reject after numeric range filter | Malformed; reject occurrence |
| `gzip;level=1`, `gzip;q=1;q=0` | Ignores unknown parameter / uses first lowercase q | Malformed; explicit policy required |

The uppercase-Q case is a concrete pre-existing valid-input negotiation bug.
Permissive float syntax and ignored extra parameters are observable grammar
differences, but do not alone prove a standards violation because recovery
is permitted. A strict parser changes those invalid-input outcomes and needs
separate controls. It must also reject signed zero and excessive precision
rather than accidentally treating them as valid zero representations.

Preserve the surrounding compression policy during this experiment: ranges,
existing content encoding/range, no-transform, bounded buffered bodies, and
only keeping gzip when it is smaller. Absent Accept-Encoding permits compression
but does not require the server to compress, so retaining the current false
default is not itself a bug. This proposal removes unneeded numeric work;
linked size and performance remain unmeasured. Valid-input differential cases,
malformed-input policy cases, multiple lines, reordered explicit/wildcard lists,
and long unrelated-coding lists should be checked before qualification.

## Implemented gzip policy

The experiment uses the boolean state and strict decimal qvalue parser above.
It fixes uppercase `Q`. It preserves the existing recovery that skips unknown
parameters and uses the first q parameter, including when later parameters
repeat q. Only malformed *weight values* change from float recovery to rejection.
The implementation does not claim to validate the entire Accept-Encoding field.
The surrounding range, no-transform, buffered-size, and compression-benefit
policies remain in place. Measurements and retention decisions are recorded
in the performance report separately from this standards research.
