#!/usr/bin/env python3
"""Run validated k6/wrk workloads in an isolated Linux benchmark Pod."""

import argparse, csv, hashlib, io, json, math, os, pathlib, re, shlex, subprocess, time, uuid

HARNESS_SOURCE = pathlib.Path(__file__).read_bytes()
P = argparse.ArgumentParser(description=__doc__)
P.add_argument("--context", required=True)
P.add_argument("--namespace", required=True)
P.add_argument("--label", default="homelab")
P.add_argument("--resume", action="store_true")
P.add_argument("--workers", default="1,2,4")
P.add_argument("--paths", default="/plaintext,/echo")
P.add_argument("--duration", default="30s")
P.add_argument("--repeats", type=int, default=3)
P.add_argument("--generator", default="k6", choices=["k6", "wrk"])
P.add_argument("--perf", action="store_true")
P.add_argument("--sample-rss", action="store_true")
P.add_argument("--rate", type=int, default=0)
P.add_argument("--vus", type=int, default=128)
P.add_argument("--arrival-vus", type=int, help="arrival-rate VUs; defaults to --vus")
P.add_argument("--protocol", default="plain", choices=["plain", "h1", "h2"])
P.add_argument("--engines", default="maximus,epoll,hyper,axum,actix")
P.add_argument("--server-cpus", default="0,2,4,6")
P.add_argument("--out", default="artifacts/homelab")
P.add_argument("--source-root", default="/work")
P.add_argument("--source-id", help="expected SOURCE_COMMIT (full identity or 7+ digit commit prefix)")
P.add_argument("--binary-map", help="JSON labels to binary/argument and optional source_root/source_id")
P.add_argument("--workload-root", help="remote load.js/wrk.lua directory; default retains /work/benches/load.lua")
P.add_argument("--client-cpus", default="8,10,12,14")
A = P.parse_args()
if A.generator == "wrk" and (A.rate or A.protocol != "plain"):
    P.error("wrk campaigns use plaintext closed-loop only")
if A.rate < 0 or A.repeats < 1 or A.vus < 1 or (A.arrival_vus is not None and A.arrival_vus < 1):
    P.error("rate must be nonnegative; repeats and VUs must be positive")
if not re.fullmatch(r"(?:[0-9]+(?:\.[0-9]+)?)[smh]", A.duration) or not math.isfinite(float(A.duration[:-1])) or float(A.duration[:-1]) <= 0:
    P.error("duration must be a positive number followed by s, m or h")
