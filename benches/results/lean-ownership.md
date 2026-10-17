# Reusing route matches, borrowed headers and response ownership

This pass evaluates five small changes against merged baseline `23ec7c87a55c6c53043b5fc59befda627b08c560`. Four candidates remain in this draft after focused checks: reuse a selected static route group, borrow Content-Type during JSON/multipart extraction, move H2 DATA from its frame, and reuse the existing H1 output buffer for the no-trailer terminator. The production diff adds nine net lines across five existing modules. There are no dependency, configuration, public API or product unsafe-code additions; Compio remains.

The code is not qualified for merge under the stated performance gates: the corrected H2 streaming p99.9 and JSON p99/p99.9 intervals remain inconclusive. No further pass-seeking repetitions are run. The implementation and its measured local gains remain reviewable without a no-regression claim.

## Direct measurements

| Fixture | Baseline → candidate allocation/reallocation calls | Cumulative requested bytes |
| --- | ---: | ---: |
| JSON boolean extraction | 5 → 4 | 816 → 800 |
| Multipart extractor construction | 9 → 8 | 1,122 → 1,084 |
| H1 streamed response, no trailers | 8 → 6 | 17,139 → 17,128 |
| H2 owned 1 KiB response | 10 → 9 | 8,992 → 8,968 |

These fixtures include request/body/output construction. They are not allocations per complete network request. H1's saving removes a short-lived terminator vector and its growth; peak tracked bytes remain unchanged. H2's allocation saving applies to an owned payload that fits the available frame/credit: static, already shared and split 128 KiB payloads do not save an allocation. No body copy is removed by replacing the Bytes clone; the benefit is avoiding unnecessary sharing bookkeeping.

A separate allocation build uses a fixed Date in the H1 fixture. The timing binaries use the normal allocator without counters. Net outstanding requested-byte counters are instrumentation, not physical memory measurements or proof of leak freedom. All strengthened preflights validate routing status, exact concatenated H2 DATA, stream ID and END_STREAM.

Twenty-five rotating repetitions per variant, including a byte-identical A/A control, were measured on Linux x86-64 and macOS ARM64. Paired geometric-mean changes in the four-change bundle:

| Focused operation | Linux time | ARM64 time |
| --- | ---: | ---: |
| Captured POST route | −17.6% | −20.3% |
| Captured PUT route | −27.0% | −30.6% |
| Captured method rejection | −26.5% | −26.0% |
| JSON extraction | −22.0% | −12.3% |
| Multipart construction | −0.8% | −9.5% |
| H1 chunked response | +0.1% | −10.0% |
| H2 owned 1 KiB response | −11.8% | −9.7% |

Linux multipart/H1 and several control cases are neutral or inconclusive, not demonstrated speedups. H2 probes cover response polling/encoding with ample credit, excluding request decoding, handlers, TLS, sockets and output draining. Multipart parsing is exercised by the socket workload. The evidence manifest retains all focused observations, final-screen summaries and paired bootstrap intervals; these microbenchmarks do not establish whole-server throughput.

The standalone Linux qualification server's text section shrinks by 520 bytes, but its total file grows by 208 bytes (3,451,992 → 3,452,200); no whole-executable size saving is claimed. The ARM64 server grows by 432 bytes. The separate 192-route fixture stays at 575,280 bytes and passes 320 dispatch checks on both revisions.

## Socket qualification

The final comparison covers eight workloads and 47,244,822 checked responses. Each response checks status, complete body and protocol. Positive started/request/completed counts match. Validity and performance decisions are separate; see the decision column and full intervals in the evidence. The table uses the independently analyzed confirmation for JSON/multipart and the corrected admission fixture for H2; it does not pool them with earlier attempts. Changes below are paired geometric means; the full intervals are in the evidence.

| Workload | Pairs | Requests checked | Throughput | CPU/request | p99 | p99.9 | Decision |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Plaintext | 5 | 4,473,610 | +1.13% | -0.01% | -1.71% | -1.60% | pass |
| Routing | 5 | 4,400,890 | -0.63% | -1.48% | -0.12% | -0.09% | pass |
| JSON | 10 | 12,608,107 | -0.30% | +0.36% | +0.65% | +0.08% | pass |
| Multipart | 10 | 12,493,828 | +0.10% | -0.34% | -0.71% | -1.38% | pass |
| Chunked stream | 5 | 4,038,503 | -0.79% | +0.16% | +0.23% | +1.10% | pass |
| SSE control | 5 | 4,020,287 | +1.10% | -1.04% | -0.29% | -2.54% | pass |
| H2 stream (corrected fixture) | 10 | 2,604,823 | -0.00% | +0.18% | -2.50% | +6.51% | inconclusive |
| H2 JSON (corrected fixture) | 10 | 2,604,774 | +0.00% | -0.27% | -2.50% | -15.94% | inconclusive |

