# Shared-path performance implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Qualify lean improvements to shared connection, body, policy, and application-size costs against baseline Maximus, Hyper, and Axum.

**Architecture:** Keep the current engines and public APIs. Test one private mechanism per frozen candidate, preserve baseline ownership and protocol invariants, and retain only changes supported by correctness, performance, and size measurements. Execute sequentially in this chat.

**Tech stack:** Rust 1.96, edition 2024, Compio 0.19.2, existing bytes and futures utilities, Python benchmark drivers, Docker Linux, k6, wrk.

**Spec:** `docs/superpowers/specs/2026-10-21-shared-path-performance-design.md`

## Global constraints

- Rust 1.96 and edition 2024 remain the floor.
- Do not add product dependencies, unsafe code, public configuration, or public API changes for this campaign.
- Do not copy competitor implementation code.
- Treat correctness and security regressions as unconditional rejection.
- Performance, memory, and size tradeoffs require an explicit review of the measured benefit and cost before retention.
- Keep benchmark and diagnostic dependencies outside the product graph.
- Do not hide an adverse workload behind an aggregate score.
- A noisy environment cannot qualify a no-regression claim; preserve the candidate separately and report it as unqualified if necessary.

## Review focus

- Fragmentation or cancellation during reads must not lose payload ownership or reset an absolute continuation deadline.
- A pending producer or blocked socket must not indefinitely delay streaming headers, control traffic, or shutdown.
- Reset, completion, and consumed-body credit must not schedule stale work or return flow-control credit twice.
- Direct shared-dispatch callers must retain validation even if a protocol parser validates the same property.
- Large transfers followed by idle connections must not retain unbounded working storage or corrupt the next request.

## Candidate rules

Each product task produces a candidate, which can be rejected independently.
Save its patch, source hashes, tests, and measurements before restoration.
Do not commit an unqualified optimization into the final product combination.
The campaign can legitimately finish with some areas investigated and rejected.

Use baseline `f49fdd6`. Freeze independent candidates against that product
baseline first. Build the selected combination only after individual screening,
then qualify the combination separately. Reusing diagnostics in both exports
must not change product source or competitor versions between variants.

### Task 1: reproducible baseline and broad comparison inputs

**Files:** `benches/workloads.py`, `benches/compare-final.py`, `benches/report-final.py`, `benches/test_report_final.py`, existing baseline fixture binaries, and ignored `artifacts/shared-path-performance/`.

**Interfaces:** Consume `SOCKET_CASES`, `response_keys(case, engines)`, and existing `run.py` options. Produce frozen source/build manifests and validated campaign JSON in the existing schema. Append workload cases without changing historical case indices.

- [ ] Test first: extend reporter fixtures with appended plain and TLS small/large echo and small-text cases. Assert that missing peer rows, duplicate cases, unequal body sizes, and failed completions produce `qualified_input == false` and no qualification statistics. Assert that complete new cases preserve all baseline, candidate, Hyper, and Axum rows.
- [ ] Run `python3 -m unittest discover -s benches -p 'test_*.py'`. Expected: new assertions fail until the appended case contract is supported, then all tests pass.
- [ ] Extend existing case tables and reporter inputs minimally. Use k6 for TLS, large-body, and fixed-rate cases. Preserve the current wrk contract for plain 1 KiB saturation. Record duration, worker count, offered rate, and source identities in each campaign.
- [ ] Run `scripts/check.sh` on the baseline, with full output retained. Expected: zero exit and no unexplained baseline failures. Run baseline Linux tests before interpreting native-I/O results.
- [ ] Export baseline sources with `git archive f49fdd6`, then overlay the identical benchmark-only fixtures in all exports. Build with `benches/build-frozen.py`; build application sizes with `benches/build-applications.py`.
- [ ] Run unchanged-binary comparisons with `benches/compare-final.py --baseline <baseline-dir> --candidate <baseline-dir> --no-peers`, and a separate rotating Hyper/Axum baseline campaign. Expected: complete checked responses, preserved raw rows, and a recorded variation envelope. Variation itself is not a source improvement.
- [ ] Commit only validated tooling and baseline metadata. Keep logs and executables ignored.

### Task 2: reusable HTTP/2 envelope storage

**Files:** `crates/http2/src/lib.rs`, focused tests in its test module, and the existing allocation diagnostic only if it needs an additional fixture.

**Interfaces:** Consume private `ReadWire`, `read_before`, `read_frame`, and `ReceivedFrame`. Candidate interface: `ReadWire::exact(&mut self, size: usize, buffer: Vec<u8>) -> io::Result<Vec<u8>>`. Carry envelope storage with the owned reader across `read_frame` calls. Keep `ReceivedFrame.payload: Vec<u8>` independently owned and preserve the external `serve` signatures.

