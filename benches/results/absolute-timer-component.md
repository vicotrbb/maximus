# Absolute H2 timer mechanism experiment

**Status: component-measured; HTTP performance unqualified.** A standalone probe
compares the current relative timer expression with direct absolute registration.
It does not measure an HTTP server, competing frameworks, or socket latency.
The implementation candidate remains separate from every earlier frozen source.

The H2 main loop already computes an absolute deadline. Its baseline passes
`deadline.saturating_duration_since(Instant::now())` to `compio::time::sleep`.
Compio then adds another clock reading to reconstruct an absolute target.
The new expression is `compio::time::sleep_until(deadline).fuse()`.
The [research note](../../docs/performance-research-2026-10-20.md#absolute-h2-timer-experiment)
traces the exact locked dependency source, expired-deadline behavior, and scope.

## Measured mechanism

The [component evidence](absolute-timer-component.json) records Rust 1.96.1,
Linux ARM64, io_uring, and CPU 0. It uses Compio 0.19.2 and compio-runtime
0.12.6. All probe dependency versions match the product lock, including libc
0.2.190. The probe is an isolated Cargo workspace and introduces no dependency
to the product or maintained benchmark workspace.

Each case creates a timer for a fixed deadline one hour ahead, polls it once,
checks `Pending`, and drops it. Timer registration and cancellation remain in
both expressions. The actual `Runtime::block_on` context supplies the waker.
Seven rounds alternate which expression runs first. Each mode warms up for
10,000 operations, then measures 2,000,000 operations. Both modes additionally
check that an already-expired deadline returns `Ready`.

| Round | Relative ns/operation | Absolute ns/operation | Paired time change |
| --- | ---: | ---: | ---: |
| 0 | 87.386 | 28.207 | −67.72% |
| 1 | 55.431 | 27.157 | −51.01% |
| 2 | 59.645 | 28.235 | −52.66% |
| 3 | 63.087 | 27.386 | −56.59% |
| 4 | 78.647 | 25.634 | −67.41% |
| 5 | 54.857 | 24.874 | −54.66% |
| 6 | 53.267 | 25.943 | −51.30% |

The relative median is 59.645 ns and the absolute median is 27.157 ns. All seven
pairs favor the absolute expression. The measured fused timer future is
120 → 96 bytes. All expired/future controls pass in the fourteen mode/round
cases. The evidence retains every row, binary hash, source/lock hashes, and
raw artifact identity.

## What remains unproved

The direct form avoids two explicit clock reads when this timer is polled,
along with relative conversion and reconstruction. The timer runtime still
performs its existing deadline check. Removing an async wrapper also changes
the future's representation, so the timing difference is not a separately
instrumented measurement of exactly two clock calls.

The fixture does not await timer expiry, transmit bytes, run request handlers,
or measure scheduling fairness. Its future size is not the size of an entire
HTTP connection future. It establishes no server allocation reduction,
throughput gain, p99 improvement, or regression-free behavior. Linked code
generation and unrelated HTTP paths still need matched controls.

The product change is frozen in `absolute-timer-source` with its own manifest
and patch. Independent source review found no deadline, drain, or feature-path
issue. Candidate correctness and fuzz checks are still running, and the planned
five-second HTTP/1 and TLS/H2 comparator cohorts have no results assigned here.
The earlier persistent-shutdown H2 rejection remains unchanged.

Reproduction sources and logs remain under
`artifacts/performance-2026-10-20/timer-probe/`. The recorded binary SHA-256 is
`8d586f2ec2dcdd1dd45624a133602637a192004ff162d57587b969ff517bfd15`.
The component evidence identifies its isolated manifest, full lockfile, source,
CSV, stderr, Clippy result, and final build log. Rebuilding or rerunning this
mechanism probe does not replace HTTP qualification.
