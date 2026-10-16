# Reproducible performance checks

The comparison servers are an isolated Cargo workspace. Product builds do not resolve Axum, Actix or their benchmark dependencies. All Rust servers use thin LTO and one codegen unit. Commit both lockfiles when changing dependencies.

Latest measurements: [lean core and dynamic routing](results/lean-core.md). The preceding [native HTTP/2 hot-path experiments](results/native-hot-paths.md) include rejected hypotheses, matched framework comparisons and dependency scope.

## Build and run

```sh
docker pull rust:1.96-bookworm
docker pull grafana/k6:1.6.0
docker pull nginx:1.29.6-alpine
docker build -f benches/Dockerfile.tools -t maximus-tools .
docker run --rm --security-opt seccomp=unconfined \
  -v "$PWD:/work" -v maximus-cargo:/usr/local/cargo \
  -v maximus-bench-target:/work/benches/baselines/target \
  -w /work/benches/baselines rust:1.96-bookworm cargo build --release --locked
python3 benches/run.py --label plaintext --paths /plaintext --repeats 3
python3 benches/run.py --label arrival --rates 5000,15000 --duration 30s --repeats 3
python3 benches/run.py --label scaling --workers 2 --paths /plaintext --repeats 3
python3 benches/run.py --label tls-h2 --engines maximus,hyper,axum --protocol h2 --paths /plaintext,/echo --rates 5000 --repeats 3
python3 benches/run.py --label tls-h1 --engines maximus,hyper,axum --protocol h1 --paths /plaintext --rates 5000 --repeats 3
python3 benches/run.py --label native --engines maximus,nginx --paths /plaintext,/json --repeats 3
```

The runner assumes three Linux CPUs: server CPU 0 (0–1 with two workers), client CPU 2. Use a dedicated host for performance claims. Existing background services must be recorded when the host cannot be dedicated. It warms the server for 3 seconds in a separate k6 process, then starts new client connections for measurement. TLS connection establishment therefore contributes to cold-start pressure; request-duration metrics do not include the entire handshake. It rotates server order across repeats, checks every status/body/protocol, records p50/p99/p99.9, achieved requests/sec, failure counts, dropped iterations, server cgroup CPU microseconds/request, server/client CPU accounting ratios, and process-tree RSS. Use `--sample-seconds 5` for periodic server memory samples during a sustained run; short comparisons avoid this sampling overhead. `--client-cpus` can allocate more client cores when available. The Docker runner's CPU percentages divide the server cgroup CPU delta, or the client's entire cgroup CPU usage, by k6's reported test duration, using one CPU as 100%. Those accounting windows include work outside that duration. These duration-normalized ratios can therefore exceed the assigned CPU budget without demonstrating average cores used or quota saturation. RSS is the sum of process-tree observations, which double-counts shared pages for multi-process servers such as Nginx; it is not unique physical memory. Tables distinguish post-run RSS from the largest observation, which is not a continuous peak. Raw JSON, logs and environment/image digests go to `artifacts/bench/`. Fixed arrival rates are open-loop; zero means a closed-loop constant-VU scenario. [k6 arrival-rate behavior](https://grafana.com/docs/k6/latest/using-k6/scenarios/executors/constant-arrival-rate/) and [threshold semantics](https://grafana.com/docs/k6/latest/using-k6/thresholds/) define the distinction. Dropped iterations are a load-generator shortfall and must not be counted as completed server requests.

TLS runs use the same generated RSA certificate, Rustls/Ring configuration, TLS 1.3 range, ALPN selection and response payload. The local CA is trusted by k6; certificate verification stays enabled. H2 flow-control and concurrent-stream settings match. k6 protocol assertions reject silent HTTP/1 fallback. The TLS Hyper branch shares an adapter that clones the Axum router and wraps its response body even when calling the Hyper handler; it is not a minimal raw-Hyper TLS lower bound. Plaintext Hyper uses its direct service path. k6 does not qualify HTTP/3 here; aioquic interoperability is a separate test.

