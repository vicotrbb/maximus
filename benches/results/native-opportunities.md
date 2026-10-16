# Native HTTP opportunity experiments

Archived source: `9cb096a27962a2966ca75b30e643f9e466c5cc4b`, Maximus version `0.1.0`. Measured 2026-10-16 on the shared x86-64 homelab node. No active product, Cargo registry, previous result, or common manifest was changed.

The allocation-free Compio error construction removes repeatable allocation traffic. Its throughput benefit varies by worker placement. Explicit worker pinning removes migrations without establishing a consistent throughput or latency benefit. Connection churn retains resident pages while live Rust heap returns to a small, stable value. Synthetic policy and adapter profiles provide partial attribution and do not justify dropping enforcement or predicting native HTTP speed.

## Full HTTP and placement trials

Thirty-six independent wrk trials completed 65,833,036 checked responses with zero failed checks, network errors or recorded server cgroup throttling. Each trial uses 128 connections, four client threads, a new two-second warmup process and a new eight-second measured process. Entries are three-trial medians; p99 values are medians of per-trial corrected percentiles. Baseline and patch alternate, with reversed order in repeat two.

| Workers | Placement | Baseline RPS | Patch RPS | Change | Baseline / patch p99 ms | Baseline / patch sched migrations |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | mask | 118,147 | 118,944 | +0.67% | 1.579 / 1.805 | 0 / 0 |
| 1 | pin | 118,795 | 117,012 | -1.50% | 1.635 / 2.015 | 0 / 0 |
| 2 | mask | 213,697 | 214,757 | +0.50% | 1.045 / 1.014 | 12 / 7 |
| 2 | pin | 218,285 | 215,549 | -1.25% | 0.912 / 0.976 | 0 / 0 |
| 4 | mask | 341,374 | 354,699 | +3.90% | 1.193 / 1.514 | 104 / 126 |
| 4 | pin | 349,742 | 347,650 | -0.60% | 2.310 / 1.990 | 0 / 0 |

The process mask contains the first N logical CPU IDs from `0,2,4,6`, mapped to distinct physical server cores. Pinning passes those same CPUs to the existing `WorkerConfig.cpus` API, one CPU per worker. Client logical CPU IDs are `8,10,12,14`, mapped to disjoint physical cores. Pinning produced zero observed aggregate scheduler migrations in every trial. In the baseline, median two/four-worker migrations dropped from 12/104 to zero, while RPS rose about 2.1%/2.5%; patched pinning gave a different throughput pattern and four-worker p99 rose. This supports explicit placement as an available experiment, not a universally faster default.

Auxiliary one-worker process-mask instruction counts were 34,835 versus 33,885 per completed request. These approximate task-attached counts include kernel/interrupt attribution and are not a pure application instruction count. Raw RPS ranges, cache misses, CPU use, scheduler snapshots and every trial are in `raw/summary.json` and per-trial files. Shared workloads, frequency changes and cache/IRQ sharing remain confounders. PMU counters use an approximate eight-second interval rather than acknowledged client-boundary controls.

## Allocation-free WouldBlock

Only the isolated Compio 0.10.1 copy changes `io::Error::new(io::ErrorKind::WouldBlock, msg)` to `io::ErrorKind::WouldBlock.into()`. ErrorKind and I/O control flow remain; display text loses the detailed message. The patch is experimental and has not been promoted.

| Scope | Baseline allocations | Patch allocations | Baseline requested bytes | Patch requested bytes |
| --- | ---: | ---: | ---: | ---: |
| Empty buffered read returning WouldBlock, per operation | 3.0000 | 0.0000 | 72.0000 | 0.0000 |
| Complete HTTP/1 keepalive, per request, three-trial median | 30.9943 | 18.9988 | 12,707.1118 | 12,416.6192 |

