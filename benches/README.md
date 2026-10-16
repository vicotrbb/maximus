# Reproducible performance checks

The comparison servers are an isolated Cargo workspace. Product builds do not resolve Axum, Actix or their benchmark dependencies. All Rust servers use thin LTO and one codegen unit. Commit both lockfiles when changing dependencies.

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

The runner assumes three Linux CPUs: server CPU 0 (0–1 with two workers), client CPU 2. Use a dedicated host for performance claims. Existing background services must be recorded when the host cannot be dedicated. It warms the server for 3 seconds in a separate k6 process, then starts new client connections for measurement. TLS connection establishment therefore contributes to cold-start pressure; request-duration metrics do not include the entire handshake. It rotates server order across repeats, checks every status/body/protocol, records p50/p99/p99.9, achieved requests/sec, failure counts, dropped iterations, server cgroup CPU microseconds/request, server/client CPU utilization, and process-tree RSS. Use `--sample-seconds 5` for periodic server memory samples during a sustained run; short comparisons avoid this sampling overhead. `--client-cpus` can allocate more client cores when available. CPU percentages use one logical CPU as 100%. Cgroup snapshots bracket a slightly wider interval than k6 reports, so percentages are approximate and can exceed the assigned core count briefly. RSS is the sum of process-tree observations, which double-counts shared pages for multi-process servers such as Nginx; it is not unique physical memory. Tables distinguish post-run RSS from the largest observation, which is not a continuous peak. Raw JSON, logs and environment/image digests go to `artifacts/bench/`. Fixed arrival rates are open-loop; zero means a closed-loop constant-VU scenario. [k6 arrival-rate behavior](https://grafana.com/docs/k6/latest/using-k6/scenarios/executors/constant-arrival-rate/) and [threshold semantics](https://grafana.com/docs/k6/latest/using-k6/thresholds/) define the distinction. Dropped iterations are a load-generator shortfall and must not be counted as completed server requests.

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

Run `python3 benches/summarize.py` after the full `qualified-*` campaign to regenerate compact tables and trial CSVs. It rejects missing trials and preserves failed trial counts. Raw artifacts remain ignored.

## Additional qualification commands

```sh
# Provision enough VUs for a common offered load, preserving any earlier failed run.
python3 benches/run.py --label qualified-arrival-provisioned --paths /plaintext --rates 5000 --vus 512 --duration 10s --repeats 3
# Give the VM client two CPUs while keeping the server on CPU 0.
python3 benches/run.py --label qualified-capacity --engines maximus,epoll,hyper,axum,actix --paths /plaintext --client-cpus 1,2 --duration 30s --repeats 3
python3 benches/summarize-hardware.py
```

The checked-in [report](results/README.md), [VM tables](results/tables.md) and [hardware tables](results/hardware-tables.md) retain failed trials and explain the observed limits. Hardware setup and cleanup are documented in [HARDWARE.md](HARDWARE.md). Benchmark infrastructure and generated caches never enter the product dependency graph.

To redraw the scaling chart from the checked-in trial CSV, install Matplotlib in an optional local Python environment and run `python3 benches/plot.py`. Matplotlib is not a product or benchmark-runner dependency.
