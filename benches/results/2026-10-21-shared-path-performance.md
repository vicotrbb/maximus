# Shared-path performance campaign, 2026-10-21

## PR #12 revision, 2026-10-22

PR #12 now includes the HTTP/1 single-buffer implementation as a concrete
memory/size optimization for review. It removes the separate read scratch
buffer, reads into initialized parser storage and advances an unread range
instead of shifting bytes after each consumed prefix. Production code matches
the previously measured candidate. The ranged-read test now permits legal
short TCP reads. No dependency or public API is added.

This revision changes the proposed product scope; it does not qualify the
unresolved runtime observations. The measured benefit is 19.71% lower held
process RSS in the tested scenario and smaller linked sections in five builds.
Plain SSE throughput/CPU and intermittent TLS latency remain explicit costs
or uncertainties. These results support reviewing a memory/size tradeoff;
they do not establish an overall speedup or a bounded runtime regression.
The original campaign disposition and baseline identity below record the
state before this revision. Fresh `scripts/check.sh` and clean Linux workspace/all-feature tests passed
for this PR revision, including native/TLS body ownership, direct-buffer
boundaries and explicit io_uring. Check-log hashes are recorded in the
machine-readable companion. Runtime qualification remains unresolved.

The original campaign and restored-branch verification are complete.
The HTTP/1 receive-storage experiment remains runtime-unqualified and is not
retained: smaller applications and promising retention do not resolve the
adverse timing observations. No framework-leadership claim is made.

## Product scope

Baseline is `f49fdd6`, before this campaign. The HTTP/1 candidate reads directly
into initialized parser storage, tracks the unread range, and compacts only
when another read needs space. It replaces the separate read scratch buffer
and avoids shifting pipelined bytes after every consumed prefix. Native owned
reads and the futures/TLS adapter receive the same explicit writable range.
Only received bytes become input. Upgrade handoff copies the unread prefix
and releases initialized parser storage before invoking the callback.

Public APIs, dependencies, resource limits, framing, dispatch policy, and
protocol protections remain unchanged. No product unsafe code was added.
Competitor implementation code was not copied.

| Investigated area | Candidate and disposition |
| --- | --- |
| Receive storage | HTTP/1 single-buffer candidate is runtime-unqualified after the seven-pair main matrix and is not selected for retention. H2 envelope reuse is preserved as unqualified: smaller linked TLS/full applications, but no established broad runtime benefit. |
| Output batching | H2 ready-stream passes bounded by bytes and four scheduling passes. SSE improved in one screen, while the large-upload screen was adverse. Three upload pairs did not reproduce the doubled p99. Broad retention remains unqualified. |
| Scheduling/deadlines | Reused bounded stream-ID scratch avoids temporary allocation, but adds retained capacity and grows the TLS application. Mixed route timings do not qualify that cost. No deadline or container replacement is retained. |
| Validation ownership | Typed authority experiments remove one absolute-form allocation but every tested representation grows at least one application. All are rejected. Direct-dispatch validation remains intact. |
| Application size | Measure five applications, section totals, and stripped/unstripped copies separately. No product feature/dependency change was justified. |

Rejected representations include boxed H1 storage, H1 storage without shared
prefix consumption, a separate H2 batching wrapper, and multiple policy helper
boundaries. Frozen exports, patches, manifests and logs remain under the local
ignored `artifacts/shared-path-performance/` directory.

## Experimental controls

The Linux comparison uses the existing frozen builder, immutable binaries,
identical `/work` paths, pinned Rust image, and local-package cache invalidation.
A three-pair unchanged-binary screen had a request timeout and long sampling
stalls. Its report correctly withheld qualification statistics. It cannot
establish a Linux throughput or non-regression result.

The native fallback is a separate macOS ARM64/Poll cohort. It is not a Linux
io_uring or Linux epoll measurement. It uses identical checked k6 workloads
for every framework, fresh servers, 32 VUs, three seconds of server warmup,
and the recorded measurement duration. The final main matrix is a closed-loop
32-VU target; the `saturation` artifact label does not certify peak throughput
or quota saturation. Hyper 1.12.0, Axum 0.8.9, Tokio 1.53.3,
Compio 0.19.2 and rustls 0.23.46 remain pinned by the comparison lockfile.
Peer fixtures retain their existing bare policies. No equivalent-policy or
security-superiority inference is made from valid-input load results.

