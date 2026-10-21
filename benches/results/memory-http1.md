# HTTP/1 memory-transport control

**Decision: diagnostic complete; strict performance qualification remains
UNQUALIFIED.** All 56 processes pass their protocol/output assertions, supplying
168 observations and 84 matched engine/case pairs. Timing results remain mixed,
including slower candidate samples and substantial variation in unchanged
Hyper/Axum controls. This probe establishes no HTTP/2, native TCP I/O, socket
capacity, latency-distribution, or RSS result.

The comparison exercises the H1 shared futures transport with an always-ready
memory reader/writer. Direct Maximus uses the `Kind` handler; direct Hyper uses
its service function. Dynamic Maximus and Axum include routing. These are
distinct service/runtime stacks, rather than equivalent measurements of only a
single handler function. The Maximus candidate contains borrowed error bodies,
optional H1 completion state and rejected-CONNECT closure. Its borrowed H2 timer
change is outside the exercised protocol path.

## Plan, validation, and timed work

Seven rounds alternate baseline/candidate order. Within each round, the four
engines rotate in the order Maximus, dynamic Maximus, Hyper, Axum; both variants
use the same rotation. Each process runs plain, static-error, and owned-error
cases in that fixed order. Every case warms up ten connections and times 1,000
connections, with 1,000 pipelined requests per connection: **one million timed
responses and 10,000 warmup responses per case**. Across all cases/engines/
variants/rounds, that is **168,000,000 timed and 1,680,000 warmup checked
responses**. All process exits are zero.

The final request on each connection asks for closure. Validation checks exactly
1,000 HTTP/1.1 responses, expected status/body/Content-Type/Content-Length,
absence of Transfer-Encoding, and Connection: close only on the final response,
with no trailing bytes. Warmup output is checked before timing. Measured outputs
are retained during timing, then fully checked and destroyed outside it. The
interval includes allocation, connection/protocol work, memory-wire writes and
output retention. Input bytes, handlers/services, and the output-pointer vector
are prepared outside the measured interval.

These are uninstrumented elapsed ns/request observations, not allocation counts.
The denominator is fixed and checked against source constants and every CSV.
Raw start/end timestamps and raw HTTP output bytes are not retained; the source
assertions and process outcomes provide the recorded correctness evidence.

## Every case and matched pair

Medians are ns/request across seven rounds. The final column lists **all seven
candidate/baseline percentage changes in round order**; positive values are
slower. The complete [trial CSV](memory-http1-trials.csv) and
[84-pair CSV](memory-http1-pairs.csv) preserve each raw value and difference.

| Engine | Case | Baseline median | Candidate median | Paired changes, rounds 0–6 |
| --- | --- | ---: | ---: | --- |
| Direct Maximus | Plain | 986.928 | 921.100 | +5.54, −6.67, −7.66, −10.00, −1.98, +2.95, −0.87% |
| Direct Maximus | Static error | 887.520 | 892.773 | −1.95, +4.05, −6.55, −2.95, −2.95, +4.84, +5.78% |
| Direct Maximus | Owned error | 880.164 | 883.999 | +8.23, −5.27, −3.02, +0.28, −4.43, +3.60, −0.42% |
| Dynamic Maximus | Plain | 1,116.019 | 1,088.664 | +3.63, −2.45, −0.30, −4.18, −2.95, −7.78, −2.25% |
| Dynamic Maximus | Static error | 1,038.902 | 1,025.730 | −2.73, −0.22, +4.07, +0.28, −2.34, −0.42, −1.78% |
| Dynamic Maximus | Owned error | 1,037.035 | 1,044.847 | −2.00, −2.01, +0.32, +6.53, −0.03, +12.13, −5.13% |
| Hyper control | Plain | 380.461 | 370.370 | −21.35, −8.00, +9.34, +1.34, +15.79, −14.66, +9.01% |
| Hyper control | Static error | 335.485 | 324.673 | −22.83, +1.52, +5.73, −7.06, +6.13, −6.09, −5.07% |
| Hyper control | Owned error | 343.776 | 331.487 | −6.23, −3.99, +2.80, −2.31, −7.33, +0.18, −1.64% |
| Axum control | Plain | 680.611 | 664.110 | −6.97, −1.51, +3.87, −7.69, +4.94, +2.15, −8.98% |
| Axum control | Static error | 642.107 | 640.295 | −0.17, +2.83, +11.85, −4.44, −0.28, −2.05, −9.31% |
| Axum control | Owned error | 638.096 | 634.021 | −3.72, −25.35, +7.25, +1.24, −1.13, +4.24, −1.67% |

Direct plain is faster in five pairs and dynamic plain in six, while their
remaining pairs are slower. Direct static/owned errors and dynamic owned error
have adverse medians. The unchanged comparator implementations retain their
large paired differences; these are not correction factors to subtract from
Maximus observations. No slower sample is discarded, and this cohort is not
pooled with socket or timer-mechanism measurements.

## Exact artifacts and reproduction

| Artifact | SHA-256 |
| --- | --- |
| Final baseline probe | `76d71330cfd9c995607ca5237c92eb419fdee0a19f57d57314e88e0350acddb9` |
| Final candidate probe | `30ff4f5201cfa0b4007f30efdd9e5178d0189a071b2177cf89ec38e7b7bee02d` |
| Final probe source | `c822c3c9549934a8ed4919b0a1c8ea650c779e2168f7fc7f19c012a923f06e21` |
| Probe Cargo.lock | `123021a824bb7c5c6a0eb7485e969aacb7d182e3b5c87ce87258e0477fe43e10` |
| Captured Rust image ID | `sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663` |

The product snapshots are `core-h2-baseline-source` and
`absolute-timer-borrowed-source`, identified by their respective manifests.
Final build logs are `memory-http1-core-h2-baseline-build.log` and
`memory-http1-absolute-timer-borrowed-build.log`. Both build/run metadata records show Rust
1.96.1, aarch64 Linux, and CPU 0 affinity. Final host Clippy passes. The probe
lock's 120 dependency packages match the frozen comparator lock by name,
version, registry source and checksum; only the standalone probe package is
extra. Versions include compio 0.19.2, futures-util 0.3.34, Hyper 1.12.0, Axum
0.8.9, and Tokio 1.53.3. No product or benchmark dependency is added.

The [offline analyzer](../analyze-memory-http1.py) checks source, lock, driver,
build and manifest hashes; execution metadata and exact commands; all 56
outcomes, process order, case order and denominators; and all 168 rows. The
[maintained evidence](memory-http1-evidence.json) embeds probe source,
Cargo.toml, Cargo.lock, product manifests and library patches, build logs,
build/run drivers, execution metadata, raw rows, pairs, summaries and hashes.
Pre-protocol-assertion build logs remain historical and are excluded from the
measured identities.

Root directly observed the campaign's zero exit in command session 28169 and
reports no owned competing load. The always-ready memory wire removes network
backpressure; different runtimes and linked layouts still affect timing. This
diagnostic neither explains earlier socket variation nor qualifies the current
[borrowed-timer candidate](borrowed-timer.md) under the strict no-regression
requirement.