The complete-path runs each validate 10,000 measured responses after 100 warmup requests and use process mask `0,8`. The counters cover both client and server allocations in that process. Baseline allocations ranged 30.9918 to 30.9961; patched allocations ranged 18.9973 to 18.9997. Static route/body dispatch remained zero; static text response conversion remained two allocations / 656 bytes. This scheduling and environment differ from the earlier approximately 20-allocation measurement, so the absolute counts must not be pooled. Allocation traffic does not describe retained heap.

All five upstream Compio compatibility tests passed with the patch. The first test invocation could not resolve its packaged lockfile, which pinned bytemuck 1.25.0 while the retained vendor cache has 1.25.2. Resolving a new lockfile only in the isolated patch copy made the tests runnable; both logs and the resolved test lockfile are retained.

## Live heap and retained RSS

The decisive run gates the next client cycle on the server memory sample. Each cycle opens 20,000 connections, validates all responses, waits five seconds, revalidates all connections, waits five seconds, closes them, and waits for six server FDs before acknowledging the next cycle. The same server survives all three cycles, completing 120,000 checked responses.

| Quiescent phase | RSS MiB | Live requested Rust bytes | FDs |
| --- | ---: | ---: | ---: |
| baseline | 5.12 | 613,468 | 6 |
| After cycle 1 | 526.32 | 908,829 | 6 |
| After cycle 2 | 590.82 | 908,829 | 6 |
| After cycle 3 | 601.45 | 908,829 | 6 |
| finished | 601.45 | 908,829 | 6 |

At the final sample, resident memory is roughly 600 MiB above startup but live requested Rust heap is only 0.87 MiB. Live bytes remain exactly 908,829 after every close. This strongly supports retained freed allocator pages rather than a growing population of live Rust allocations. Full `smaps_rollup` samples show anonymous/private resident memory; they do not classify glibc arena fragmentation or identify every direct libc/mmap allocation. A live Rust heap leak of hundreds of megabytes is contradicted by this instrumented run. No allocator tuning or product memory fix is proposed from RSS alone.

The allocator forwards allocation contracts unchanged to System, counts successful alloc/realloc sizes, subtracts deallocation sizes, and stores no pointer map. Live bytes exclude allocator metadata, fragmentation, direct libc allocations, mmap and kernel socket buffers. Requested bytes can exceed resident bytes while buffers have untouched pages. The observer is a separate binary and is absent from throughput binaries.

The first, ungated churn run also ended at six FDs, 908,829 live bytes and 600.61 MiB RSS. Its intermediate close samples raced the following cycle and are retained as unqualified phase measurements in `raw/churn.jsonl`.

## Policy, deadlines and adapters

The staged synthetic profile constructs identical valid plaintext requests and exercises direct handler conversion, standard request timeout, and exact shared `dispatch_request`. It includes request construction and response conversion, with 300,000 iterations per stage and three stages of each. The baseline medians are 252.5, 359.1 and 575.0 ns/request respectively. The timeout increment is about 106.5 ns; the full-dispatch increment over direct handling is about 322.5 ns. These are partial attribution measurements, excluding connection admission, wire parsing, idle/header lifecycles, pending I/O and most cancellation paths.

Both profile builds reject duplicate Host with 400 and expire a deliberately pending handler with 408. Standard enforcement remains enabled in every full HTTP trial. No complete equivalent-policy Tokio server was introduced, and bare Hyper remains an unequal-policy comparison. The earlier first-poll timeout change stays rejected: before allocation counts 20.0825/20.1221/20.1254 overlapped after counts 20.0748/20.2409/20.0682.

| Synthetic operation | Baseline median ns/op | Patched median ns/op |
| --- | ---: | ---: |
| Direct ready read with payload copy | 9.26 | 10.38 |
| Existing compatibility adapter ready read | 15.10 | 15.87 |
| Direct ready write + flush | 1.06 | 1.10 |
| Existing compatibility adapter ready write + flush | 6.94 | 7.75 |
| Compio WouldBlock + refill + 13-byte buffered read, payload checked | 56.40 | 31.91 |

