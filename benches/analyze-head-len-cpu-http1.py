#!/usr/bin/env python3
"""Offline joint wall/thread-CPU head_len H1 memory-cohort reconciliation.

Run only after all 56 processes finish and root confirms zero driver exit.
Requires confirmed zero exit, final binary/source SHA-256 arguments, and the
final driver completion marker. CPU-clock accounting is not physical cycles.
Never generates load or compiles source.
"""

import argparse
import csv
import difflib
import hashlib
import json
import math
import re
import statistics
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "artifacts/performance-2026-10-20"
OUT = ROOT / "benches/results"
ENGINES = ["maximus", "dynamic", "hyper", "axum"]
CASES = ["plain", "static-error", "owned-error"]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--confirmed-driver-exit", type=int, choices=[0], required=True)
parser.add_argument("--expected-baseline-hash", required=True)
parser.add_argument("--expected-candidate-hash", required=True)
parser.add_argument("--expected-probe-source-hash", required=True)
parser.add_argument("--execution-session", type=int)
args = parser.parse_args()
HASHES = {"baseline":args.expected_baseline_hash, "candidate":args.expected_candidate_hash}
for value in [*HASHES.values(), args.expected_probe_source_hash]:
    if re.fullmatch("[0-9a-f]{64}", value) is None:
        raise ValueError("final SHA-256 arguments must be sixty-four lowercase hexadecimal digits")
observed = set()
embedded = {}


def read(path, embed=False):
    observed.add(path)
    text = path.read_text()
    if embed:
        embedded[str(path.relative_to(ROOT))] = text
    return text


def digest(path):
    return hashlib.sha256(read(path).encode()).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


env = json.loads(read(ART / "head-len-cpu-http1-environment.json", True))
outcomes = json.loads(read(ART / "head-len-cpu-http1-outcomes.json", True))
source = read(ART / "memory-http1-cpu-probe/src/main.rs", True)
require(hashlib.sha256(source.encode()).hexdigest() == args.expected_probe_source_hash, "probe source differs from final frozen CPU fixture")
compact_source = re.sub(r"\s+", "", source)
require("rustix::time::clock_gettime(rustix::time::ClockId::ThreadCPUTime)" in compact_source and "Duration::try_from(thread_cpu()-cpu_started)" in compact_source, "expected safe thread CPU clock/subtraction form is missing")
for filename, sha in env["probe_source_sha256"].items():
    require(digest(ROOT / filename) == sha, "probe input hash differs from run environment")
    read(ROOT / filename, True)
driver_path = ART / "measure-head-len-cpu-http1.py"
require(digest(driver_path) == env["driver_sha256"], "run driver hash differs")
read(driver_path, True)
read(ART / "build-head-len-cpu-http1.sh", True)
require(env["plan"] == dict(rounds=7, connections_per_case=1000, requests_per_connection=1000, warmup_connections_per_case=10, cases=CASES, engines=ENGINES, source_order="alternates each round", engine_order="rotates each round, same order across variants"), "unexpected fixed plan")
require(env["image_id"] == "sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663", "unexpected captured Rust image")
constants = dict(re.findall(r"const (REQUESTS|CONNECTIONS|WARMUP): usize = (\d+);", source))
require(constants == {"REQUESTS":"1000", "CONNECTIONS":"1000", "WARMUP":"10"}, "source denominator differs")
denominator = int(constants["REQUESTS"]) * int(constants["CONNECTIONS"])
warmup_count = int(constants["REQUESTS"]) * int(constants["WARMUP"])

manifests = {}
for variant, details in env["builds"].items():
    log_path = ROOT / details["build_log"]
    manifest_path = ROOT / details["product_manifest"]
    require(digest(log_path) == details["build_log_sha256"], "build log hash differs")
    require(digest(manifest_path) == details["product_manifest_sha256"], "product manifest hash differs")
    require(digest(manifest_path) == {"baseline":"71c2090f3dac94ab0a6c286b23691f2f2b42986e58a45faca45769c5f0379ac9", "candidate":"ffeb7e51da92cdce6466cb087cdb06057b7f622e01b53d597baf0d991c6ae11a"}[variant], "unexpected frozen cohort manifest")
    log = read(log_path, True)
    require(re.findall(r"^([0-9a-f]{64})\s+\S*/probe$", log, re.MULTILINE) == [HASHES[variant]], "unexpected final probe build identity")
    require(log.count("Finished `release` profile") >= 2 and "release: 1.96.1" in log and "host: aarch64-unknown-linux-gnu" in log, "incomplete final release Clippy/build or unexpected compiler")
    metadata = details["metadata"]
    require(re.findall(r"^([0-9a-f]{64})\s+\S*/probe$", metadata, re.MULTILINE) == [HASHES[variant]], "execution metadata binary differs")
    require("Cpus_allowed_list:\t0" in metadata and "release: 1.96.1" in metadata, "execution metadata placement/compiler differs")
    manifests[variant] = json.loads(read(manifest_path, True))
