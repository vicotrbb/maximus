# Lean core and dynamic routing

This pass compares merged baseline `047d0e3` with `687d452`. It removes redundant state and registration work while preserving the framework's public APIs and features. After these measurements, the unused standalone `maximus-memory` crate was removed as well. Direct imports of its `Slab`, `Key` and `BufferPool` APIs are no longer supported; the framework had already stopped depending on it.

- TCP admission uses the live task collection's length instead of a separate generational slab containing only `()`.
- Empty bodies use empty `Bytes`, eliminating a third body variant and the nested buffered-body extraction match.
- Dynamic routes normalize their shape once. Binary insertion preserves the previous stable precedence order without sorting the entire table after each addition. Successful method lookup happens before allocating an `Allow` header.

At the measured commit, runtime source is 17 lines shorter. Regression tests and the extended existing allocation diagnostic add verification code. Validation and protocol rationale comments remain. The Linux normal dependency graph, counting unique package/version pairs including Maximus, drops from 65 to 64 packages for the minimal framework and from 129 to 128 with all features. No third-party versions changed or new dependencies were added.

The subsequent crate removal deletes another 175 lines across its implementation, tests and manifest, plus its lockfile entry. Minimal and all-feature framework dependency trees are identical before and after this removal. With Rust 1.96.0 on macOS ARM64, `cargo build -p maximus --all-features --release --example features --locked` produces identical executables before and after removal, including a forced rebuild of the example: SHA-256 `06bb3805c3a3635d6e9c6e5170e6e46670e9069ac75b36b51a06b29b815ade85`. This establishes no executable change for that configuration, not an additional speedup. Post-removal checks passed 152 local test executions and 130 Linux test executions, with one privileged BPF test skipped; downstream compatibility, independent protocol clients and the 2,062-case raw-wire stress check also passed. The three fewer tests in each suite belong to the deleted crate. The measurements below retain their original source identities and were not rerun for the deletion.

## Allocation and layout measurements

The [diagnostic rows](lean-core-allocations.csv) contain five alternating baseline/candidate pairs. Both builds use the candidate's diagnostic source and benchmark manifest, Rust 1.96.0, thin LTO and one codegen unit, on unpinned macOS ARM64. Allocation counts include all threads through the system allocator. Timing includes instrumentation and is an average, not a latency percentile or a production-throughput result.

| Operation | Baseline allocations | Candidate allocations | Baseline requested bytes | Candidate requested bytes | Median time, baseline → candidate |
| --- | ---: | ---: | ---: | ---: | ---: |
| Register and destroy a 64-route table | 8,433 | 389 | 400,524 | 20,280 | 269.69 → 33.25 µs |
| Dynamic GET | 2 | 2 | 625 | 609 | 126.83 → 130.01 ns |
| Dynamic POST after GET on the same path | 3 | 2 | 634 | 609 | 164.02 → 128.45 ns |
| Dynamic PATCH returning 405 | 5 | 5 | 698 | 698 | 209.06 → 200.96 ns |
| Static route returning a borrowed body | 0 | 0 | 0 | 0 | 44.74 → 45.32 ns |

Registration measures 100 complete tables per trial, each with 32 paths and GET/POST handlers, including pattern formatting and destruction. It uses 95.4% fewer allocations and requests 94.9% fewer allocation bytes. The observed median time is 87.7% lower. Dispatch measures 10,000 requests per case. Small timing differences, including the GET and static-route increases, do not establish a meaningful regression or improvement on this unpinned host. Socket diagnostic allocation counts include runtime/client activity and should not be compared with earlier Linux diagnostics.

On these ARM64 builds, `size_of::<Body>()` falls from 48 to 40 bytes, `Request` from 296 to 288, and `Response` from 160 to 152. Rust layout is not a stable ABI promise. The Linux comparison executable changes from 9,528,872 to 9,528,824 bytes: there is no material executable-size improvement to claim. The removed slab previously reserved two vectors totaling approximately 24 bytes per admitted connection slot on 64-bit targets, independently of the number of active connections.

## Matched socket load

The [trial rows](lean-core-trials.csv) and [build identities](lean-core-evidence.json) retain every completed run. These are regression checks on a shared Linux ARM64 Docker VM with three virtual CPUs and 4 GiB RAM, hosted by an Apple M5 Pro. Eleven unrelated service containers remained running. The server was pinned to VM CPU 0 and k6 to CPUs 1–2; the same placement applied to both variants. Rust 1.96.1 builds use thin LTO and one codegen unit. No compilation or other benchmark ran concurrently with the timed trials.

Each cell has three trials in alternating order, a separate three-second warmup, then ten seconds of measurement unless marked 30 seconds. Echo sends 1 KiB. HTTP/2 uses the same Rustls configuration, TLS 1.3 certificate and verified local CA for both variants. Handshake startup pressure is present even though k6's request-duration metric excludes the full handshake. Small responses contain `Hello, World!`.