The initial screen uses 64-VU H1 runs for 20 seconds and verified TLS 1.3/H2 runs that ramp from 100/s to 4,000/s over five seconds, then hold for 20 seconds. In the table, H1 plaintext/routing/stream/SSE retain those 20-second runs; JSON/multipart use the independent 30-second confirmation. Both corrected H2 workloads use 35-second runs: a five-second ramp and 30-second hold. Throughput is the whole-run rate including warmup; 4,000/s is the steady offered rate. Latency covers responses completing after five seconds. CPU uses process user+system ticks divided by all completed responses, including warmup; separately scheduled kernel work is excluded. RSS is a post-run sample, not peak memory.

The server runs one io_uring worker on CPU 2; k6 1.6.0 uses CPUs 4/6 in a separate container. Both containers have two-CPU limits. These are different physical cores on a shared AMD Ryzen 7 5825U/Linux 6.6.68 host, not exclusive reservations. Rust 1.96.1, identical dependency resolution, thin LTO and one codegen unit are used for the Linux pair; local probes use Rust 1.96.0 on macOS ARM64. No task compilation or competing task benchmark runs on the Linux host during measurement. Results are bounded regression screens under this client load, not maximum capacity or a ranking against other frameworks.

The acceptance boundaries are directional: the 95% paired bootstrap interval must remain above −3% for throughput, below +3% for CPU per request, and below +5% for each tail. A CPU improvement cannot offset an unresolved tail result. JSON and multipart initially leave p99.9 inconclusive. Both pass all gates in their independent ten-pair confirmation (12,608,107 and 12,493,828 checked requests, respectively). JSON's confirmed p99.9 change is +0.08% (95% interval −2.19% to +2.48%); multipart's is −1.38% (−3.45% to +0.52%). Their confirmed throughput and CPU results are neutral, while the allocation reductions are directly observed. Before the first campaign completed, a fixed confirmation plan was recorded: exactly ten new alternating pairs for any workload whose interval overlaps a boundary, 30-second H1/35-second H2 runs with the same five-second latency warmup. An interval entirely beyond a boundary triggers diagnosis/rejection instead. Confirmation is analyzed independently; pooled results are sensitivity checks only, and no further reruns are permitted to obtain a pass. This adaptive engineering screen does not provide an overall 95% no-regression guarantee.

The original H2 arrival-rate runs failed validity: one baseline stream trial dropped 54 iterations and one candidate JSON trial dropped 37. All started requests passed their response checks. Both logs explicitly report exhaustion of the 64-VU pool. A later cumulative client cgroup snapshot records no CPU-quota throttling; it does not identify the cause of those occupancy spikes.

The first 256-VU investigation then stopped on its first baseline trial with 139 request timeouts. This exposed an admission mismatch in the fixture: its 4 GiB budget, default 1 MiB request limit and 32 streams reserve 36,569,088 bytes per TLS/H2 connection, admitting only 117. The 139 failures equal 256 minus 117. The actual Rust capacity method, with the TCP TLS I/O adjustment used by `Transport`, verifies that result. No candidate trial ran in this failed configuration.

The corrected H2 fixture sets only its request-body bound to 1 KiB; the measured requests are empty or four bytes, and the streamed response remains 1 KiB. The same budget now admits 1,409 connections. Product defaults are unchanged. Both revisions were rebuilt at the same path with the same lockfile; original binaries and failed results remain preserved. All four preflights pass, totaling 1,024 checked responses (256 shared iterations with 256 VUs each, not proof of 256 distinct simultaneous connections). The separate fixed campaign uses ten alternating 35-second pairs per workload, 256 VUs, the same arrival schedule and gates, and client CPU/throttling snapshots. Its results qualify only this corrected configuration; they cannot erase or qualify the earlier targets. Review found one stopping-rule deviation: the running controller stops on response/check failures but permits threshold exit 99 and dropped arrivals to leave subsequent rows. It was not changed mid-campaign. Final acceptance still rejects any drop or nonzero exit, regardless of later rows.

The complete corrected H2 streaming screen checks 2,604,823 responses with zero validity failures and zero client CPU-quota throttling. Throughput, CPU/request and p99 pass. Its p99.9 change is +6.51%, with a 95% paired bootstrap interval from −23.88% to +43.03%; the upper bound exceeds the +5% criterion. This neither establishes a regression nor rules one out. Variability occurs in both revisions, and these data do not isolate its cause or attribute it to a particular change. A JSON pass cannot override this endpoint.