Plaintext baselines use each framework's normal server path. Maximus performs its documented admission/extraction policy; Hyper is a lower-level library baseline. JSON here is a fixed serialized payload, not a serialization benchmark. Nginx is a native static-response reference; it does not represent application routing, uploads or middleware. `/echo` posts 1 KiB and `/users/42` exercises typed routing. Maximus's benchmark admission estimate is raised explicitly so the memory reservation does not cap 64 benchmark connections; this is not its production default.

## Allocations, kernels and profiling

```sh
docker run --rm --security-opt seccomp=unconfined -v maximus-bench-target:/target:ro rust:1.96-bookworm /target/release/allocations
docker run --rm --security-opt seccomp=unconfined -v "$PWD:/work" -v maximus-cargo:/usr/local/cargo -v maximus-target:/work/target -w /work rust:1.96-bookworm cargo run --release -p maximus-io --example kernel
benches/profile.sh
```

Allocation instrumentation reports the in-process route separately from the complete keepalive HTTP path; it must not be used to claim zero allocations on the wire. Kernel measurements compare ordinary, managed/pool, multishot reads and zero-copy sends with verified payloads and three trials. They are socket roundtrips, not HTTP results. The main HTTP adapter stays on its maintained bounded path unless a representative HTTP benchmark demonstrates a gain.

`profile.sh` records software CPU-clock samples with Linux perf and emits a flamegraph. Hardware cycles/instructions/cache misses require a PMU exposed by the host; record unsupported counters explicitly instead of deriving fictional cycle counts from wall time. The [Linux hardware runner](HARDWARE.md) adds task-attached PMU measurements with acknowledged enable/disable controls, physical-core topology, source/binary hashes and response-checked wrk workloads.

`python3 benches/connections.py --docker` runs three cycles against the same server process. Each cycle opens 20,000 connections, validates a response on each, holds them idle, revalidates every connection, and closes them. Use `IDLE_CONNECTIONS=25000` on the Maximus comparison server, `--ulimit nofile=65536:65536` on both containers, and the same Linux host network. This workload sets body allowance to zero and uses 1 KiB I/O buffers and a 1 GiB admission budget. The runner records process RSS and descriptor counts before, during and after each cycle. Set `CYCLES=10` for a longer churn test. Retained RSS alone does not prove a leak; look for growth across repeated cycles and descriptors returning to baseline.

Results and caveats belong in `results/`, with raw files retained locally under `artifacts/`. A VM/client-limited run is not evidence of best-in-class throughput or near-linear scaling.

Each runner writes per-trial CSVs, summaries and metadata under its output directory. Preserve failed trials when summarizing a new campaign; raw artifacts remain ignored.

## Additional qualification commands

```sh
# Provision enough VUs for a common offered load, preserving any earlier failed run.
python3 benches/run.py --label qualified-arrival-provisioned --paths /plaintext --rates 5000 --vus 512 --duration 10s --repeats 3
# Give the VM client two CPUs while keeping the server on CPU 0.
python3 benches/run.py --label qualified-capacity --engines maximus,epoll,hyper,axum,actix --paths /plaintext --client-cpus 1,2 --duration 30s --repeats 3
```

The checked-in [comparison report](results/native-http-comparison.md) and [trial data](results/native-http-trials.csv) retain failed targets and explain the observed limits. Hardware setup and cleanup are documented in [HARDWARE.md](HARDWARE.md). Benchmark infrastructure and generated caches never enter the product dependency graph.

To redraw the native throughput and p99 scaling chart from the checked-in trial CSV, install Matplotlib in an optional local Python environment and run `python3 benches/plot.py`. Matplotlib is not a product or benchmark-runner dependency.

## Kubernetes archive comparisons and resume

Use the existing [temporary Pod setup](HARDWARE.md) and `benches/kubernetes.py` for comparisons between separately built source archives. Run only one campaign at a time in a Pod: the runner owns its server, client and profiler PID files. The default command still uses `/work`, `/work/benches/baselines/target/release/maximus-comparison`, `/work/benches/load.js`, `/work/benches/load.lua`, and 128 connections/VUs.

