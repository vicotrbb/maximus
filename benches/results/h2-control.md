# HTTP/2 control credit follows DATA progress

The control-frame counter previously grew for the lifetime of a connection. A
valid 1 MiB echo with a 1 KiB response window stopped after 557,056 bytes with
ENHANCE_YOUR_CALM on both macOS and Linux. The client sent a stream and connection
WINDOW_UPDATE after consuming each response DATA frame. A 65,535-byte window
completed the same transfer.

The correction repays two units of accumulated control debt per DATA byte,
covering separate stream and connection updates even with a one-byte window.
Saturating subtraction prevents banking credit. Empty DATA, padding, rejected
payloads and WINDOW_UPDATE alone earn no credit. Existing control increments,
request-based allowance, admission limits and flood termination remain.
A complete reset was rejected because a single DATA byte could erase an entire
control allowance. The permanent tiny-DATA flood test catches that alternative.

The implementation adds one net production line, no fields, allocations, timers,
dependencies, public APIs or unsafe code. END_STREAM Content-Length validation
moves into the existing pre-queue rejection check so malformed payloads earn no
credit. Existing stream-error and connection-credit behavior is preserved.

## Correctness

Both transfer regressions failed against the original implementation. The fixed
wire tests cover 1 MiB through a 1 KiB window, 8 KiB through a one-byte window,
valid upload progress interleaved with control traffic, tiny-DATA floods and
empty-DATA/WINDOW_UPDATE floods. Existing PING-only flood coverage remains.

- `scripts/check.sh`: 163 passing test executions, formatting, warnings-denied
  Clippy, feature checks, fuzz-target compilation and documentation passed.
- `scripts/test-linux.sh`: 141 passing test executions; one privileged BPF test
  remains ignored. Final wire tests separately passed 37/37 on macOS and Linux.
- `scripts/test-interop-linux.sh`: independent HTTP/1 plain/TLS, HTTP/2, HTTP/3,
  WebSocket and feature clients passed, including concurrent uploads and 2,062
  wire mutations/disconnects with post-stress health.
- Both h2spec modes passed the documented gate: 139/147 raw cases, five legacy
  policy differences, three closure races and 3/3 held-response reruns each.
- HTTP/2 libFuzzer/AddressSanitizer: 1,384,338 executions in 61 seconds with no
  crash, using `-max_total_time=60 -max_len=16384`.
- Independent source review found no actionable issues in the proportional
  credit policy, arithmetic bounds, length rejection or credit restoration.

The independent original socket reproduction now completes all 1,048,576 bytes.
These checks are finite evidence, not complete protocol certification.

## Local release comparison

The [evidence manifest](h2-control-evidence.json) identifies source hashes,
compiler, binaries and the temporary client script. The
[trial CSV](h2-control-trials.csv) retains every measurement; the
[summary](h2-control-summary.json) includes paired bootstrap intervals.

Both builds use Rust 1.96.0, thin LTO and one codegen unit on an Apple M5 Pro
macOS ARM64 host. The baseline is `9bb479f6730e6b160566f251c1eaf6a167969860`;
the candidate changes only the HTTP/2 implementation described above. The
all-feature `features` example runs one worker. A Python standard-library client
verifies HTTP/2 status and complete payloads over one local cleartext connection.
Each of 20 alternating pairs per endpoint has 200 checked warmup requests and
9,000 checked measured requests. All 80 trials completed, totaling 736,000 valid
responses including warmups. No build ran during the comparison.

| Metric, median of trials | Plaintext baseline | Plaintext candidate | 1 KiB echo baseline | 1 KiB echo candidate |
| --- | ---: | ---: | ---: | ---: |
| Responses/s | 37,769 | 37,256 | 32,311 | 32,720 |
| Accounted CPU microseconds/response | 14.368 | 14.481 | 17.624 | 17.504 |
| p50 microseconds | 19.896 | 19.792 | 23.438 | 23.438 |
| p99 microseconds | 93.417 | 101.751 | 110.417 | 101.646 |
| p99.9 microseconds | 303.208 | 281.750 | 294.729 | 311.520 |
| Sampled RSS KiB | 3,248 | 3,248 | 3,352 | 3,328 |

The median paired throughput changes are -0.07% for plaintext and +0.12% for echo.
Their 95% paired bootstrap intervals are [-2.34%, +1.74%] and [-0.53%, +1.19%].
These summarize paired percentage changes; they are not ratios of the medians
in the table. CPU includes startup, warmup, shutdown and one identical `ps`
invocation per trial. It is accounting evidence, not pure application CPU time.

Tails are mixed. The paired plaintext p99 interval is [-6.65%, +9.98%]; echo
p99.9 is [-1.44%, +15.42%]. These observations do not establish tail-latency
non-regression. Short, sequential localhost trials on an unpinned shared host
are not saturation measurements or fixed-offered-load comparisons. There is no
claim of universal performance parity, superiority or unchanged near-capacity
latency. The established improvement is completion of valid transfers that
previously failed, with bounded control credit and no additional connection state.