The corrected H2 JSON screen checks 2,604,774 responses with zero validity failures. Throughput and CPU/request pass. Its p99 change is −2.50% (95% interval −11.23% to +7.46%) and p99.9 is −15.94% (−41.44% to +19.70%); both remain inconclusive against the +5% upper-bound gate. Both corrected workloads record zero client CPU-quota throttling. Six H1 workloads pass, but the bundle remains unqualified for merge because both H2 tail screens are unresolved.

## Rejected candidates and measurement corrections

- Direct SSE retry formatting did not remove an allocation: 12 calls stayed 12, while requested bytes rose from 986 to 995. It was reverted.
- The first routing expression had an adverse Linux miss-path signal. The retained form preserves the original no-match check order while skipping an already established match. Its later-match gains reproduce on both architectures; final miss-path observations remain in the evidence.
- The initial H2 mixed probe showed substantial slowdown. It timed per-byte payload validation, used short loops and retained the counting-allocator wrapper even when disabled. A longer default-allocator follow-up moves validation outside timing and independently measures the H2-only candidate. That follow-up favors the ownership change on both architectures. The initial observations remain available; the corrected fixture does not prove identical performance for arbitrary callers.
- Unchanged SSE code showed a +3.24% Linux signal in the mixed probe. A separate SSE-only build, with JSON/multipart features enabled, produced byte-identical before/after executables and validated 20,000 responses on each revision. The full-bundle socket SSE control passes the first regression screen.
- The original bodyless routing fixture passed an empty string to k6 PUT. A raw wire capture showed `Transfer-Encoding: chunked`; the handler left that body unread, invoking the existing documented connection-close policy. The baseline recorded 2,120 EOF/request-check failures. The corrected fixture passes `null`, verified to send `Content-Length: 0`. All eight workloads are smoke-tested on both binaries before continuing the paired campaign. The failed trial remains visible; plaintext pairs from the same campaign are unaffected.
- Initial ARM64 timing overlapped local summary computation after repetition nine. Those samples are exploratory; the 25-repetition follow-up ran without this task's concurrent local compilation or analysis.
- Early socket attempts are excluded after a client-launcher ABI mismatch, Kubernetes exec resets, and an orphan comparison server discovered sharing the port. One TLS smoke recorded three failed requests. These are retained as failed/excluded attempts, not passes. The new controller owns and waits on each server PID, requires an unused listener port, rejects existing result keys and records k6's exit status inside the pod. A guard then stopped one unpaired baseline trial while the previous io_uring listener was still disappearing; the final controller waits up to five seconds for the port to clear before proceeding.

## Correctness and reproduction

Exact-source formatting, warnings-denied Clippy, feature builds and documentation pass, with 160 test executions. Linux has 138 passes and one unchanged privileged eBPF skip. The downstream consumer check and 237 vendored H3 test executions pass. Independent HTTP/1, TLS HTTP/1, TLS HTTP/2, verified QUIC HTTP/3, WebSocket and feature clients pass, including concurrent uploads and 2,062 raw-wire mutations/disconnects with post-stress health checks.

Request, HTTP/1 and HTTP/2 fuzz campaigns complete 9,440,649, 1,298,349 and 1,591,294 executions without a finding. Both h2spec policy gates pass: each raw legacy suite records 139/147, with five documented RFC policy differences and three closure races; the three held-response reruns pass. This is not 147/147 unmodified legacy conformance.

The new tests preserve missing/duplicate/invalid Content-Type error precedence and verify H1 no-trailer completion across pipelined responses with one-byte I/O. Existing tests cover route precedence/captures/HEAD and H2 streaming, trailers, limits, resets and flow control. Independent code and harness reviews found no remaining blocking findings after the measurement guards were strengthened.

Run `scripts/check.sh`, `scripts/check-downstream.sh`, `scripts/test-linux.sh`, `scripts/test-interop-linux.sh`, and the conformance/fuzz commands in [qualification](../../docs/qualification.md). The [evidence manifest](lean-ownership-evidence.json) records source hashes, experimental patches, build identities, focused results and standalone reproduction sources. The [trial CSV](lean-ownership-trials.csv) records socket results with explicit eligibility. Raw logs, generated projects and binaries remain ignored under `artifacts/lean-ownership/`; no benchmark wrapper enters product code. Reconstruct each source from the baseline and recorded patch, build both at the same path, and keep compilation separate from timed runs.

The four-change bundle remains a draft because the H2 tail gates are unresolved. The temporary benchmark namespace was deleted and verified absent after downloading and checksum-verifying all 904 raw result files. No additional repetitions were run to obtain a pass.