| Workload | RPS baseline → candidate | p99 ms | p99.9 ms | CPU µs/response | RSS KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| HTTP/1 small, 1,000/s | 1000 → 1000 | 2.094 → 1.930 | 17.196 → 5.488 | 40.67 → 39.16 | 6996 → 6892 |
| HTTP/1 echo, 1,000/s | 1000 → 1000 | 1.721 → 1.911 | 5.251 → 5.350 | 44.42 → 46.52 | 7308 → 7232 |
| HTTP/2 TLS small, 1,000/s | 1000 → 1000 | 2.076 → 2.518 | 5.089 → 11.721 | 60.15 → 60.47 | 8560 → 8468 |
| HTTP/2 TLS echo, 1,000/s | 1000 → 1000 | 2.382 → 2.219 | 7.639 → 4.967 | 77.18 → 77.60 | 8404 → 8760 |
| HTTP/1 small, 64 VUs | 65218 → 65790 | 5.068 → 4.959 | 10.087 → 9.778 | 6.68 → 6.62 | 7340 → 6924 |
| HTTP/2 TLS small, 1,000/s, 30 seconds | 1000 → 1000 | 1.990 → 1.898 | 7.168 → 5.358 | 57.64 → 58.43 | 8520 → 8608 |

The 36 completed trial records contain 4,286,291 measured responses with zero HTTP failures and zero failed response checks. Thirty-five runs passed every threshold. One baseline 30-second trial dropped three scheduled iterations and exited 99; its 29,999 completed requests, metrics and failed target are retained. Every started iteration completed. The candidate passed all 18 of its trials.

The short HTTP/2 small-response trials worsened from 2.076 to 2.518 ms at p99 and from 5.089 to 11.721 ms at p99.9. A separate 30-second follow-up used the same binaries and offered load with reversed initial order. Its p99 medians were 1.990 → 1.898 ms and p99.9 medians 7.168 → 5.358 ms, but the dropped baseline iterations prevent treating it as an entirely clean matched-target campaign. Both sets are retained; they do not establish a stable tail-latency improvement or rule out a regression in another environment. HTTP/1 echo also had a higher candidate p99 in the short trials.

The saturation medians differ by less than 1% and trial ranges overlap. Client CPU accounting was about 188% against its two-CPU placement while server accounting was about 44%; this shared-VM setup does not establish server capacity. The defensible gains are fewer registration allocations, lower registration time in the diagnostic, a smaller body representation, and removal of redundant connection state. There is no general HTTP-throughput improvement claim.

CPU figures divide the server cgroup CPU delta over the broader measurement interval by completed responses. RSS is the post-run process-tree observation, not a continuous peak or an allocator limit. The CSV also preserves client/server CPU accounting percentages and the largest RSS observation. Short runs on this shared VM cannot establish production capacity, tail guarantees or framework rankings.

The load campaign used `benches/run.py` from `687d452` with a local staging adaptation: resolve `ROOT` to the checkout, keep output under `artifacts/lean-core/load`, bind `linux-before` or `linux-after` read-only at `/server` instead of the single benchmark-target volume, and execute `/server maximus` for either engine label. Reuse the generated certificate across TLS phases. The stored runner hash identifies this adaptation; workload and source hashes are recorded separately. To reproduce each variant with the standard runner, build its comparison workspace into `maximus-bench-target` and run:

```sh
python3 benches/run.py --engines maximus --paths /plaintext,/echo --rates 1000 --vus 64 --duration 10s --repeats 3 --client-cpus 1,2 --label plain
python3 benches/run.py --engines maximus --paths /plaintext,/echo --rates 1000 --vus 64 --duration 10s --repeats 3 --client-cpus 1,2 --protocol h2 --label h2
python3 benches/run.py --engines maximus --paths /plaintext --rates 0 --vus 64 --duration 10s --repeats 3 --client-cpus 1,2 --label saturation
python3 benches/run.py --engines maximus --paths /plaintext --rates 1000 --vus 64 --duration 30s --repeats 3 --client-cpus 1,2 --protocol h2 --label h2-tail
```

Use distinct output labels per source revision. Reproducing the paired order requires staging both server binaries as described above.

## Verification and reproduction

`cargo run --release --manifest-path benches/baselines/Cargo.toml --bin allocations` runs the diagnostic. For a baseline comparison, overlay the diagnostic source and benchmark manifest from `687d452` onto a separate checkout of `047d0e3`, retain its lockfile, and build both with the same compiler/profile. Timing starts after enabling counters and ends before output; network cases exclude their 100-request warmup. CSV `requests` means complete table operations for the registration row.

The [candidate CI run](https://github.com/vicotrbb/maximus/actions/runs/38967322583) passed both Linux and macOS jobs. Local checks recorded 155 passing test executions; the final Linux ARM64 suite recorded 133 passes and one opt-in privileged BPF test skipped. The full check script passes formatting, warnings-denied Clippy and documentation, all/minimal-feature tests, isolated feature and dependency checks, and compilation of all fuzz targets. Added behavior tests cover admission exhaustion and reuse under both connection and memory caps, buffered/terminal body limits, literal-path precedence, interleaved route shapes, renamed captures, explicit/fallback HEAD, duplicate shapes, and unchanged `Allow` ordering. They passed against both baseline and candidate. Independent clients exercise HTTP/1 plaintext/TLS, HTTP/2, HTTP/3, JSON, multipart uploads, SSE, compression, CORS, HEAD and WebSocket text/binary traffic. Raw-wire stress covers 2,062 mutations/disconnects with 16 clients and a healthy request afterward.

No new privileged BPF run, parser fuzz campaign, or full RFC-conformance claim is made by this pass. Previous protocol qualification remains documented separately. Raw logs, binaries and intermediate experiments remain ignored under `artifacts/`; the checked-in rows and identity file are the compact evidence for this pass.