Both release executable files are 4,319,536 bytes; both executable `__text`
sections are 2,442,348 bytes. The candidate unwind section grows eight bytes
inside unchanged segment/file sizes. These are ARM64 measurements, not a claim
about every target or binary layout.

The existing allocation diagnostic was also built from each source at the same
path. Static/dynamic routing allocation counts match. Complete HTTP/2 socket
observations vary from 38.3870 to 38.5186 allocation/reallocation calls per request
and 38,897.8352 to 38,883.1952 requested bytes per request. Those process-wide
counts include executor scheduling and are single observations, not proof of
allocation equivalence or a speed estimate. The product diff introduces no
allocation operation.

## Reproduction

Run the commands in [qualification](../../docs/qualification.md). Focused tests
are `cargo test -p maximus-http2`; the independent clients and h2spec commands are
unchanged. Build the release example with
`cargo build --release -p maximus --all-features --example features` at each
source, saving the baseline binary before rebuilding the candidate at the same
path. The temporary comparator, allocation builds, binaries, logs and original
socket probe remain under ignored `artifacts/h2-progress/`. Compare their hashes
with the retained manifest. The comparator uses the workloads and counts above;
it is a local diagnostic, not a reusable capacity qualification runner.

The local campaign above does not include privileged BPF, a production soak or
broad performance qualification. The subsequent Linux merge gate is recorded below.


## Linux merge qualification: not qualified

The conditional merge campaign tested source revision
`42bd61155f35789a4a6205ff6cef40f2b5937b66`. Its
[frozen plan, build identities and environment](h2-control-linux-evidence.json)
and [complete trial rows](h2-control-linux-trials.csv) preserve the failed target.

The predefined gates require the paired 95% bootstrap interval to stay above
-3% for throughput, below +3% for CPU/request and below +5% for p99/p99.9, with
zero validity failures. Ten alternating baseline/candidate pairs were planned
per workload, with 30-second fixed-arrival stages for H2 plaintext and 1 KiB echo
at 5,000/s, H2 128 KiB echo at 500/s, and an H1 plaintext control at 5,000/s.
No additional repetitions are permitted to obtain a pass. A validity failure
stops the controller immediately.

Both release builds use the same source path, lockfile, Rust 1.96.1 compiler,
thin LTO and one codegen unit on the shared AMD Ryzen 7 5825U/Linux x86-64 host.
The server uses one explicitly selected io_uring worker on CPU 2. k6 1.6.0 runs
on CPUs 4/6, distinct physical cores; the controller uses CPU 0. Containers have
two-CPU quotas. The powersave governor and other node workloads remain
confounders; process affinity is not exclusive reservation of the host.
Both executable files are 3,044,344 bytes, with identical reported text, data
and BSS sizes. This does not substitute for runtime performance qualification.

All eight preflights passed, including verified TLS1.3/ALPN H2 and complete
large-body checks. The first measured **baseline** H2 plaintext trial completed
149,854 valid responses but dropped **145 scheduled arrivals**. It had zero
HTTP/check failures and exited 99 because the zero-drop threshold failed.
The controller stopped. No measured candidate trial or statistical gate was
completed, and the remaining workloads were not run. This is a failed offered
load for the unchanged baseline, not evidence of a candidate regression.

The result does **not** confirm performance non-regression and does not permit
merge under the agreed acceptance conditions. The PR remains a draft. No extra
trials were run to turn this result into a pass.

The exact latency sample implemented by the frozen client is completions after
five seconds through final drain of the 30-second arrival stage. The plan's
`[5,30]` shorthand does not impose an upper bound on completion time. Latency is
k6 HTTP request duration, excluding TLS setup, connection blocking and client
scheduling. These definitions limit interpretation; no tail acceptance is
claimed from this aborted campaign. Fixed-arrival completion rates also do not
measure maximum sustainable throughput.

Raw client summaries, logs, the harness lockfile and frozen scripts remain in
ignored `artifacts/h2-final/`. A 51-file result archive was downloaded and its
SHA-256 verified before the owned temporary namespace was deleted. Source and
lockfile hashes were checked against the PR implementation and baseline.


CI at implementation revision `42bd611` also remains mixed. The
[push run](https://github.com/vicotrbb/maximus/actions/runs/39316605873) passed both
jobs. The [PR run](https://github.com/vicotrbb/maximus/actions/runs/39316615029)
passed Linux but failed the standalone vendored h3 test
`tests::request::request_invalid_frame_after_data`: its client observed a local
H3_NO_ERROR close instead of the expected remote application-close error.
The vendored source is unchanged, and this standalone suite does not depend on
Maximus HTTP/2. The failure remains recorded; one passing run does not erase it.