require(set(manifests) == {"baseline", "candidate"}, "missing product manifests")
require({name for name in set(manifests["baseline"]) | set(manifests["candidate"]) if manifests["baseline"].get(name) != manifests["candidate"].get(name)} == {"crates/http1/src/lib.rs"}, "head_len is not exactly the one-file source change")
product_patches = {}
for filename in ["crates/http1/src/lib.rs"]:
    contents = {}
    for variant, frozen in [("baseline", "h1-head-baseline-source"), ("candidate", "h1-head-len-source")]:
        path = ART / frozen / filename
        contents[variant] = read(path)
        require(digest(path) == manifests[variant][filename], "frozen changed product source differs from manifest")
    product_patches[filename] = "".join(difflib.unified_diff(contents["baseline"].splitlines(keepends=True), contents["candidate"].splitlines(keepends=True), fromfile="a/" + filename, tofile="b/" + filename))
probe_lock = tomllib.loads(read(ART / "memory-http1-cpu-probe/Cargo.lock", True))
benchmark_lock_path = ART / "h1-head-baseline-source/benches/baselines/Cargo.lock"
require(digest(benchmark_lock_path) == manifests["baseline"]["benches/baselines/Cargo.lock"], "frozen comparator lock differs")
benchmark_lock = tomllib.loads(read(benchmark_lock_path, True))
packages = {(p["name"], p["version"], p.get("source"), p.get("checksum")) for p in benchmark_lock["package"]}
extra = [p for p in probe_lock["package"] if (p["name"], p["version"], p.get("source"), p.get("checksum")) not in packages]
require(len(extra) == 1 and extra[0]["name"] == "maximus-memory-http1-cpu-probe", "probe dependency differs from frozen comparator lock")
require(manifests["baseline"]["benches/baselines/Cargo.lock"] == manifests["candidate"]["benches/baselines/Cargo.lock"], "variant comparator locks differ")
footprints = {}
for variant, name in [("baseline", "h1-head-baseline"), ("candidate", "h1-head-len")]:
    text = read(ART / f"verify-size-{name}.log", True)
    require(text.count("Finished `release` profile") == 2, "incomplete footprint builds")
    footprints[variant] = {}
    for artifact in ["hello", "conformance"]:
        sha = re.findall(r"^([0-9a-f]{64})\s+\S*/" + artifact + "$", text, re.MULTILINE)
        fields = re.findall(r"^\s*(\d+)\s+(\d+)\s+(\d+)\s+\d+\s+[0-9a-f]+\s+\S*/" + artifact + "$", text, re.MULTILINE)
        sizes = re.findall(r"^\S*/" + artifact + r"(\.stripped)? (\d+)$", text, re.MULTILINE)
        require(len(sha) == len(fields) == 1 and len(sizes) == 2, "ambiguous matched footprint rows")
        footprints[variant][artifact] = dict(sha256=sha[0], gnu_text_bytes=int(fields[0][0]), data_bytes=int(fields[0][1]), bss_bytes=int(fields[0][2]), file_bytes={suffix:int(value) for suffix,value in sizes})
layouts = {}
for variant, name in [("baseline", "h1-head-baseline"), ("candidate", "h1-head-len")]:
    text = read(ART / f"head-layout-{name}.log", True)
    layouts[variant] = {key:int(value) for key,value in re.findall(r"^(compat_serve_future_bytes|native_serve_future_bytes),(\d+)$", text, re.MULTILINE)}
    require(layouts[variant] == {"compat_serve_future_bytes":4328,"native_serve_future_bytes":4352}, "unexpected H1 serve layout")
for filename in ["src/main.rs", "Cargo.toml", "Cargo.lock"]:
    read(ART / "head-layout-probe" / filename, True)
