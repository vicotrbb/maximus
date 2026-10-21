#!/usr/bin/env python3
"""Offline reconciliation of the fixed HTTP/1 memory-transport probe."""

import collections
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
HASHES = {"baseline":"76d71330cfd9c995607ca5237c92eb419fdee0a19f57d57314e88e0350acddb9", "candidate":"30ff4f5201cfa0b4007f30efdd9e5178d0189a071b2177cf89ec38e7b7bee02d"}
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


env = json.loads(read(ART / "memory-http1-environment.json", True))
outcomes = json.loads(read(ART / "memory-http1-outcomes.json", True))
source = read(ART / "memory-http1-probe/src/main.rs", True)
require(hashlib.sha256(source.encode()).hexdigest() == "c822c3c9549934a8ed4919b0a1c8ea650c779e2168f7fc7f19c012a923f06e21", "probe source differs from reviewed final protocol-assertion version")
for filename, sha in env["probe_source_sha256"].items():
    require(digest(ROOT / filename) == sha, "probe input hash differs from run environment")
    read(ROOT / filename, True)
driver_path = ART / "measure-memory-http1.py"
require(digest(driver_path) == env["driver_sha256"], "run driver hash differs")
read(driver_path, True)
read(ART / "build-memory-http1.sh", True)
require(env["plan"] == dict(rounds=7, connections_per_case=1000, requests_per_connection=1000, warmup_connections_per_case=10, cases=CASES, engines=ENGINES, source_order="alternates each round", engine_order="rotates each round, same order across variants"), "unexpected fixed plan")
require(env["image_id"] == "sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663", "unexpected captured Rust image")
constants = dict(re.findall(r"const (REQUESTS|CONNECTIONS|WARMUP): usize = (\d+);", source))
require(constants == {"REQUESTS":"1000", "CONNECTIONS":"1000", "WARMUP":"10"}, "source denominator differs")
denominator = int(constants["REQUESTS"]) * int(constants["CONNECTIONS"])
warmup_count = int(constants["REQUESTS"]) * int(constants["WARMUP"])
clippy = read(ART / "memory-http1-probe-clippy-final.log", True)
require("Finished `dev` profile" in clippy, "incomplete final Clippy log")
manifests = {}
for variant, details in env["builds"].items():
    log_path = ROOT / details["build_log"]
    manifest_path = ROOT / details["product_manifest"]
    require(digest(log_path) == details["build_log_sha256"], "build log hash differs")
    require(digest(manifest_path) == details["product_manifest_sha256"], "product manifest hash differs")
    log = read(log_path, True)
    require(re.findall(r"^([0-9a-f]{64})\s+\S*/probe$", log, re.MULTILINE) == [HASHES[variant]], "unexpected final probe build identity")
    require("Finished `release` profile" in log and "release: 1.96.1" in log and "host: aarch64-unknown-linux-gnu" in log, "unexpected build compiler/profile")
    metadata = details["metadata"]
    require(re.findall(r"^([0-9a-f]{64})\s+\S*/probe$", metadata, re.MULTILINE) == [HASHES[variant]], "execution metadata binary differs")
    require("Cpus_allowed_list:\t0" in metadata and "release: 1.96.1" in metadata, "execution metadata placement/compiler differs")
    manifests[variant] = json.loads(read(manifest_path, True))
require(set(manifests) == {"baseline", "candidate"}, "missing product manifests")
product_patches = {}
for filename in ["crates/core/src/lib.rs", "crates/http1/src/lib.rs", "crates/http2/src/lib.rs"]:
    contents = {}
    for variant, frozen in [("baseline", "core-h2-baseline-source"), ("candidate", "absolute-timer-borrowed-source")]:
        path = ART / frozen / filename
        contents[variant] = read(path)
        require(digest(path) == manifests[variant][filename], "frozen changed product source differs from manifest")
    product_patches[filename] = "".join(difflib.unified_diff(contents["baseline"].splitlines(keepends=True), contents["candidate"].splitlines(keepends=True), fromfile="a/" + filename, tofile="b/" + filename))