- [ ] Test first: add a native partial-read fixture and a futures partial-read fixture that deliver back-to-back frames one byte at a time. After processing the first DATA frame, hold its payload across the second read and assert unchanged exact bytes, flags, stream IDs, and frame boundaries.
- [ ] Add a reuse assertion using the same nonempty envelope allocation across completed reads. Cover zero-length payload, EOF at every envelope offset, truncated payload, and continuation deadline retention. Existing payload ownership and flow-credit tests must remain green.
- [ ] Run `cargo test -p maximus-http2 --lib --locked`. Expected: the new reuse requirement fails on baseline; malformed input must still fail with the recorded protocol or I/O error.
- [ ] Implement the private buffer-transfer interface. Native reads must restrict the submitted buffer to exactly the requested length even when its capacity is larger. Futures reads resize initialized storage and use only the requested slice. Do not add a shared pool or copy DATA to recover scratch storage.
- [ ] Run the HTTP/2 unit suite and framework HTTP/2 integration tests. Expected: exact payload and lifecycle assertions pass. Run `scripts/test-linux.sh` for completion-I/O behavior.
- [ ] Freeze, measure allocations and application size, and compare complete sockets against baseline, Hyper, and Axum. Include unchanged HTTP/1 controls. Reject or archive the candidate if code growth, retention, CPU, or tails remain unjustified.
- [ ] Commit a retained candidate and tests, or restore product code and commit only the experiment's compact evidence and useful functional tests.

### Task 3: bounded production of output batches

**Files:** `crates/http2/src/lib.rs`, `crates/framework/tests/native_http2.rs`, and existing HTTP/1 response tests if a separately measured H1 candidate is justified.

**Interfaces:** Consume `Connection::poll(&mut self, cx: &mut Context<'_>, out: &mut Vec<u8>) -> Poll<()>`, existing `WriteWire::all`, and `Limits::io_buffer_bytes`. Keep external serving and body interfaces unchanged. Keep one in-flight owned write per connection.

- [ ] Test first: a ready streaming producer yielding several small DATA chunks must fill a bounded batch without repeated socket flushes. A producer that becomes pending after headers must still flush those headers promptly. Assert complete payload, frame order, and bounded work before returning to read/control selection.
- [ ] Add or extend partial-write tests to assert that a write or flush error propagates. Add mixed large/small stream, zero-window resume, trailer, and shutdown-during-blocked-write cases. Assert no stream starves and flow credit remains exact.
- [ ] Run the relevant unit and framework integration tests. Expected: the new ready-chunk batching assertion fails before implementation; existing error tests continue to pass.
- [ ] Implement repeated useful stream work only within the existing batch budget. Stop when a pass produces no output, the budget is reached, or all work is blocked. Preserve rotating order and avoid an always-ready empty polling loop. Treat overlapping read/write progress as a separate candidate only if measurements justify the additional state.
- [ ] Run native and TLS tests, independent clients, and response-checked plaintext/TLS socket comparisons with Hyper and Axum. Record DATA frames per batch and write/flush counts in diagnostics, never in throughput binaries.
- [ ] Freeze, compare footprint and broad workloads, then retain or restore the product candidate with its disposition recorded.

### Task 4: stream scheduling and deadline costs

**Files:** `crates/http2/src/lib.rs`, focused scheduler/body wakeup tests, and one isolated diagnostic under `benches/baselines/src/bin/` if existing probes cannot measure useful work.

**Interfaces:** Consume `Connection::poll`, `deadline(&self) -> Instant`, `expired(&mut self, out: &mut Vec<u8>) -> bool`, and existing stream lifecycle methods. Produce the same contracts and wire behavior. No public scheduler type is introduced.

- [ ] Add scenario tests for 1, 16, and 256 configured streams with sparse readiness, wraparound, stream reset during body completion, and delayed flow credit. Assert rotating progress, no duplicate DATA, no stale stream access, and exact reset/credit behavior.
- [ ] Measure current visits, lookups, allocations, wakeups, and deadlines with checks outside the timed region. Preserve the earlier rejected traversal model as historical evidence, not a replacement baseline.
- [ ] Select the smallest candidate supported by the measurements: eliminate temporary ID storage if a single traversal can preserve mutation order, otherwise test bounded reused scratch storage. Add a readiness queue only after demonstrating that scanning dominates its extra lifecycle cost. Do not replace the container alone.
- [ ] If deadline machinery dominates, test its smallest independently frozen change. Preserve absolute continuation deadlines and stalled-body timeout behavior. Record the exact selected representation and rejected alternatives in the execution ledger before editing.
- [ ] Run all relevant failure, cancellation, fairness, and timeout tests, then baseline/Hyper/Axum socket and size comparisons. Expected: no unresolved material regression. Keep scheduling and deadline candidates separate until qualification.
- [ ] Retain qualified changes or restore original product code and record why no candidate passed.

### Task 5: shared policy and application-size simplification

**Files:** `crates/core/src/policy.rs`, affected protocol callers, `crates/framework/Cargo.toml`, affected crate manifests, and corresponding core/framework tests. Change only files supported by a concrete duplication or feature-activation finding.