read(ART / "verify-head-len-footprints.sh", True)
controls = read(ART / "h1-head-len-native-controls.log", True)
baseline_control = read(ART / "h1-head-baseline-control.log", True)
require("19 passed; 0 failed; 0 ignored" in controls and "1 passed; 0 failed; 0 ignored" in baseline_control, "native protocol controls differ")
host_path = ART / "head-len-cpu-http1-host-observation.json"
host_context = json.loads(read(host_path, True)) if host_path.exists() else None
excluded_generated_lock = tomllib.loads(read(ART / "memory-http1-cpu-generated-unmeasured.lock", True))
excluded_libc_versions = [p["version"] for p in excluded_generated_lock["package"] if p["name"] == "libc"]
clock_resolutions = []
expected_order = [(repeat, variant, engine) for repeat in range(7) for variant in (["baseline", "candidate"] if repeat % 2 == 0 else ["candidate", "baseline"]) for engine in ENGINES[repeat % 4:] + ENGINES[:repeat % 4]]
require(len(outcomes) == 56 and [(x["repeat"], x["variant"], x["engine"]) for x in outcomes] == expected_order, "process count or fixed order differs")
rows = []
for item in outcomes:
    repeat, variant, engine = item["repeat"], item["variant"], item["engine"]
    require(item["exit"] == 0 and item["passed"] is True, "failed process retained; unqualified")
    expected_command = ["docker", "run", "--rm", "--security-opt", "seccomp=unconfined", "--cpuset-cpus", "0", "-v", "maximus-bench-target:/target:ro", "rust:1.96-bookworm", env["builds"][variant]["binary"], engine]
    require(item["command"] == expected_command, "process command differs")
    stem = f"head-len-cpu-http1-{repeat}-{variant}-{engine}"
    require(item["stdout"] == stem + ".csv" and item["stderr"] == stem + ".stderr", "outcome file identity differs")
    stderr = read(ART / item["stderr"])
    resolution = re.fullmatch(r"thread CPU clock resolution:\s*Timespec\s*\{\s*tv_sec:\s*(\d+),\s*tv_nsec:\s*(\d+)\s*\}", stderr.strip())
    require(resolution is not None, "missing clock resolution or unexpected stderr")
    seconds, nanos = map(int, resolution.groups())
    require(0 <= nanos < 1_000_000_000 and seconds * 1_000_000_000 + nanos > 0, "invalid thread CPU clock resolution")
    clock_resolutions.append(dict(repeat=repeat, variant=variant, engine=engine, seconds=seconds, nanoseconds=nanos))
    records = list(csv.DictReader(read(ART / item["stdout"]).splitlines()))
    require(len(records) == 3 and [r["case"] for r in records] == CASES, "case rows differ")
    for row in records:
        require(row["engine"] == engine and int(row["requests"]) == denominator, "engine/denominator differs")
        require(set(row) == {"engine", "case", "requests", "ns_per_request", "cpu_ns_per_request"}, "unexpected joint wall/CPU fields")
        require(all(math.isfinite(float(row[field])) and float(row[field]) > 0 for field in ["ns_per_request", "cpu_ns_per_request"]), "invalid/nonpositive wall or CPU timing")
        rows.append(dict(repeat=repeat, variant=variant, **row))
require(len(rows) == 168, "expected all168 rows")
campaign = read(ART / "head-len-cpu-http1-run.log", True)
require(campaign.splitlines() == [f"head-len-cpu-http1-{r}-{v}-{e} PASS" for r, v, e in expected_order] + ["All56processes/168observations passed, every output response validated"], "campaign log differs")
require(len({(r["seconds"], r["nanoseconds"]) for r in clock_resolutions}) == 1, "thread CPU clock resolution varies within cohort")
versions = {p["name"]:p["version"] for p in probe_lock["package"] if p["name"] in ["libc", "rustix"]}
require(versions == {"libc":"0.2.190", "rustix":"1.1.5"}, "CPU clock dependency versions drifted")
probe_manifest = tomllib.loads(read(ART / "memory-http1-cpu-probe/Cargo.toml", True))
require(probe_manifest["dependencies"]["rustix"] == {"version":"=1.1.5", "features":["time"]}, "unexpected direct clock feature dependency")
summaries, pairs = [], []
for engine in ENGINES:
    for case in CASES:
        metric_summaries = {}
        for metric in ["ns_per_request", "cpu_ns_per_request"]:
            samples = {v:[float(r[metric]) for r in rows if r["engine"] == engine and r["case"] == case and r["variant"] == v] for v in ["baseline", "candidate"]}
            changes = [100 * (samples["candidate"][i] / samples["baseline"][i] - 1) for i in range(7)]
            metric_summaries[metric] = dict(variant_statistics={v:dict(median=statistics.median(a), minimum=min(a), maximum=max(a), samples=a) for v,a in samples.items()}, paired_percent_changes=changes, faster_pairs=sum(x < 0 for x in changes), slower_pairs=sum(x > 0 for x in changes))
        for repeat in range(7):
            wall = metric_summaries["ns_per_request"]
            cpu = metric_summaries["cpu_ns_per_request"]
            pairs.append(dict(engine=engine, case=case, repeat=repeat, baseline_wall_ns=wall["variant_statistics"]["baseline"]["samples"][repeat], candidate_wall_ns=wall["variant_statistics"]["candidate"]["samples"][repeat], wall_percent_change=wall["paired_percent_changes"][repeat], baseline_cpu_ns=cpu["variant_statistics"]["baseline"]["samples"][repeat], candidate_cpu_ns=cpu["variant_statistics"]["candidate"]["samples"][repeat], cpu_percent_change=cpu["paired_percent_changes"][repeat]))
        summaries.append(dict(engine=engine, case=case, metrics=metric_summaries))