probe_lock = tomllib.loads(read(ART / "memory-http1-probe/Cargo.lock", True))
benchmark_lock_path = ART / "core-h2-baseline-source/benches/baselines/Cargo.lock"
require(digest(benchmark_lock_path) == manifests["baseline"]["benches/baselines/Cargo.lock"], "frozen comparator lock differs")
benchmark_lock = tomllib.loads(read(benchmark_lock_path, True))
packages = {(p["name"], p["version"], p.get("source"), p.get("checksum")) for p in benchmark_lock["package"]}
extra = [p for p in probe_lock["package"] if (p["name"], p["version"], p.get("source"), p.get("checksum")) not in packages]
require(len(extra) == 1 and extra[0]["name"] == "maximus-memory-http1-probe", "probe dependency differs from frozen comparator lock")
require(manifests["baseline"]["benches/baselines/Cargo.lock"] == manifests["candidate"]["benches/baselines/Cargo.lock"], "variant comparator locks differ")
expected_order = [(repeat, variant, engine) for repeat in range(7) for variant in (["baseline", "candidate"] if repeat % 2 == 0 else ["candidate", "baseline"]) for engine in ENGINES[repeat % 4:] + ENGINES[:repeat % 4]]
require(len(outcomes) == 56 and [(x["repeat"], x["variant"], x["engine"]) for x in outcomes] == expected_order, "process count or fixed order differs")
rows = []
for item in outcomes:
    repeat, variant, engine = item["repeat"], item["variant"], item["engine"]
    require(item["exit"] == 0 and item["passed"] is True, "failed process retained; unqualified")
    expected_command = ["docker", "run", "--rm", "--security-opt", "seccomp=unconfined", "--cpuset-cpus", "0", "-v", "maximus-bench-target:/target:ro", "rust:1.96-bookworm", env["builds"][variant]["binary"], engine]
    require(item["command"] == expected_command, "process command differs")
    stem = f"memory-http1-{repeat}-{variant}-{engine}"
    require(item["stdout"] == stem + ".csv" and item["stderr"] == stem + ".stderr", "outcome file identity differs")
    require(read(ART / item["stderr"]) == "", "unexpected stderr")
    records = list(csv.DictReader(read(ART / item["stdout"]).splitlines()))
    require(len(records) == 3 and [r["case"] for r in records] == CASES, "case rows differ")
    for row in records:
        require(row["engine"] == engine and int(row["requests"]) == denominator, "engine/denominator differs")
        require(math.isfinite(float(row["ns_per_request"])) and float(row["ns_per_request"]) > 0, "invalid timing")
        rows.append(dict(repeat=repeat, variant=variant, **row))
require(len(rows) == 168, "expected all168 rows")
campaign = read(ART / "memory-http1-campaign.log", True)
require(campaign.splitlines() == [f"memory-http1-{r}-{v}-{e} PASS" for r, v, e in expected_order] + ["All56processes/168observations passed, every output response validated"], "campaign log differs")
summaries = []
pairs = []
for engine in ENGINES:
    for case in CASES:
        samples = {v:[float(r["ns_per_request"]) for r in rows if r["engine"] == engine and r["case"] == case and r["variant"] == v] for v in ["baseline", "candidate"]}
        pp = [dict(engine=engine, case=case, repeat=repeat, baseline_ns=samples["baseline"][repeat], candidate_ns=samples["candidate"][repeat], percent_change=100 * (samples["candidate"][repeat] / samples["baseline"][repeat] - 1)) for repeat in range(7)]
        pairs.extend(pp)
        summaries.append(dict(engine=engine, case=case, variant_statistics={v:dict(median=statistics.median(a), minimum=min(a), maximum=max(a), samples=a) for v, a in samples.items()}, paired_percent_changes=[p["percent_change"] for p in pp], faster_pairs=sum(p["percent_change"] < 0 for p in pp), slower_pairs=sum(p["percent_change"] > 0 for p in pp)))
result = dict(status="H1 memory-transport diagnostic completed; strict performance qualification unresolved", process_outcomes=outcomes, engine_case_observations=168, matched_engine_case_pairs=84, timed_checked_responses=len(rows) * denominator, warmup_checked_responses=len(rows) * warmup_count, environment=env, binary_sha256=HASHES, source_manifests=manifests, changed_product_library_patches=product_patches, dependency_comparison=dict(probe_packages=len(probe_lock["package"]), matching_frozen_benchmark_packages=len(probe_lock["package"]) - 1, only_extra_package=extra[0]["name"], comparison="name/version/registry source/checksum"), rows=rows, summaries=summaries, pairs=pairs, embedded_artifact_text=embedded, root_tool_observed_run=dict(exit=0, exec_session=28169, no_owned_competing_load=True, final_host_clippy_exit=0), limits=["H1 shared futures memory transport only: no H2 or native TCP I/O, socket capacity, latency distribution or RSS claim.", "Direct Maximus Kind handler versus direct Hyper service; dynamic Maximus versus Axum includes routing.", "The input bytes and handlers/services are constructed before the interval. Timed work includes connection/protocol processing, allocation, memory-wire writes and output retention.", "Timed response output validation and final destruction are after the interval. Source assertions plus zero process exits establish exact protocol/status/body/header/count contracts; no separate raw HTTP output archive exists.", "ns/request is reported elapsed time divided by the fixed one-million response denominator. Raw nanosecond start/end timestamps are not retained for independent elapsed-time reconstruction.", "The fixed memory wire is always ready and has no network backpressure. Runtime implementations and service abstractions differ across engines.", "Hyper/Axum implementations and dependency versions are unchanged across variants; their substantial paired variations are retained controls, not causal normalization factors.", "One shared Linux ARM VM, seven alternating pairs per engine/case, no pooling with socket or timer cohorts; slower samples remain included.", "Source/build/manifest hashes and embedded inputs support reproduction but alone cannot prove a binary was built from particular source contents."], artifact_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(observed)}, analysis_script=Path(__file__).read_text())
(OUT / "memory-http1-evidence.json").write_text(json.dumps(result, indent=2) + "\n")
for filename, data in [("memory-http1-trials.csv", rows), ("memory-http1-pairs.csv", pairs)]:
    with (OUT / filename).open("w") as output:
        writer = csv.DictWriter(output, fieldnames=list(data[0]))
        writer.writeheader()
        writer.writerows(data)
print(json.dumps({"timed_checked_responses":result["timed_checked_responses"], "warmup_checked_responses":result["warmup_checked_responses"], "summaries":summaries}, indent=2))