Native Rust is 1.96.0. The pinned k6 v1.6.0 module was built with Go 1.27.3 so
Darwin can honor a process-local CA file. TLS certificate verification remains
enabled. The controller uses an empty client configuration and disables usage
reporting. No global certificate store or unrelated service was changed.
The official Go change is documented in
[crypto/x509 platform CA overrides](https://go.googlesource.com/go/+/4bf23b51b8efaf337b06e0e1b7f90b1fb9f45daa%5E%21/).

All warmup and measured iterations must complete exactly one checked request;
failures, mismatched counts, drops and missing peers invalidate inputs. An
input-valid report is not itself performance acceptance. Native CPU/request
covers complete server lifetime, including startup/warmup/cleanup, divided
by validated warmup plus measured requests. RSS is process observation, not
live requested heap. Native CPU affinity and quotas are uncontrolled; native
and container metrics must not be pooled.

Three native unchanged-binary pairs had no failed requests. Median paired
RPS changes were -6.31% on plaintext, -1.57% on typed routing, +3.91% on errors,
and +1.51% on H2 128 KiB echo. Individual pair changes included both signs;
p99 variation reached about 20%. Small timing changes require caution.

Individual candidate screens used one pair with Hyper/Axum rows and are
explicitly diagnostic. The adverse batching upload screen had p99 +101.88%;
its three-pair follow-up had median p99 +2.41%, RPS -3.69%, and CPU/request
+0.72%. Preserve both observations rather than selecting the favorable run.

## Seven-pair main matrix

All 154 trials and 532 response rows passed the independent input checks.
The following medians are macOS ARM64/Poll observations at a closed-loop
32-VU target, three seconds warmup and three seconds measurement per path.
They do not establish peak throughput. Each framework column contains seven
observations per path. Hyper and Axum remain bare peer fixtures.
Percentage columns are medians of the seven matched candidate/baseline changes,
which can differ from changes between unpaired column medians.

| Transport / workload | Baseline RPS | H1 experiment RPS | Hyper RPS | Axum RPS | Paired RPS % | Paired p99 % | Paired CPU/request % |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| plain /plaintext | 77,771 | 72,276 | 79,948 | 80,196 | -5.50 | +3.69 | +6.26 |
| plain /users/42 | 72,888 | 71,721 | 79,300 | 82,256 | -1.60 | +3.93 | +1.24 |
| plain /error | 77,353 | 69,757 | 81,345 | 78,548 | -3.35 | +0.61 | +2.15 |
| plain /sse | 57,823 | 52,750 | 75,916 | 68,951 | -9.45 | +8.47 | +7.21 |
| h1 /sse | 50,741 | 51,951 | 76,622 | 69,740 | -2.58 | +3.41 | +0.87 |
| h2 /sse | 43,225 | 46,710 | 69,052 | 68,405 | +5.73 | -7.99 | -9.20 |
| h2 /echo 128 KiB | 4,404 | 4,594 | 5,777 | 5,371 | +6.09 | -4.40 | -2.14 |
| plain /compress | 41,961 | 38,937 | 45,042 | 44,030 | -3.28 | +6.69 | +0.96 |
| plain /echo 1 KiB | 73,326 | 66,377 | 75,132 | 75,015 | -5.63 | +9.26 | +1.40 |
| plain /echo 128 KiB | 12,355 | 13,110 | 17,462 | 17,517 | +4.23 | +1.50 | -4.76 |
| h1 /plaintext | 75,551 | 68,237 | 82,531 | 78,514 | -5.50 | +4.07 | +4.25 |
| h1 /users/42 | 73,221 | 70,824 | 81,274 | 77,761 | -1.64 | +3.14 | +2.83 |
| h1 /error | 69,051 | 75,391 | 79,026 | 74,795 | +0.70 | -0.63 | -1.18 |
| h1 /echo 1 KiB | 64,425 | 70,926 | 72,611 | 69,265 | +7.89 | -9.47 | -4.66 |
| h2 /plaintext | 53,000 | 55,973 | 69,859 | 74,882 | +0.13 | +4.97 | -1.63 |
| h2 /users/42 | 53,826 | 52,468 | 71,288 | 76,407 | -4.24 | +5.80 | -0.26 |
| h2 /error | 56,400 | 53,625 | 69,726 | 76,412 | -3.03 | +4.32 | +2.10 |
| h2 /echo 1 KiB | 47,643 | 46,647 | 58,779 | 62,634 | -1.93 | +1.49 | +1.41 |
| h1 /echo 128 KiB | 6,918 | 7,419 | 9,648 | 9,680 | +2.53 | -0.94 | -3.27 |

Plain SSE has adverse median throughput, p99 and CPU/request observations.
TLS HTTP/1 plaintext includes individual throughput changes of -49.45% and
-54.15%, with p99 changes of +187.13% and +424.31%. Some large-transfer
spikes also affect peers, but other candidate spikes lack that explanation.
No observation is dropped or normalized away. These inputs are valid, while
source attribution and broad runtime acceptance remain unresolved.

The approved design requires no unresolved material regression. Accordingly,
the H1 experiment is preserved separately and is not retained. This is an
unqualified result, not a demonstrated source regression or an accepted
performance tradeoff.

## Supplemental probes

The 1,000-RPS fixed-load campaign stopped during baseline TLS HTTP/1 warmup
(case 8, 1 KiB echo): 19 dropped iterations and client exit 99. The 2,982
completed warmup requests had no response or network errors. The strict reporter
withheld qualification statistics because the full campaign was incomplete.
This is an invalid load-input result, not a candidate regression. It is preserved
without a replacement rate or selected retry. The four-worker probe subsequently passed all six trials and 44 response rows;
its single pair is diagnostic. Unbounded churn failed during baseline warmup
with 22 failed checks caused by client-side `can't assign requested address`
connection errors. The immediately following retention probe hit the same
address error before collecting an observation. Both failed campaigns remain
preserved. After a port-recovery interval, a separately labeled 250-RPS churn
probe and seven-pair retention probe run with fresh output directories. Their
results do not replace the failed observations. The 250-RPS churn probe passed
all six trials and 20 response rows, with no drops or failed checks. Its single
pair is diagnostic, not a peak churn or final comparative performance claim.

## Size and retention evidence

Exact experimental builds reproduce the initial linked-section reductions. Each
application is built independently with identical feature sets, compiler and
source path. Stripping uses copies; original executables remain immutable.

| Application | Baseline sections | Finalist sections | Change | Stripped baseline → finalist |
| --- | ---: | ---: | ---: | ---: |
| Hello/default | 858,049 | 854,025 | -4,024 | 922,400 → 856,864 |
| SSE | 840,465 | 836,441 | -4,024 | 856,808 → 856,808 |
| Compression | 904,744 | 896,608 | -8,136 | 922,344 → 922,344 |
| TLS/H2 SSE | 2,201,764 | 2,189,476 | -12,288 | 2,233,128 → 2,233,128 |
| All features | 3,697,858 | 3,685,930 | -11,928 | 3,744,152 → 3,744,152 |

All values are bytes. Section totals are text + data + bss. File lengths can
remain unchanged despite smaller sections; do not turn section reductions
into an equivalent stripped-file reduction. Unstripped files and exact
executable hashes are retained in the footprint logs.

The recovered seven-round retention probe completed all 28 framework trials.
Each held 256 connections after a 128 KiB echo and a following small request,
took three RSS snapshots, then checked another request on every original
socket. Median per-trial held RSS was 20,704 KiB baseline, 16,624 KiB H1
experiment (-19.71%), 31,232 KiB bare Hyper, and 31,280 KiB bare Axum.
These are process-RSS observations, not live requested heap, continuous peak,
or allocator/admission equivalence. The earlier three-round screen also
showed lower candidate RSS; its observations remain preserved. The repeated
byte payload used for these timing/retention cohorts does not distinguish
reordered windows; separate position-dependent payload
and retained-frame correctness checks close that coverage gap. Allocation-traffic probes were mixed and
do not establish a steady allocation improvement or live-heap equivalence.

## Correctness and standards

The combined experimental source passed host checks, Linux tests, downstream
H2/H3 compilation, independent verified TLS/H2/H3 clients, concurrent uploads,
WebSockets, 2,062 wire mutations/disconnects and post-stress health. The final
H1-only source separately passed the host check, downstream build, both h2spec
gates, fresh-clean Linux tests and independent interoperability. Fresh Linux tests explicitly
invalidate local packages after switching exports at the common `/work` path;
older source timestamps must not silently reuse another variant's objects.

Both h2spec policy gates passed on the combined source. Original runs were
139/147, with five documented closed-stream policy differences and three
response-closure races; all three held-response reruns passed. This is not
147/147 legacy compliance. Four 61-second fuzz smoke campaigns completed
without findings on the combined source: request 876,133, H1 1,583,676, HPACK
226,365, H2 720,124 executions. Final H1-only smoke campaigns also completed without findings (61s each):
request 4,573,988, H1 1,277,384, HPACK 1,852,762, H2 1,911,756 executions.

The normative review preserves [RFC 9112 message parsing and body framing](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3),
including received-byte boundaries, CRLF, chunk framing and pipelined bytes.
[RFC 9931 section 8](https://www.rfc-editor.org/rfc/rfc9931.html#section-8)
updates HTTP/1 CONNECT rejection: close the connection without processing a
following request. That baseline protection remains unchanged. URI, authority,
HEAD and length policy remain under [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html).

H2 experiments preserve [RFC 9113 framing, state and flow control](https://www.rfc-editor.org/rfc/rfc9113.html),
absolute continuation deadlines, payload ownership, reset behavior and exact
consumed-body credit. [Verified erratum 7013](https://errata.rfc-editor.org/errata/eid7013)
corrects the zero-length end-stream frame name to DATA; the implementation
already uses DATA. URI-mapping erratum 9175 is reported, not verified; this
campaign changes no URI translation. [HPACK](https://www.rfc-editor.org/rfc/rfc7541.html)
and its held-for-update integer/static-table errata introduce no decoder
change here. Current official errata pages for 9110, 9112, 9931, 9113 and 7541
were fetched and retained with IDs/statuses on 2026-10-21. No whole-RFC or
all-platform certification is inferred from these checks.

## Review and final verification

The single independent whole-branch review found no concrete H1 correctness
defect and confirmed the unqualified disposition. It identified a P2 coverage
gap and a P3 peer sample-count description error. Both are addressed in the
final fix pass. New native and verified-TLS tests send a position-dependent
128 KiB body at 1 KiB and 16 KiB I/O capacities, retain the first delivered
frame while later frames are read, compare the complete exact body, then
validate a small response on the same stream. Both tests pass on baseline
and the unqualified H1 experiment. Both fail under a temporary repeated
256-byte receive-window mutation. The mutation and experimental product
source are restored before final checks. The original timed payloads remain
unchanged. The reviewer assessed source and recorded evidence read-only; it
did not independently execute tests or certify the subsequent fix.

Final restored-branch `scripts/check.sh` passed, including formatting,
warnings-denied Clippy, workspace and feature-matrix tests, fuzz compilation
and documentation. Fresh-clean Linux workspace/all-feature tests passed,
including both new integrity tests and `explicit_uring_is_used`. All 13 Python
benchmark-tool tests passed. All 41 product Rust source/crate-manifest files
match `f49fdd6` byte-for-byte. No product optimization or dependency change
is retained. Earlier candidate interop, downstream, h2spec and bounded fuzz
evidence remains separately labeled above; these checks do not certify
universal correctness or platform performance.

## Execution rulings

| Decision | Reason, cost, and verification |
| --- | --- |
| Use the existing framework `http2.rs` test file | The plan named a nonexistent file. Wrong coverage would leave a gap; private and integration suites ran instead. |
| Restrict reusable completion buffers to the requested range | Extra capacity must not consume a following frame. The capacity-64/nine-byte envelope test and independent held payloads check this boundary. H2 remains unqualified. |
| Prepare the policy prototype while frozen builds run | Independent source exports allow useful work without timing/build overlap. Mixing identities would invalidate results; source/build manifests preserve each variant. |
| Stop the owned concurrent Linux baseline build under VM pressure | Preserve user workloads and retry the incomplete baseline serially. The cost was build time; the fresh-clean baseline retry later passed. |
| Test bounded reused stream-ID scratch | Preserve traversal/removal order instead of replacing the container. It costs a Vec field and retained peak capacity; insufficient broad evidence means no retention. |
| Test an outlined authority helper derivative | Check whether sharing avoids linked growth. It did not; the extra experiment is preserved and no attribute reaches the product. |
| Add the independently frozen H1 storage experiment | A concrete scratch copy and prefix shifts justified testing within connection memory scope. Stale initialized bytes or cancellation/reuse mistakes would be serious; adapter, framing, deadline, interop and wire checks passed. |
| Keep native fallback results separate | Linux controls failed; native Poll provides independent evidence. The cost is that Linux io_uring performance remains unqualified. |
| Continue independent probes after the fixed-load control fails | The 1,000-RPS baseline TLS warmup dropped 19 iterations; the controller and reporter rejected the campaign. Preserve its raw failure and run worker/churn/retention diagnostics separately. Fixed-load acceptance remains unqualified. |
| Reject policy representations before dedicated socket cohorts | Application-size growth independently disqualified every representation. Correctness, allocation and size evidence is preserved, but the plan's every-candidate socket gate was not completed for these discarded variants. No runtime conclusion is assigned to them. |

Initial controller smoke probes also exposed an unsupported Darwin CA setting,
a failed fallback-root wrapper, and an occupied hardcoded gzip port. Their raw
failures remain in artifacts. The final cohort uses the corrected process-local
CA setup and identical address/worker-aware fixtures on both sides; failed
probes are not promoted to performance observations.

## Final disposition

No product optimization qualifies for retention. The seven-pair H1 experiment
reduces application sections and held process RSS, but unresolved adverse
runtime observations fail the approved broad acceptance rule. H2 experiments
remain unqualified; policy representations are rejected on size. The final
product implementation has been restored to the baseline. Focused regression
coverage and validated benchmark tooling are retained. The review coverage fix and final host/Linux/tooling verification passed. No universal non-regression or
framework-leadership claim is made.

## Reproduction and evidence locations

The machine-readable companion is
[`2026-10-21-shared-path-performance.json`](2026-10-21-shared-path-performance.json).
It contains paired main metrics, bare peer medians, incomplete-campaign
diagnostics, RSS rows, product source identity, and hashes of local evidence.
Ignored raw logs, frozen source exports, immutable executable manifests, failed
probes and candidate patches are under `artifacts/shared-path-performance/`.
They remain local; no large generated binary or benchmark dependency enters
the product repository. Key files are `final-native-commands.json`,
`final-h1-native-saturation-report.json`, `final-h1-native-fixed-report.json`,
`final-h1-native-workers-report.json`, `final-h1-native-churn-report.json`,
`final-h1-native-churn-bounded-report.json`,
`final-h1-native-idle-recovered/results.json`,
`final-restored-product-identity.json`, `final-restored-host-check.log`,
`final-restored-linux-tests.log`, `final-restored-benchmark-tests.log`, and
`final-body-integrity-{green,candidate,mutation}.log`.

Recreate frozen executables with `build-native.py` or `build-frozen.py`, using
the recorded manifests and identical benchmark fixture overlays. Run the
recorded commands with fresh output directories. `run-native.py`,
`report-final.py` and `idle-native.py` preserve their response contracts;
`qualified_input` certifies input completeness only. Reproduction on a
different host is a new cohort and cannot silently replace these results.