For the native HTTP comparison, stage these source trees on the shared `/work` volume and build each benchmark workspace with the same compiler, release profile and CPU allocation:

The product source was frozen at `8a8fa9b23c652ae4b6e40fec6322cb9e094e22c9`. Use archive `f85abcc841ee47c4d7739e21d669fd062112414b` for locked reproduction: it corrects eleven stale path-package versions in the comparison lockfile without changing product code.

| Tree | Source | Binary |
| --- | --- | --- |
| `/work/native` | `git archive f85abcc841ee47c4d7739e21d669fd062112414b` | `/work/native/benches/baselines/target/release/maximus-comparison` |
| `/work/baseline-source` | `git archive 9cb096a27962a2966ca75b30e643f9e466c5cc4b` | `/work/baseline-source/benches/baselines/target/release/maximus-comparison` |
| `/work/wouldblock-source` | The same baseline archive, with the experiment below | `/work/wouldblock-source/benches/baselines/target/release/maximus-comparison` |

The WouldBlock experiment changes only `compio-io` 0.10.1's `src/compat/sync_stream.rs` helper:

```diff
-fn would_block(msg: &str) -> io::Error {
-    io::Error::new(io::ErrorKind::WouldBlock, msg)
+fn would_block(_msg: &str) -> io::Error {
+    io::ErrorKind::WouldBlock.into()
 }
```

Copy that locked dependency into `/work/wouldblock-source/compio-io`, apply the change, and add `compio-io = { path = "../../compio-io" }` to `[patch.crates-io]` in that tree's `benches/baselines/Cargo.toml`. Update its benchmark lockfile once with `cargo update --manifest-path benches/baselines/Cargo.toml -p compio-io --offline` after making the locked dependencies available. Keep this modified dependency, manifest and lockfile in the archived experiment. This preserves the error kind but changes its diagnostic text; it is a comparison variant, not a product setting.

Before building, write each tree's `SOURCE_COMMIT`: the full commit for native and baseline, and `9cb096a27962a2966ca75b30e643f9e466c5cc4b+wouldblock` for the patched tree. From each staged tree on Linux, create its manifest with `find . -type f ! -name SOURCE_SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SOURCE_SHA256SUMS`. Do this before generating build outputs, and include any patched dependency and Cargo source configuration. Then build each tree with `cargo build --manifest-path benches/baselines/Cargo.toml --release --locked`. Hash checks detect subsequent changes to the recorded files; they do not establish that an arbitrary supplied binary was built from them, so retain the build commands and logs too.

Save this map locally as `artifacts/native-comparison-binaries.json`:

```json
{
  "native": {"binary": "/work/native/benches/baselines/target/release/maximus-comparison", "argument": "maximus", "source_root": "/work/native", "source_id": "f85abcc"},
  "baseline": {"binary": "/work/baseline-source/benches/baselines/target/release/maximus-comparison", "argument": "maximus", "source_root": "/work/baseline-source", "source_id": "9cb096a"},
  "wouldblock": {"binary": "/work/wouldblock-source/benches/baselines/target/release/maximus-comparison", "argument": "maximus", "source_root": "/work/wouldblock-source", "source_id": "9cb096a27962a2966ca75b30e643f9e466c5cc4b+wouldblock"}
}
```

The current k6 workload counts each iteration before its HTTP call. Both tracked runners require positive integer counts with `iteration_starts == iterations == http_reqs` for warmup and measurement, and record started/completed counts in new k6 rows. This workload makes exactly one HTTP request per iteration. Missing counters fail validation even if k6 exits zero and every completed response passes. The archived `f85abcc` workload predates this instrumentation and cannot satisfy the new gate.

Stage the current checkout's workloads separately from the frozen product source. The extra counter adds client-side work, so every variant in a future comparison must use the same staged workload. Kubernetes metadata records both workload hashes; the Docker runner records its mounted `load.js` hash. Existing historical results are not retroactively validated by the new counter.