The wrapper adds about six nanoseconds in these ready-only cases. The buffered case executes real WouldBlock construction, refill, copying and exact payload assertions. These lower-level costs are not socket/HTTP speed predictions. The ready-read driver adds a Hyper-to-Tokio observation wrapper that may inline; the write driver calls Hyper traits directly. Cross-build direct-handler measurements varied substantially, 252.5 versus 338.0 ns, despite identical handler logic, so these short diagnostics cannot establish precise causal end-to-end deltas.

## Reproduction and evidence

All ignored artifacts live in `artifacts/native-opportunities/`. Source identity, binary hashes, the complete analysis lockfile, isolated test lockfile, patches, scripts, build failures, interrupted attempt, raw counters and successful outputs are retained there. The archived source retains its original package version. Diagnostic manifests add direct references to existing dependencies; no active/common manifest or global registry is edited.

```sh
git archive 9cb096a | tar -x -C artifacts/native-opportunities/baseline
kubectl --context homelab create -f artifacts/native-opportunities/pod.yaml
# Copy the archived tree to /work/base and retained vendor cache to /work/vendor.
# .cargo/config.toml replaces crates.io with /work/vendor.
kubectl --context homelab -n maximus-native-9cb096a-opportunities exec benchmark -c server -- sh -c 'cd /work/base/benches/baselines && CARGO_TARGET_DIR=/work/target cargo build --release --offline'
# Copy original binaries to /work/baseline and /work/allocations-baseline.
# Apply compio-wouldblock.patch to a COPY at /work/compio-patch.
# Add isolated manifest patch: compio-io = { path = "/work/compio-patch" }.
# Build and copy /work/wouldblock and /work/allocations-wouldblock.
python3 artifacts/native-opportunities/run_campaign.py
kubectl --context homelab -n maximus-native-9cb096a-opportunities exec benchmark -c server -- python3 /work/diagnostics.py
kubectl --context homelab -n maximus-native-9cb096a-opportunities exec benchmark -c server -- python3 /work/churn-gated.py
python3 artifacts/native-opportunities/summarize.py
python3 artifacts/native-opportunities/verify_evidence.py
```

The timed runner was updated after a Kubernetes exec reset. Twenty-four trials finished before it; the interrupted third-repeat attempt had 943,473 checked responses but no final server snapshot and is excluded. Its logs remain under `raw/interrupted` and `raw/control-reset-state.log`. The updated runner submits each detached client once and observes atomic completion status, avoiding resubmission of an ambiguous launch. Twelve third-repeat trials then completed.

At the Task 1 handoff, the owned namespace was retained for the parent native qualification campaign. Its historical paths were `/work/base`, `/work/vendor`, `/work/target`, `/work/baseline`, `/work/wouldblock`, `/work/instrumented`, `/work/load.lua`, and `/work/tools`. Client wrk runs through `/work/tools/ld-linux-x86-64.so.2 --library-path /work/tools`; the tools directory includes taskset and wrk dependencies. Compiler is Rust 1.96.1 and all Rust release builds use thin LTO with one codegen unit. The image digests and binary SHA-256 hashes are in the environment logs.


## Final campaign cleanup

The parent comparative campaign and all reserved diagnostics completed before cleanup. Required raw data, source patches, profile data and binaries were retained locally and verified against remote hashes. Only namespace `maximus-native-9cb096a-opportunities` in context `homelab` was deleted, with a verified NotFound response at 2026-10-16T15:55:21.293425+00:00. The exact commands and output are in `artifacts/native-http/qualification/cleanup-evidence.json`; the verified 127,119,360-byte remote evidence archive is `artifacts/native-http/qualification/remote-native-measurements.tar`, SHA-256 `151c8fe92ff7b4575c566d782e45a8e378022a1221461dce6030e29c6138c41e`. Historical kubectl reproduction commands require recreating and staging an owned benchmark Pod. Existing Docker containers and all other namespaces were untouched.
