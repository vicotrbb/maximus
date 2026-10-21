#!/usr/bin/env python3
"""Reconcile the completed post-cache H2 A/A control; never generates load."""

import csv
import hashlib
import json
import math
import re
import statistics
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RAW = ROOT / "artifacts/bench"
ART = ROOT / "artifacts/performance-2026-10-20"
OUT = ROOT / "benches/results"
LABEL = "post-cache-aa-h2"
BASELINE = "be05e0aea314cfdda839441c4c3c923e4cae9c8100f4836f382d7ca1e703e8f2"
ENGINES = ["maximus", "hyper", "axum"]
FIELDS = ["rps", "cpu_us_per_request", "p50_ms", "p99_ms", "p999_ms", "rss_kib", "peak_rss_kib", "cpu_interval_seconds", "server_cpu_percent", "client_cpu_percent"]
observed = set()


def read(path):
    observed.add(path)
    return path.read_text()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def close(a, b):
    return math.isclose(float(a), float(b), rel_tol=1e-9, abs_tol=1e-8)


env = json.loads(read(RAW / f"{LABEL}-environment.json"))
expected_args = dict(engines="maximus,hyper,axum", paths="/echo", rates="0", body_bytes=131072, workers=1, duration="5s", repeats=6, vus=64, label=LABEL, client_cpus="1,2", sample_seconds=0, protocol="h2", binary="/target/oct20-core-h2-baseline/maximus-comparison", reuse_certificate=False, generator="k6")
require(env["args"] == expected_args, "unexpected control settings")
require(env["binary_sha256"].split()[0] == BASELINE, "unexpected executable identity")
require(env["source_commit"] == "b5bb664bd6faf9b417cd8af5235ec12e7f0037de", "unexpected owning commit")
require(env["image_ids"] == {"grafana/k6:1.6.0": "sha256:be2a9ceb1b6ffc573277af4157135882a6d6433968b41e858ab02c5fd5847532", "rust:1.96-bookworm": "sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663"}, "unexpected captured image IDs")
require(env["certificate_sha256"] == "d2bb8d5f5222cc6f96483eda99ae6bbc6bb2eef1a01b7ac22d46fc0c296d3f8d", "unexpected captured certificate identity")
require(env["cpus"] == "3" and env["network"] == {"server": "none", "client": "container:<server>"}, "unexpected host/network configuration")
workload = read(ROOT / "benches/load.js")
read(ROOT / "benches/run.py")
require(hashlib.sha256(workload.encode()).hexdigest() == env["workload_sha256"], "workload differs from captured hash")
manifest = json.loads(read(ART / "core-h2-baseline-manifest.json"))
for filename in ["benches/baselines/src/main.rs", "benches/baselines/Cargo.lock", "benches/load.js"]:
    require(hashlib.sha256(read(ART / "core-h2-baseline-source" / filename).encode()).hexdigest() == manifest[filename], "frozen source differs from manifest")
build = read(ART / "core-h2-baseline-build.log")
require(re.findall(r"^([0-9a-f]{64})\s+\S*/maximus-comparison$", build, re.MULTILINE) == [BASELINE] and "Finished `release` profile" in build, "baseline build identity mismatch")
lock = tomllib.loads(read(ART / "core-h2-baseline-source/benches/baselines/Cargo.lock"))
versions = {p["name"]: p["version"] for p in lock["package"] if p["name"] in ["hyper", "axum", "rustls", "tokio"]}
rows = list(csv.DictReader(read(RAW / f"{LABEL}.csv").splitlines()))
require(len(rows) == 18, "expected eighteen engine observations")
expected_order = [(repeat, engine) for repeat in range(6) for engine in ENGINES[repeat % 3:] + ENGINES[:repeat % 3]]
require([(int(r["repeat"]), r["engine"]) for r in rows] == expected_order, "unexpected rotating order")
validation = []
for row in rows:
    require(all(row[k] == v for k, v in dict(body_bytes="131072", path="/echo", workers="1", protocol="h2", generator="k6", offered_rps="0").items()), "row settings differ")
    require(all(float(row[k]) == 0 for k in ["failures", "checks_failed", "network_errors", "dropped", "exit"]), "failed engine observation")
    require(all(math.isfinite(float(row[k])) and float(row[k]) >= 0 for k in FIELDS), "invalid numeric field")
    stem = f"{LABEL}-{row['engine']}-echo-w1-r0-n{row['repeat']}"
    phases = {}
    for suffix, phase in [("-warmup", "warmup"), ("", "measurement")]:
        summary = json.loads(read(RAW / f"{stem}{suffix}.json"))
        metrics = summary["metrics"]
        value = lambda name, field: metrics[name]["values"][field]
        count = value("http_reqs", "count")
        require(isinstance(count, (int, float)) and math.isfinite(count) and count > 0 and count == int(count), "invalid response count")
        require(value("checks", "passes") == value("iteration_starts", "count") == value("iterations", "count") == count, "incomplete or unchecked iterations")
        require(value("checks", "fails") == value("http_req_failed", "passes") == 0, "raw response validation failed")
        require(metrics.get("dropped_iterations", {}).get("values", {}).get("count", 0) == 0, "dropped iterations")
        checks = summary["root_group"]["checks"]
        require(len(checks) == 1 and checks[0]["name"] == "correct status, body and protocol" and checks[0]["passes"] == count and checks[0]["fails"] == 0, "unexpected response check")
        duration = summary["state"]["testRunDurationMs"]
        require(math.isfinite(duration) and duration >= (3000 if phase == "warmup" else 5000), "unexpected reported client duration")
        phases[phase] = dict(checked_responses=int(count), completed_iterations=int(count), reported_duration_ms=duration)
        if phase == "measurement":
            require(int(row["requests"]) == int(row["iterations_started"]) == int(row["iterations_completed"]) == count, "CSV count mismatch")
            for column, name, field in [("rps", "http_reqs", "rate"), ("p50_ms", "http_req_duration", "med"), ("p99_ms", "http_req_duration", "p(99)"), ("p999_ms", "http_req_duration", "p(99.9)")]:
                require(close(row[column], value(name, field)), "CSV/JSON metric mismatch")
    client = dict(line.split() for line in read(RAW / f"{stem}-client-cpu.txt").splitlines())
    interval = float(row["cpu_interval_seconds"])
    require(interval > 0 and close(row["client_cpu_percent"], 100 * int(client["usage_usec"]) / (interval * 1_000_000)), "client CPU arithmetic mismatch")
    require(close(row["server_cpu_percent"], 100 * float(row["cpu_us_per_request"]) * int(row["requests"]) / (interval * 1_000_000)), "server CSV arithmetic mismatch")
    validation.append(dict(engine=row["engine"], repeat=int(row["repeat"]), **phases))