**Interfaces:** Preserve `dispatch_request<H: Handler>(Request, Rc<H>, Rc<Limits>) -> Response`, `content_length(&http::HeaderMap) -> Result<Option<u64>>`, `Limits`, and feature names. Protocol-specific validation remains at protocol boundaries. Direct dispatch must keep the complete documented policy.

- [ ] Trace the parse-to-dispatch callers and feature activation. Use `cargo tree --workspace -e features` and clean application section/symbol reports. Record a concrete duplicate calculation or unnecessary activated feature before choosing a candidate.
- [ ] Test direct dispatch and each affected wire entry with duplicate Host, invalid authority, conflicting Content-Length, decoded-header budgets, HEAD, CONNECT, and stalled bodies. Assert identical status, closure, handler invocation, and timeout behavior.
- [ ] For a candidate that removes duplicated work, write a failing regression test for the new internal contract before implementation. Do not add a public trusted-request bypass. If the safe proof requires more complexity than the work removed, reject the candidate.
- [ ] Simplify only the evidenced policy or dependency path. Validate no-default-feature, individual-feature, TLS/H2, and all-feature builds. Shared changes that reach H3 also run upstream H3 tests, downstream compilation, and H3 interop.
- [ ] Compare every core candidate against Hyper and Axum on applicable complete socket paths, including invalid-input behavior where policy changes. Compare all five native application footprints. A size-only candidate must keep runtime performance within the measured acceptance requirements.
- [ ] Retain or restore each candidate independently. Report unchanged dimensions and explicit costs, not only favorable section totals.

### Task 6: final combination, protocol qualification, and report

**Files:** selected product files and tests, `benches/results/2026-10-21-shared-path-performance.md`, compact machine-readable evidence, and relevant benchmark documentation links.

**Interfaces:** Consume qualified candidate patches and the unchanged baseline. Produce one final source identity and reproducible report with accepted, rejected, and unqualified dispositions for all five areas.

- [ ] Build the selected combination cleanly. Rerun at least seven alternating matched pairs on all applicable final cases with rotating Hyper/Axum peers and unchanged-binary controls. Run fixed-rate and saturation trials. Record single/multiple worker results, connection churn, and held-idle memory observations separately.
- [ ] Run `scripts/check.sh`, `scripts/test-linux.sh`, `scripts/test-interop-linux.sh`, `scripts/check-downstream.sh`, applicable h2spec policy gates, and four bounded fuzz campaigns using the existing commands in `docs/qualification.md`. Run vendored-H3 checks for any shared-path impact. Expected: zero unexpected failures; identify privileged or unavailable checks explicitly.
- [ ] Check current relevant RFC status and errata. Cite the exact semantics touched, including RFC 9110, RFC 9112 and RFC 9931, RFC 9113, RFC 7541, and SSE/H3 requirements where affected.
- [ ] Compare final minimal/full application section totals and stripped/unstripped files using identical builds. Compare allocation traffic, live requested heap where available, and observed RSS without conflating them.
- [ ] Run an independent whole-branch correctness and Rust-quality review, as required by the inline execution workflow. Address actionable findings and rerun affected checks. Do not ask the reviewer to repeat performance claims without inspecting evidence.
- [ ] Remove rejected product experiments from the final combination. Publish no unmeasured leadership or universal non-regression claim. Commit the qualified final report and product state, and present reviewable changes with remaining limitations. Do not merge or publish a release as part of this plan.

## Handoff

Execution method is already selected: sequential implementation in this chat.
Review this plan before implementation. Routine implementation decisions and
rejections are recorded in the execution ledger and do not require repeated
permission. An explicit tradeoff outside the acceptance rules still needs review.

## Execution outcome, 2026-10-21

This campaign is closed with no product optimization retained. The checklist
above is the original planned gate list, not an assertion that every gate
passed. The final [report](../../../benches/results/2026-10-21-shared-path-performance.md)
and machine evidence record actual outcomes and skipped/failed gates.

| Task | Outcome |
| --- | --- |
| 1 | Tooling retained and tested. Linux unchanged-binary performance control failed; native controls remain a separate cohort. |
| 2 | H2 envelope storage implemented, tested, frozen and compared; runtime unqualified, restored. |
| 3 | Bounded batching implemented, tested, frozen and compared; mixed results, restored. |
| 4 | Bounded scheduling scratch implemented, tested at 1/16/256 streams and compared; cost unjustified, restored. No deadline change qualified. |
| 5 | Policy representations implemented and tested; all rejected on independently sufficient application-size growth before dedicated socket cohorts. This gate was skipped, not passed. |
| 6 | Seven-pair H1 matrix and retention completed. Fixed-load and unbounded churn failed; separately labeled bounded churn and four-worker probes passed diagnostically. H1 runtime remains unqualified. Product restored, review fixes and final host/Linux/tooling checks passed. |

The final branch retains reproducible diagnostics and focused body-ownership
regression coverage. It establishes no overall throughput gain or leadership.