result = dict(
    status="Artifact-only head_len H1 joint wall/thread-CPU diagnostic; strict performance qualification unresolved",
    cohort="head-len-cpu-http1", matched_source_scope="Only crates/http1/src/lib.rs differs between frozen baseline and candidate",
    process_outcomes=outcomes, engine_case_observations=168, matched_engine_case_pairs=84,
    timed_checked_responses=len(rows) * denominator, warmup_checked_responses=len(rows) * warmup_count,
    environment=env, binary_sha256=HASHES, expected_final_probe_source_sha256=args.expected_probe_source_hash,
    source_manifests=manifests, changed_product_library_patches=product_patches,
    matched_footprints=footprints, h1_serve_future_layouts=layouts,
    native_controls=dict(candidate_passed=19, baseline_fragmented_pipeline_control_passed=1),
    current_cohort_host_observation=host_context, clock_resolutions=clock_resolutions,
    dependency_comparison=dict(probe_packages=len(probe_lock["package"]), matching_frozen_benchmark_packages=len(probe_lock["package"]) - 1, only_extra_package=extra[0]["name"], comparison="name/version/registry source/checksum", clock_dependency_versions=versions, probe_only_direct_rustix_time_feature=True),
    excluded_unmeasured_generated_lock=dict(filename="memory-http1-cpu-generated-unmeasured.lock", libc_versions=excluded_libc_versions, status="Unmeasured dependency drift; excluded from selected builds and all results. Final selected lock requires libc0.2.190."),
    rows=rows, summaries=summaries, pairs=pairs, embedded_artifact_text=embedded,
    root_tool_observed_run=dict(confirmed_driver_exit=args.confirmed_driver_exit, exec_session=args.execution_session),
    limits=["H1 shared futures memory transport only: no H2 or native TCP I/O, socket capacity, latency distribution or RSS claim.",
            "Linux thread CPU-clock accounting covers the executing thread, not whole-process CPU. It is not physical cycles or proof of absent guest/host contention, frequency effects, cache effects or CPU migration outside captured affinity constraints.",
            "Direct Maximus Kind handler versus direct Hyper service; dynamic Maximus versus Axum includes routing.",
            "Both wall and CPU intervals include connection/protocol work, allocation, memory-wire writes and output retention; setup and final validation/destruction are outside timing.",
            "Wall and CPU windows have slightly different clock-read boundaries. Raw clock start/end values are not independently retained; printed values use the same fixed one-million response denominator.",
            "All measured output assertions pass via recorded source and zero process outcomes; raw HTTP output buffers are not separately archived.",
            "The fixed memory wire is always ready and has no network backpressure. Runtime and service abstractions differ across engines.",
            "All slower wall/CPU samples and unchanged Hyper/Axum control variation are preserved; no comparator normalization or pooling with earlier elapsed/socket/timer cohorts.",
            "Any current-cohort host observation is context only, not causal attribution. The earlier elapsed-cohort snapshot is not presented as a current-cohort capture.",
            "GNU text reductions, data/file growth and unchanged stripped lengths remain separate; no automatic size-gate acceptance follows.",
            "Probe-only rustix time instrumentation adds no product dependency or product change. Artifact-only candidate remains unqualified unless independently established by the full acceptance evidence.",
            "Source/build/manifest hashes and embedded inputs support reproduction but alone cannot prove binary construction from a particular source."],
    artifact_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(observed)},
    analysis_script=Path(__file__).read_text(),
)
(OUT / "head-len-cpu-http1-evidence.json").write_text(json.dumps(result, indent=2) + "\n")
for filename, data in [("head-len-cpu-http1-trials.csv", rows), ("head-len-cpu-http1-pairs.csv", pairs)]:
    with (OUT / filename).open("w") as output:
        writer = csv.DictWriter(output, fieldnames=list(data[0]))
        writer.writeheader()
        writer.writerows(data)
print(json.dumps({"timed_checked_responses":result["timed_checked_responses"], "warmup_checked_responses":result["warmup_checked_responses"], "summaries":summaries}, indent=2))
