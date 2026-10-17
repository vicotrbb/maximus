# Static route metadata and HPACK trie capacity

This change retains two small optimizations against merged baseline `50f81bfb92f2fc375ec093e491098b432b78c727`: build static routes' `Allow` strings during macro expansion, and reserve the fixed Huffman trie's capacity once. The production diff adds ten lines across two existing modules. It adds no dependencies, public APIs, configuration, fields or unsafe code; Compio remains.

## Direct savings

| Measurement | Baseline | Retained change |
| --- | ---: | ---: |
| Successful static POST dispatch, allocations | 1 | 0 |
| Successful static PUT dispatch, allocations/reallocations | 2 | 0 |
| Static 405 response, allocations/reallocations | 6 | 2 |
| First Huffman decode, allocations/reallocations | 12 | 3 |
| First Huffman decode, cumulative requested bytes | 49,104 | 12,336 |
| Shared trie node-capacity bytes | 24,576 | 12,312 |
| Linux x86-64 comparison executable, bytes | 9,884,328 | 9,883,216 |
| Linux executable text, bytes | 7,867,007 | 7,866,023 |
| macOS ARM64 192-route fixture executable, bytes | 608,624 | 575,280 |

The static fixture registers GET/POST/PUT on the same path; GET and HEAD remain allocation-free. The checked-in allocation diagnostic now asserts zero successful-dispatch allocations for all four methods. Request construction uses an empty body and does not allocate in this fixture. A 405 still allocates its response header map. Exact `Allow` ordering, explicit/fallback HEAD, literal precedence, renamed captures and empty route sets are tested.

The Huffman saving is **once per process**, shared across connections. Its 513 nodes use 24 bytes each on the measured platforms. The node vector changes from ten allocation/reallocation calls to one; two output-vector allocations remain in the first-decode measurement. Capacity bytes exclude allocator overhead and do not predict RSS. Cold latency was not measured. Decoder traversal, padding/EOS validation and output limits are unchanged.

The comparison executable includes the other benchmark frameworks; its size is not Maximus's standalone crate size. The larger routing fixture contains 64 literal paths with GET/POST/PUT each, and validates 320 GET/HEAD/POST/PUT/PATCH dispatches on both revisions. It is a size/correctness check, not a large-router throughput claim.

## Retained-pair socket checks

Five alternating pairs per workload on the same Linux host; values are medians, baseline → retained pair. All final trials require equal positive started/completed/HTTP-request counts and validate every status, body and protocol.

| Workload | Responses/s | Process CPU µs/response | p99 ms | p99.9 ms |
| --- | ---: | ---: | ---: | ---: |
| HTTP/1 GET, 64 VUs | 22,695.8 → 22,705.3 | 13.54 → 13.32 | 12.455 → 12.261 | 18.229 → 17.723 |
| HTTP/1 1 KiB echo, 64 VUs | 20,885.2 → 21,057.3 | 14.35 → 14.18 | 13.012 → 13.045 | 17.789 → 18.181 |
| TLS HTTP/2 1 KiB echo, 4,000/s | 4,000.1 → 4,000.1 | 32.02 → 32.02 | 1.121 → 1.049 | 4.537 → 2.870 |

The retained pair completes 4,824,039 responses across all 15 trials with zero failed response checks or dropped arrivals. Its paired baseline misses 35 arrivals in one HTTP/2 trial; that failed target stays in the table and CSV. A secondary calculation using only the four successful HTTP/2 pairs also stays inside the 3% CPU/throughput and 5% tail screens. The large observed HTTP/2 tail improvement is not attributed entirely to these small code changes.

HTTP/1 runs last 20 seconds. TLS/HTTP/2 runs last 25 seconds, ramping from 100/s to 4,000/s over five seconds and then holding for 20 seconds. Fixed-rate responses/s uses the steady response count; closed-loop rates use the entire run. Latency includes responses completing after the first five seconds. CPU includes warmup and uses process user+system ticks divided by all completed responses; separately scheduled kernel work is excluded. RSS is a post-run sample, not peak memory.

These are workload regression screens, not saturated server-capacity measurements or rankings against other frameworks. The k6 app has one method per path and ordinary fixed-length uploads: the dispatch and Huffman probes establish the direct savings; k6 checks the wider server. The shared host and small samples limit claims about tails or universal performance parity. Per-pair results, paired bootstrap intervals and unfavorable observations remain in the evidence.