engines = A.engines.split(",")
if len(set(engines)) != len(engines) or any(not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", e) for e in engines + [A.label]):
    P.error("label and distinct engine names must contain only letters, digits, dots, underscores or hyphens")
try:
    workers = [int(w) for w in A.workers.split(",")]
    if not workers or min(workers) < 1 or max(workers) > len(A.server_cpus.split(",")) or len(set(workers)) != len(workers):
        raise ValueError()
    BINARIES = json.loads(pathlib.Path(A.binary_map).read_text()) if A.binary_map else {
        e: {"binary": "/work/benches/baselines/target/release/maximus-comparison", "argument": e} for e in engines
    }
    if not isinstance(BINARIES, dict):
        raise ValueError()
    for engine in engines:
        entry = BINARIES[engine]
        if not isinstance(entry, dict) or set(entry) - {"binary", "argument", "source_root", "source_id"}:
            raise ValueError()
        if not isinstance(entry.get("binary"), str) or not entry["binary"].startswith("/") or not isinstance(entry.get("argument"), str) or not entry["argument"]:
            raise ValueError()
        if "source_root" in entry and (not isinstance(entry["source_root"], str) or not entry["source_root"].startswith("/")):
            raise ValueError()
        if "source_id" in entry and (not isinstance(entry["source_id"], str) or not entry["source_id"]):
            raise ValueError()
except (OSError, ValueError, KeyError, TypeError) as error:
    P.error(f"invalid worker list or binary map: {error}")
if not A.source_root.startswith("/") or (A.workload_root is not None and not A.workload_root.startswith("/")):
    P.error("remote source/workload roots must be absolute")
if any(not path.startswith("/") or any(ord(c) < 32 for c in path) for path in A.paths.split(",")):
    P.error("paths must be absolute HTTP paths without control characters")
OUT = pathlib.Path(A.out)
OUT.mkdir(parents=True, exist_ok=True)
ENVIRONMENT = OUT / f"{A.label}-environment.json"
HARNESS_HASH = hashlib.sha256(HARNESS_SOURCE).hexdigest()
old_meta = None
if A.resume:
    try:
        old_meta = json.loads(ENVIRONMENT.read_text())
        if old_meta["harness_sha256"] != HARNESS_HASH:
            raise ValueError("resume requires the same retained harness")
        previous = {k: v for k, v in old_meta["args"].items() if k != "resume"}
        requested = {k: v for k, v in vars(A).items() if k != "resume"}
        if previous != requested or old_meta["binary_map"] != BINARIES:
            raise ValueError("resume configuration or binary map changed")
    except (OSError, ValueError, KeyError, TypeError) as error:
        P.error(f"cannot resume campaign: {error}")
elif any(OUT.glob(f"{A.label}-*")) or (OUT / f"{A.label}.csv").exists():
    P.error("campaign label already used; choose a unique label or explicit --resume")
WORKLOADS = [f"{A.workload_root or '/work/benches'}/load.js", f"{A.workload_root}/wrk.lua" if A.workload_root else "/work/benches/load.lua"]


def retain(path, text):
    """Replace complete local evidence atomically; never truncate the previous checkpoint."""
    temporary = path.with_name(path.name + ".tmp-" + uuid.uuid4().hex)
    with temporary.open("w") as file:
        file.write(text)
        file.flush()
        os.fsync(file.fileno())
    temporary.replace(path)


def passed(row):
    requests = float(row["requests"])
    if not math.isfinite(requests) or requests <= 0 or requests != int(requests):
        raise ValueError("invalid recorded request count")
    return int(row["exit"]) == 0 and all(float(row[k]) == 0 for k in ("failures", "checks_failed", "network_errors", "dropped"))

BASE = ["kubectl", "--context", A.context, "-n", A.namespace, "exec", "benchmark"]


def command(container, *args, check=True, timeout=30):
    result = subprocess.run(
        BASE + ["-c", container, "--", *map(str, args)],
        text=True,
        capture_output=True,
        check=False,
        timeout=timeout,
    )
    if check and result.returncode:
        raise RuntimeError(f"control command failed in {container}: {args!r}\n{result.stdout}\n{result.stderr}")
    return result


def stop(container, name=None, signal="TERM"):
    pid_file = f"/work/{name or container}.pid"
    script = """test ! -f {file} && exit 0
pid=$(cat {file})
kill -{signal} "$pid" 2>/dev/null || true
for attempt in $(seq 1 50); do
 if ! kill -0 "$pid" 2>/dev/null || [ "$(awk '{{print $3}}' /proc/$pid/stat 2>/dev/null)" = Z ]; then rm -f {file}; exit 0; fi
 sleep 0.1
done
kill -KILL "$pid" 2>/dev/null || true
sleep 0.1
if kill -0 "$pid" 2>/dev/null && [ "$(awk '{{print $3}}' /proc/$pid/stat 2>/dev/null)" != Z ]; then exit 1; fi
rm -f {file}""".format(file=shlex.quote(pid_file), signal=signal)
    command(container, "sh", "-c", script)


def profile_control(action):
    command(
        "server",
        "timeout",
        "5",
        "sh",
        "-c",
        f"echo {action} > /work/perf-control; read ack < /work/perf-ack",
    )


def profile_start(key):
    script = "rm -f /work/perf-control /work/perf-ack; mkfifo /work/perf-control /work/perf-ack; "
    script += "tids=$(ls /proc/$(cat /work/server.pid)/task | paste -sd,); "
    script += (
        shlex.join(
            [
                "perf",
                "stat",
                "-D",
                "-1",
                "--control",
                "fifo:/work/perf-control,/work/perf-ack",
                "-x,",
                "-e",
                "cycles,instructions,cache-misses,task-clock,cpu-migrations",
                "-o",
                f"/work/{key}-perf.csv",
            ]
        )
        + ' -t "$tids" >/work/perf.log 2>&1 & echo $! >/work/perf.pid'
    )
    command("server", "sh", "-c", script)


def snapshot(container):
    script = "cat /sys/fs/cgroup/cpu.stat"
    if container == "server":
        script += "; cat /proc/$(cat /work/server.pid)/status; cat /proc/$(cat /work/server.pid)/stat; ps -T -p $(cat /work/server.pid) -o pid,tid,comm; head -n 17 /proc/stat; for t in /proc/$(cat /work/server.pid)/task/*; do echo TID $t; cat $t/schedstat; done"
    values = {}
    raw = command(container, "sh", "-c", script).stdout
    with (OUT / f"{A.label}-snapshots.log").open("a") as f:
        f.write(container + " " + str(time.time()) + "\n" + raw + "\n")
    for line in raw.splitlines():
        parts = line.replace(":", "").split()
        if len(parts) > 1 and parts[0] in (
            "usage_usec",
            "VmRSS",
            "nr_throttled",
            "throttled_usec",
        ):
            values[parts[0]] = int(parts[1])
    return values


def workload(key, path, duration, rate, log):
    env = [
        "env",
        f"PATHNAME={path}",
        f"DURATION={duration}",
        f"VUS={(A.arrival_vus or A.vus) if rate else A.vus}",
        f"RESULT=/work/{key}.json",
    ]
    if rate:
        env.append(f"RATE={rate}")
    if A.protocol != "plain":
        env += [
            "SSL_CERT_FILE=/work/cert.pem",
            "URL=https://localhost:8080",
            f"PROTOCOL={A.protocol}",
        ]
    command("client", "rm", "-f", f"/work/{key}.json")
    client = (
        ["k6", "run", "--quiet", WORKLOADS[0]]
        if A.generator == "k6"
        else [
            "/work/tools/ld-linux-x86-64.so.2",
            "--library-path",
            "/work/tools",
            "/work/tools/wrk",
            "-t4",
            f"-c{A.vus}",
            "-d" + duration,
            "--latency",
            "-s",
            WORKLOADS[1],
            "http://127.0.0.1:8080",
        ]
    )
    launch = shlex.join(
        [
            *env,
            "/work/tools/ld-linux-x86-64.so.2",
            "--library-path",
            "/work/tools",
            "/work/tools/taskset",
            "-c",
            A.client_cpus,
            *client,
        ]
    )
    # Persist completion inside the container; a dropped exec stream must not be
    # confused with a failed client or cause a duplicate workload submission.
    output = f"/work/{key}.log"
    done = f"/work/{key}.exit"
    command("client", "rm", "-f", output, done, done + ".tmp", "/work/client.pid")
    script = f'( {launch} > {shlex.quote(output)} 2>&1 & pid=$!; echo $pid > /work/client.pid; wait $pid; status=$?; echo $status > {shlex.quote(done+".tmp")}; mv {shlex.quote(done+".tmp")} {shlex.quote(done)} ) </dev/null >/dev/null 2>&1 &'
    seconds = float(duration[:-1]) * {"s": 1, "m": 60, "h": 3600}[duration[-1]]
    deadline = time.monotonic() + seconds + 60
    try:
        started = command("client", "sh", "-c", script, check=False)
        log.write(started.stderr)
    except subprocess.TimeoutExpired:
        log.write(
            "Launch control timed out; observing the submitted client without resubmission.\n"
        )
    log.flush()
    last_sample = time.monotonic()
    while time.monotonic() < deadline:
        if A.sample_rss and not key.endswith("-warmup") and time.monotonic() - last_sample >= 5:
            raw_sample = command("server", "sh", "-c", "cat /proc/$(cat /work/server.pid)/status; cat /sys/fs/cgroup/cpu.stat").stdout
            sample = {"time_unix": time.time()}
            for sample_line in raw_sample.splitlines():
                values = sample_line.replace(":", "").split()
                if len(values) > 1 and values[0] in ("VmRSS", "VmHWM", "usage_usec", "nr_throttled", "throttled_usec"):
                    sample[values[0]] = int(values[1])
            with (OUT / f"{key}-rss.jsonl").open("a") as sample_file:
                sample_file.write(json.dumps(sample) + "\n")
            last_sample = time.monotonic()
        try:
            status = command(
                "client",
                "sh",
                "-c",
                "test -f " + shlex.quote(done) + " && cat " + shlex.quote(done),
                check=False,
                timeout=min(30, max(0.1, deadline - time.monotonic())),
            )
        except subprocess.TimeoutExpired:
            continue
        if status.returncode == 0:
            log.write(command("client", "cat", output).stdout)
            log.flush()
            retained = command("client", "cat", f"/work/{key}.json", check=False)
            if retained.returncode == 0:
                retain(OUT / f"{key}.json", retained.stdout)
            return int(status.stdout.strip())
        time.sleep(1)
    raise RuntimeError("client completion was not recorded before its deadline")


def verify_source(root, expected=None):
    commit = command("server", "cat", root + "/SOURCE_COMMIT").stdout.strip()
    if not commit or (expected and not (commit == expected or re.fullmatch(r"[0-9a-fA-F]{7,40}", expected) and re.fullmatch(r"[0-9a-fA-F]{40}", commit) and commit.lower().startswith(expected.lower()))):
        raise RuntimeError(f"source identity mismatch at {root}: expected {expected!r}, recorded {commit!r}")
    verification = command("server", "sh", "-c", "cd " + shlex.quote(root) + " && sha256sum -c SOURCE_SHA256SUMS")
    return {"commit": commit, "manifest_sha256": command("server", "sha256sum", root + "/SOURCE_SHA256SUMS").stdout, "verification": verification.stdout}


sources = {A.source_root: verify_source(A.source_root, A.source_id)}
for engine in engines:
    entry = BINARIES[engine]
    root = entry.get("source_root", A.source_root)
    if root not in sources or entry.get("source_id"):
        sources[root] = verify_source(root, entry.get("source_id"))
metadata = {
    "args": vars(A),
    "source_identity": sources[A.source_root]["commit"],
    "source_commit": sources[A.source_root]["commit"],
    "sources": sources,
    "source_root": A.source_root,
    "binary_map": BINARIES,
    "harness_commit": subprocess.check_output(
        ["git", "rev-parse", "HEAD"], text=True
    ).strip(),
    "harness_worktree_dirty": bool(
        subprocess.check_output(["git", "status", "--porcelain"], text=True).strip()
    ),
    "date_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "hardware": command("server", "lscpu").stdout,
    "rustc": command("server", "rustc", "--version").stdout,
    "cgroup_cpu_max": {
        c: command(c, "cat", "/sys/fs/cgroup/cpu.max").stdout
        for c in ["server", "client"]
    },
}
metadata["workload_sha256"] = command(
    "server", "sha256sum", WORKLOADS[0], WORKLOADS[1]
).stdout
metadata["container_images"] = [
    {k: c[k] for k in ("name", "image", "imageID")}
    for c in json.loads(
        subprocess.check_output(
            [
                "kubectl",
                "--context",
                A.context,
                "-n",
                A.namespace,
                "get",
                "pod",
                "benchmark",
                "-o",
                "json",
            ],
            text=True,
            timeout=30,
        )
    )["status"]["containerStatuses"]
]
metadata["toolchain"] = command(
    "server",
    "sh",
    "-c",
    "perf --version; wrk --version; dpkg-query -W linux-perf wrk libc6 libluajit-5.1-2 libssl3 libgcc-s1 util-linux; sha256sum /work/tools/*",
).stdout
metadata.update(
    harness_sha256=hashlib.sha256(HARNESS_SOURCE).hexdigest(),
    source_manifest_sha256=command(
        "server", "sha256sum", A.source_root + "/SOURCE_SHA256SUMS"
    ).stdout,
    binary_sha256=command(
        "server",
        "sha256sum",
        *[BINARIES[e]["binary"] for e in A.engines.split(",")],
    ).stdout,
    cpu_topology=command("server", "lscpu", "-e=CPU,CORE,SOCKET,ONLINE").stdout,
    effective_cpus={
        c: command(c, "cat", "/sys/fs/cgroup/cpuset.cpus.effective").stdout
        for c in ["server", "client"]
    },
)
# lscpu includes live frequency readings; retain them above without making them
# part of the identity needed to append comparable trials to an existing CSV.
metadata["stable_environment"] = {
    "hardware": "\n".join(
        line.strip() for line in metadata["hardware"].splitlines()
        if line.strip() and not any(
            volatile in line.partition(":")[0].lower() for volatile in ("mhz", "bogomips")
        )
    ),
    "rustc": metadata["rustc"].strip(),
    "cpu_topology": metadata["cpu_topology"].strip(),
    "effective_cpus": {k: v.strip() for k, v in metadata["effective_cpus"].items()},
    "cgroup_cpu_max": {k: v.strip() for k, v in metadata["cgroup_cpu_max"].items()},
    "container_images": sorted(metadata["container_images"], key=lambda image: image["name"]),
    "toolchain": metadata["toolchain"].strip(),
}
if A.resume:
    for name in ("source_identity", "sources", "binary_sha256", "workload_sha256", "stable_environment"):
        if metadata[name] != old_meta[name]:
            raise RuntimeError(f"resume {name} changed")
rows = []
completed = set()
if A.resume and (OUT / f"{A.label}.csv").exists():
    try:
        with (OUT / f"{A.label}.csv").open() as file:
            rows = list(csv.DictReader(file))
        attempts = set()
        for row in rows:
            if not row["attempt_key"].startswith(A.label + "-") or row["attempt_key"] in attempts:
                raise ValueError("invalid or duplicate attempt identity")
            attempts.add(row["attempt_key"])
            if row["engine"] not in engines or row["path"] not in A.paths.split(",") or int(row["workers"]) not in workers or not 0 <= int(row["repeat"]) < A.repeats or row["protocol"] != A.protocol or row["generator"] != A.generator or int(row["offered_rps"]) != A.rate:
                raise ValueError("resume row does not match campaign")
            if passed(row):
                completed.add((row["engine"], row["path"], int(row["workers"]), int(row["repeat"])))
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise RuntimeError(f"invalid resume rows: {error}") from error
if A.resume:
    retain(OUT / f"{A.label}-resume-{uuid.uuid4().hex}.json", json.dumps(metadata, indent=2) + "\n")
else:
    retain(OUT / f"{A.label}-harness.py", HARNESS_SOURCE.decode())
    retain(OUT / f"{A.label}-binaries.json", json.dumps(BINARIES, indent=2) + "\n")
    retain(ENVIRONMENT, json.dumps(metadata, indent=2) + "\n")
stop("client")
stop("server", "perf", "INT")
stop("server")

try:
    for repeat in range(A.repeats):
        engines = A.engines.split(",")
        engines = engines[repeat % len(engines) :] + engines[: repeat % len(engines)]
        for workers in map(int, A.workers.split(",")):
            cpus = ",".join(A.server_cpus.split(",")[:workers])
            assert len(cpus.split(",")) == workers
            for path in A.paths.split(","):
                for engine in engines:
                    if (engine, path, workers, repeat) in completed:
                        continue
                    slug = re.sub(r"[^A-Za-z0-9_.-]+", "-", path).strip("-")[:80] or "root"
                    key = f"{A.label}-{engine}-{slug}-w{workers}-n{repeat}-a{uuid.uuid4().hex}"
                    env = [f"WORKERS={workers}"]
                    if A.protocol != "plain":
                        env += [
                            "MAXIMUS_CERT=/work/cert.pem",
                            "MAXIMUS_KEY=/work/key.pem",
                            f"PROTOCOL={A.protocol}",
                        ]
                    launch = shlex.join(
                        [
                            "nohup",
                            "env",
                            *env,
                            "taskset",
                            "-c",
                            cpus,
                            BINARIES[engine]["binary"],
                            BINARIES[engine]["argument"],
                        ]
                    )
                    command(
                        "server",
                        "sh",
                        "-c",
                        launch
                        + " </dev/null >/work/server.log 2>&1 & echo $! >/work/server.pid",
                    )
                    try:
                        for attempt in range(100):
                            if (
                                "ready"
                                in command("server", "cat", "/work/server.log").stdout
                            ):
                                break
                            time.sleep(0.1)
                        else:
                            raise RuntimeError(
                                command("server", "cat", "/work/server.log").stdout
                            )
                        with (OUT / f"{key}.log").open("w") as log:
                            if workload(key + "-warmup", path, "3s", 0, log) != 0:
                                raise RuntimeError("warmup failed")
                            if A.perf:
                                profile_start(key)
                            before = {c: snapshot(c) for c in ["server", "client"]}
                            started = time.monotonic()
                            if A.perf:
                                profile_control("enable")
                            code = workload(key, path, A.duration, A.rate, log)
                            if A.perf:
                                profile_control("disable")
                            after = {c: snapshot(c) for c in ["server", "client"]}
                            wall = time.monotonic() - started
                            if A.perf:
                                stop("server", "perf", "INT")
                        raw = command("client", "cat", f"/work/{key}.json").stdout
                        retain(OUT / f"{key}.json", raw)
                        summary = json.loads(raw)
                        metrics = summary["metrics"]
                        def metric(name, key, optional=False):
                            # k6 omits counters that were never emitted (for example,
                            # dropped arrivals); missing measurement fields are invalid.
                            if optional and name not in metrics:
                                return 0
                            try:
                                value = metrics[name]["values"][key]
                            except (KeyError, TypeError) as error:
                                raise ValueError(f"missing required client metric {name}.{key}") from error
                            if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value < 0:
                                raise ValueError(f"invalid client metric {name}.{key}: {value!r}")
                            return value
                        count = metric("http_reqs", "count")
                        if count <= 0 or count != int(count):
                            raise ValueError("client completed no valid request count")
                        cpu = {
                            c: after[c]["usage_usec"] - before[c]["usage_usec"]
                            for c in before
                        }
                        measured_seconds = summary["state"]["testRunDurationMs"] / 1000
                        if not math.isfinite(measured_seconds) or measured_seconds <= 0:
                            raise ValueError("invalid client measurement duration")
                        if metric("checks", "passes") + metric("checks", "fails") != count:
                            raise ValueError("not every response was validated")
                        row = dict(
                            attempt_key=key,
                            generator=A.generator,
                            engine=engine,
                            path=path,
                            workers=workers,
                            protocol=A.protocol,
                            offered_rps=A.rate,
                            repeat=repeat,
                            requests=count,
                            rps=metric("http_reqs", "rate"),
                            p50_ms=metric("http_req_duration", "med"),
                            p99_ms=metric("http_req_duration", "p(99)"),
                            p999_ms=metric("http_req_duration", "p(99.9)"),
                            failures=metric("http_req_failed", "passes"),
                            checks_failed=metric("checks", "fails"),
                            dropped=metric("dropped_iterations", "count", optional=True),
                            network_errors=metric("network_errors", "count", optional=True),
                            cpu_us_per_request=cpu["server"] / max(count, 1),
                            server_cpu_percent=cpu["server"] / measured_seconds / 10000,
                            client_cpu_percent=cpu["client"] / measured_seconds / 10000,
                            measurement_wall_seconds=wall,
                            rss_kib=after["server"]["VmRSS"],
                            observed_rss_kib=max(
                                before["server"]["VmRSS"], after["server"]["VmRSS"]
                            ),
                            server_throttled_usec=after["server"]["throttled_usec"]
                            - before["server"]["throttled_usec"],
                            client_throttled_usec=after["client"]["throttled_usec"]
                            - before["client"]["throttled_usec"],
                            exit=code,
                        )
                        if A.perf:
                            raw_perf = command(
                                "server", "cat", f"/work/{key}-perf.csv"
                            ).stdout
                            retain(OUT / f"{key}-perf.csv", raw_perf)
                            events = {
                                r[2]: float(r[0])
                                for r in csv.reader(raw_perf.splitlines())
                                if len(r) > 2
                                and r[2]
                                in (
                                    "cycles",
                                    "instructions",
                                    "cache-misses",
                                    "task-clock",
                                    "cpu-migrations",
                                )
                            }
                            assert (
                                set(events).issuperset({
                                    "cycles",
                                    "instructions",
                                    "cache-misses",
                                    "task-clock",
                                })
                                and events["cycles"] > 0
                            ), raw_perf
                            row.update(
                                cycles_per_request=events["cycles"] / count,
                                instructions_per_request=events["instructions"] / count,
                                cache_misses_per_request=events["cache-misses"] / count,
                                profile_task_clock_ms=events["task-clock"],
                                cpu_migrations=events.get("cpu-migrations", 0),
                            )
                        rows.append(row)
                        file = io.StringIO()
                        writer = csv.DictWriter(file, fieldnames=list(row))
                        writer.writeheader()
                        writer.writerows(rows)
                        retain(OUT / f"{A.label}.csv", file.getvalue())
                        print(json.dumps(row), flush=True)
                        if not passed(row):
                            raise RuntimeError("measured workload failed; raw evidence retained")
                    finally:
                        try:
                            stop("client")
                        finally:
                            try:
                                stop("server", "perf", "INT")
                            finally:
                                stop("server")
finally:
    try:
        stop("client")
    finally:
        try:
            stop("server", "perf", "INT")
        finally:
            stop("server")