```sh
kubectl --context "$CONTEXT" -n "$NAMESPACE" exec benchmark -c server -- mkdir -p /work/qualification-workloads
tar -C benches -cf - load.js wrk.lua | kubectl --context "$CONTEXT" -n "$NAMESPACE" exec -i benchmark -c server -- tar -xf - -C /work/qualification-workloads
```

Run from the checkout containing this runner, after selecting the context, namespace and physical CPU masks as described in the hardware setup:

```sh
python3 benches/kubernetes.py --context "$CONTEXT" --namespace "$NAMESPACE" \
  --label native-f85abcc-wrk --out artifacts/native-comparison \
  --source-root /work/native --source-id f85abcc \
  --binary-map artifacts/native-comparison-binaries.json \
  --workload-root /work/qualification-workloads --engines native,baseline,wouldblock \
  --generator wrk --workers 1,2,4 --server-cpus 0,2,4,6 --client-cpus 8,10,12,14 \
  --paths /plaintext,/echo --vus 128 --duration 30s --repeats 3 --perf
```

`--source-id` and `--binary-map` are optional. Source manifests are always verified before process control; an expected identity must match `SOURCE_COMMIT` exactly or be a commit prefix of at least seven hexadecimal digits. Per-engine source roots are also verified. An explicit `--workload-root` selects `load.js` and `wrk.lua` in that directory. For open-loop k6, use `--generator k6 --rate 5000 --arrival-vus 512` under a new label; `--arrival-vus` otherwise defaults to `--vus`. Offered rate and completed requests are recorded separately, and dropped arrivals fail the trial.

Size TLS client pools against admission limits. With the comparison binary’s 4 GiB reservation budget and default HTTP/2 limits, each TLS/H2 connection reserves 36,536,320 bytes, admitting 117 connections. The native comparison therefore uses `--vus 64 --arrival-vus 64` for TLS phases. A larger pool can leave connections waiting for admission and extend the test through k6’s graceful-stop period. Even a pool within admission capacity can drop fixed-rate arrivals during connection establishment; retain that failed target instead of counting dropped arrivals as responses or treating zero HTTP failures as a passing trial.

To resume, repeat the identical command with `--resume`. It checks the retained harness, options, binary map, verified source identities/manifests, binary hashes, workload hashes, and stable hardware/toolchain/container/CPU-allocation details before process control. Live CPU-frequency readings are retained as observations but do not block resume. Successful rows are skipped; failed rows remain and their retries get unique attempt IDs. Use a new label when changing the campaign. CSV checkpoints and complete metadata/summary files are replaced atomically, with file contents flushed to disk; raw client output is retained before result validation. A malformed result fails the campaign and is not converted into a successful trial.

`--sample-rss` adds roughly five-second process RSS/high-water and cgroup CPU observations during measured trials, saved separately as `*-rss.jsonl`. It adds control traffic and should be enabled consistently across comparisons. The CSV's `observed_rss_kib` remains the larger of its before/after observations, not a continuous peak. Raw evidence and failed attempts belong beside any reported tables; a locally simulated control test does not qualify Kubernetes execution or throughput.

The Kubernetes runner calculates each CPU percentage from that container's cgroup CPU delta divided by its own `server_sample_seconds` or `client_sample_seconds`, with one CPU equal to 100%. These intervals come from `/proc/uptime` readings adjacent to `cpu.stat`; missing, nonfinite or nonpositive intervals fail validation. For example, 36 CPU-seconds over a 15-second sampling interval is 240%, even when the client workload reports ten seconds. The intervals include control and profiling overhead around the workload. `measurement_wall_seconds` remains the controller's elapsed time from after the initial snapshots through the final snapshots; `cpu_us_per_request` remains the broader server cgroup CPU delta divided by completed requests.

Historical and in-flight qualification CSVs are preserved as recorded. Their older harness divided the broader cgroup CPU delta by the client's shorter reported workload duration. Those historical CPU percentages are duration-normalized accounting ratios and cannot by themselves establish average cores used or quota saturation. Do not combine them with the new interval-based percentages as if they measured the same quantity.