Rust 1.96.1, locked dependencies, thin LTO, one codegen unit, k6 1.6.0, verified TLS 1.3/HTTP/2, AMD Ryzen 7 5825U and Linux 6.6.68. Server CPU 2 and client CPUs 4/6 are different physical cores, but not exclusively reserved; both containers have two-CPU limits. No task compilation or competing task benchmark ran on this host during timed trials. The local size/dispatch fixture uses macOS ARM64 and Rust 1.96.0.

## Rejected experiments and controls

- Incremental CRLF scanning improved long fragmented lines, but a short one-byte-fragmented case showed an adverse ARM64 signal. Follow-up local timings were disturbed by unrelated compilation, while x86-64 favored the scan. It remains deferred; the line reader is unchanged.
- Removing H1 TE/Expect collections saved one allocation/32 requested bytes per relevant field. The three-change bundle nevertheless used 3.4% more median CPU per HTTP/1 echo response in its first five pairs. Individual-build trials varied and did not establish a unique cause. The H1 rewrite is excluded; the smaller retained pair was measured again from scratch.
- The first mixed probe suggested a borderline warm Huffman increase. A separate uninstrumented decoder-only baseline/A-A/candidate rotation ran 30 repetitions per variant: short-header medians were 134.89/135.46/135.84 ns, and long-string medians 5,557.74/5,510.51/5,509.32 ns. The candidate's paired short-header change was +0.11%, with a bootstrap interval of −2.05% to +2.34%; long strings were −1.62%, interval −2.66% to −0.71%. This supports retaining the storage saving within the 3% screening margin; it does not establish a warm speedup for every input.
- Earlier 5,000/s HTTP/2 trials missed 109 arrivals on baseline and two on the three-change bundle, across three failed load thresholds. All completed responses were correct. The retained pair's lower-rate campaign does not erase those failed targets or establish 5,000/s capacity.
- One retained-pair attempt aborted on a Kubernetes API TLS handshake timeout before k6 produced a summary. Its valid, unpaired baseline row and the failure are preserved separately; the whole phase restarted under a new label.

The CSV label `final` denotes the rejected three-change bundle; `selected` denotes this PR. The A/A phase uses a byte-identical baseline copy. The [trial CSV](lean-routing-trials.csv) retains every valid trial, including failed offered-load thresholds. The [evidence manifest](lean-routing-evidence.json) contains source/binary hashes, all build sizes, focused samples, controls, paired summaries, the aborted attempt and reproduction sources. Raw logs and local probes remain ignored under `artifacts/lean-routing/`. The temporary Kubernetes namespace was removed after testing.

## Correctness and reproduction

Final-source formatting, warnings-denied Clippy, feature builds and documentation pass, with 158 test executions. Linux records 136 passes and one unchanged privileged eBPF skip. The external-consumer check and 237 vendored H3 test executions pass. Independent HTTP/1, TLS HTTP/1, TLS HTTP/2, verified QUIC HTTP/3, WebSocket and feature clients pass, including 2,062 raw-wire mutations/disconnects and post-stress health.

The final-source request and HPACK fuzz campaigns complete 9,496,232 and 1,580,605 executions without a finding. The earlier wider experiment also ran HTTP/1 and HTTP/2 fuzz campaigns. Both final-source h2spec policy gates pass: each raw legacy suite is 139/147, with five documented policy differences and three closure races; all three held-response reruns pass. This is not a claim of 147/147 unmodified legacy conformance.

Run `scripts/check.sh`, `scripts/check-downstream.sh`, `scripts/test-linux.sh`, `scripts/test-interop-linux.sh` and the fuzz/conformance commands in [qualification](../../docs/qualification.md). Run the allocation guard with `cargo run --release --locked --manifest-path benches/baselines/Cargo.toml --bin allocations`.

For focused reproduction, extract the manifest's scripts into ignored `artifacts/lean-routing/`, archive the baseline there as `baseline/`, and run `make-probe.py` with the chosen source tree before building its generated standalone Cargo project. It compiles copies of the actual engine source with temporary private-function wrappers; no wrappers enter the product. The optional experimental H1 patch reconstructs the excluded header candidate. The manifest also includes the isolated Huffman and larger routing fixtures, exact k6 workload/runner, pod configuration and affinity launcher. Rebuild and hash each server from its identified source; alternate order and avoid compiling during measurement.
