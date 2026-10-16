# Linux hardware comparison

The Kubernetes runner uses a temporary two-container Pod on one Linux node. It is optional benchmark infrastructure; no cluster libraries enter Maximus. The source revision and binary are verified before each campaign. Use a dedicated machine for capacity claims. Existing node workloads, frequency changes, shared cache and loopback interrupt processing remain confounders on a shared host.

## Prepare the temporary Pod

Copy `benches/pod.yaml`, set its node selector, choose a new namespace and explicitly select the intended kubeconfig context. The setup block stops if that namespace already exists. The template requests four CPU cores per container and isolates process namespaces. It does not use host networking, host mounts or a service-account token. PERFMON/SYS_PTRACE and an unconfined seccomp profile allow io_uring and task-attached perf; the Pod does not need privileged mode.

```sh
set -eu
CONTEXT=your-context
NAMESPACE=maximus-bench
kubectl --context "$CONTEXT" create namespace "$NAMESPACE"
kubectl --context "$CONTEXT" -n "$NAMESPACE" apply -f benches/pod.yaml
kubectl --context "$CONTEXT" -n "$NAMESPACE" wait --for=condition=Ready pod/benchmark --timeout=180s
# The archive must correspond to the source commit being reported.
git diff --quiet && git diff --cached --quiet
git archive HEAD | kubectl --context "$CONTEXT" -n "$NAMESPACE" exec -i benchmark -c server -- tar -xf - -C /work
git rev-parse HEAD | kubectl --context "$CONTEXT" -n "$NAMESPACE" exec -i benchmark -c server -- sh -c 'cat > /work/SOURCE_COMMIT'
git ls-files -z | xargs -0 shasum -a 256 | kubectl --context "$CONTEXT" -n "$NAMESPACE" exec -i benchmark -c server -- sh -c 'cat > /work/SOURCE_SHA256SUMS'
kubectl --context "$CONTEXT" -n "$NAMESPACE" exec benchmark -c server -- sh -c 'apt-get update && apt-get install -y linux-perf python3 procps wrk && cd /work/benches/baselines && cargo build --release --locked'
```

Record the resolved image digests in each campaign’s environment metadata and use those digests when reproducing it. Floating image tags can differ by architecture and date. The checked-in qualification used Rust 1.96.1 on x86-64, and 1.96.0 on the ARM64 VM. Each comparison used one compiler for all competitors.

If registry DNS is unavailable inside the Pod, vendor the locked benchmark and product dependency graphs into an **ignored temporary directory**, copy that directory with `COPYFILE_DISABLE=1 tar --no-xattrs`, and use Cargo's printed source-replacement configuration under `/work/.cargo/config.toml`. Use `cargo vendor --locked --manifest-path benches/baselines/Cargo.toml --sync Cargo.toml artifacts/dependencies`. Do not commit this generated dependency cache. The narrow existing h3 patch remains part of the source archive.

The k6 image uses musl. The runner invokes Debian's taskset and wrk through the shared glibc loader. On the qualified x86-64 Debian image, copy these runtime files inside the server container:

```sh
kubectl --context "$CONTEXT" -n "$NAMESPACE" exec benchmark -c server -- sh -c '
mkdir -p /work/tools
cp /usr/bin/taskset /usr/bin/wrk /lib64/ld-linux-x86-64.so.2 /work/tools/
for library in libc.so.6 libluajit-5.1.so.2 libpthread.so.0 libm.so.6 libssl.so.3 libcrypto.so.3 libgcc_s.so.1; do cp /usr/lib/x86_64-linux-gnu/$library /work/tools/; done
cp /work/benches/wrk.lua /work/benches/load.lua
lscpu -e=CPU,CORE,SOCKET,ONLINE
'
```

The runner currently targets this x86-64 layout. Select physically disjoint server/client cores from the printed topology; adjacent CPU numbers may be SMT siblings. The qualified masks were server `0,2,4,6` and client `8,10,12,14`. The process affinity mask contains the first N server cores for N workers; it does not force each framework's worker to a separate core. Cgroup quotas must permit the requested worker count. Background workloads can still execute on those cores.

## Run and remove

```sh
python3 benches/kubernetes.py --context "$CONTEXT" --namespace "$NAMESPACE" --label qualified-wrk --generator wrk --perf --workers 1,2,4 --paths /plaintext --duration 10s --repeats 3
python3 benches/kubernetes.py --context "$CONTEXT" --namespace "$NAMESPACE" --label qualified-k6 --workers 1 --paths /plaintext,/echo --duration 10s --repeats 3
python3 benches/kubernetes.py --context "$CONTEXT" --namespace "$NAMESPACE" --label qualified-arrival --workers 1 --paths /plaintext --rate 5000 --duration 10s --repeats 3
python3 benches/kubernetes.py --context "$CONTEXT" --namespace "$NAMESPACE" --label qualified-sustained --generator wrk --perf --workers 1 --engines maximus --paths /plaintext --duration 300s --repeats 1
kubectl --context "$CONTEXT" delete namespace "$NAMESPACE"
```

Results are saved locally before cleanup. The runner records client completion atomically inside the container and observes it through bounded control calls. An ambiguous launch is never resubmitted. It terminates owned client, profiler and server processes in `finally` blocks, including failed warmups, but deliberately leaves Pod deletion to the explicit final command. To exercise validator failure, run `--generator wrk --paths /missing --engines maximus --workers 1 --repeats 1`; the warmup must fail, with no owned PID files left under `/work/`.

Both clients validate every completed response. wrk also fails on connection/read/write/timeout errors, bad bodies/status, missing response validation or zero responses. k6 uses the thresholds in `load.js`. An open-loop dropped iteration is an unsent arrival. The fixed 128 connections/VUs, short trials and same-host loopback constrain interpretation.

wrk and k6 latency tables stay separate: wrk applies its histogram correction before Lua receives percentiles, whereas k6 reports request-duration observations. Both exclude some connection-establishment work. See [wrk's implementation](https://github.com/wg/wrk/blob/master/src/wrk.c#L155-L181). Each trial has a separate three-second warmup client; measured connections start fresh.

The `--perf` mode attaches to existing server TIDs after warmup, enables/disables counters with acknowledged FIFO controls around the measured client, and records cycles, instructions, cache misses and task-clock. Default perf inheritance may also include subsequently created descendants; the counters do not cover the whole system. These counters include kernel/interrupt execution attributed while attached task contexts are active. Cgroup CPU excludes interrupt accounting and includes small runner/profiler overhead in the server container. They measure different scopes; neither is a pure application instruction count. CPU percentages use one CPU as 100%, with a slightly wider snapshot interval than the client duration. RSS uses process observations before/after the trial and is not a sampled peak. Raw snapshots retain per-CPU and per-thread diagnostics.