log_rows = [json.loads(line) for line in read(ART / "post-cache-aa-h2.log").splitlines() if line.startswith('{"body_bytes":')]
require(len(log_rows) == 18, "campaign log row count differs")
for recorded, row in zip(log_rows, rows):
    require(all(str(value) == row[key] or (isinstance(value, (int, float)) and close(value, row[key])) for key, value in recorded.items()), "campaign log differs from CSV")
for path in RAW.glob(f"{LABEL}*"):
    if path.is_file():
        observed.add(path)
summaries = {engine: {field: dict(median=statistics.median(values), minimum=min(values), maximum=max(values), samples=values) for field in FIELDS for values in [[float(r[field]) for r in rows if r["engine"] == engine]]} for engine in ENGINES}
pairs = []
for engine in ENGINES:
    rr = [r for r in rows if r["engine"] == engine]
    for first in [0, 2, 4]:
        pairs.append(dict(engine=engine, earlier_repeat=first, later_repeat=first + 1, relation="same executable, same engine; later/earlier adjacent-repeat diagnostic", percent_changes={field:100 * (float(rr[first + 1][field]) / float(rr[first][field]) - 1) for field in FIELDS}))
result = dict(status="Completed A/A control; host stability and candidate socket performance remain unqualified", cohort=LABEL, engine_observations=18, measured_checked_responses=sum(x["measurement"]["checked_responses"] for x in validation), warmup_checked_responses=sum(x["warmup"]["checked_responses"] for x in validation), environment=env, comparator_versions_from_frozen_lock=versions, executable_sha256=BASELINE, baseline_manifest=manifest, order=[dict(repeat=n, engine=e) for n, e in expected_order], validation=validation, rows=rows, summaries=summaries, adjacent_repeat_pairs=pairs, root_tool_observed_run=dict(exit=0, exec_session=58883, no_owned_builds_or_other_owned_loads_during_campaign=True), certificate_scope="One certificate generated for this run.py invocation and mounted for all eighteen observations; identity captured once in its environment, not independently for every engine.", limits=["All engines use the same frozen comparison executable with distinct engine dispatch arguments; adjacent repeat differences are A/A variation, not candidate gains.", "Eighteen observations come from one shared three-CPU Linux ARM VM invocation; container inventory is not proof of activity or causal interference.", "No 45-second CPU interval occurs in this cohort, but wide observed performance ranges do not establish stability.", "No causal claim about cache cleanup, no attribution of earlier adverse results, and no pooling with earlier campaigns.", "Server before/after CPU snapshots are not retained. CSV arithmetic is checked, not independently reconstructed from raw server counters.", "Environment image/certificate IDs are captured once per invocation, not separately per engine container.", "source_dirty records the working checkout; executable identity is separately fixed by captured binary hash and completed frozen build log.", "Root reports zero exit and no owned competing builds/loads through the command tool; this report does not independently audit unrelated host activity."], artifact_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(observed)}, analysis_script=Path(__file__).read_text())
(OUT / "post-cache-aa-h2-evidence.json").write_text(json.dumps(result, indent=2) + "\n")
for filename, data in [("post-cache-aa-h2-trials.csv", rows), ("post-cache-aa-h2-pairs.csv", [dict(engine=p["engine"], earlier_repeat=p["earlier_repeat"], later_repeat=p["later_repeat"], **p["percent_changes"]) for p in pairs])]:
    with (OUT / filename).open("w") as output:
        writer = csv.DictWriter(output, fieldnames=list(data[0]))
        writer.writeheader()
        writer.writerows(data)
print(json.dumps({"measured_checked_responses":result["measured_checked_responses"], "warmup_checked_responses":result["warmup_checked_responses"], "pairs":pairs}, indent=2))
