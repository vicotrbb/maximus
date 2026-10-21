#!/usr/bin/env python3
"""Reconcile the clean borrowed timer mechanism probe, without generating load."""

import csv
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
PROBE = ART / "timer-probe-borrowed"
observed = set()


def read(path):
    observed.add(path)
    return path.read_text()


def require(condition, message):
    if not condition:
        raise ValueError(message)


source = read(PROBE / "src/main.rs")
require(hashlib.sha256(source.encode()).hexdigest() == "d185f0a1e05f5b107b20f7f0bb82c574c461a740e051206b62743cde0eda1d66", "probe source differs from reviewed source")
probe_lock = tomllib.loads(read(PROBE / "Cargo.lock"))
product_lock = tomllib.loads(read(ART / "absolute-timer-borrowed-source/Cargo.lock"))
read(PROBE / "Cargo.toml")
product = {(p["name"], p["version"], p.get("source"), p.get("checksum")) for p in product_lock["package"]}
extra = [p for p in probe_lock["package"] if (p["name"], p["version"], p.get("source"), p.get("checksum")) not in product]
require(len(extra) == 1 and (extra[0]["name"], extra[0]["version"]) == ("maximus-timer-probe", "0.0.0"), "probe dependency differs from product lock")
build = read(ART / "timer-probe-borrowed-build.log")
clippy = read(ART / "timer-probe-borrowed-clippy.log")
binary_hash = "44e9c32189ddc8bb21055f01936e1010fe13506b8f31d4c0240d6087e132d374"
require(re.findall(r"^([0-9a-f]{64})\s+\S*/maximus-timer-probe$", build, re.MULTILINE) == [binary_hash], "unexpected probe executable hash")
require("Finished `release` profile" in build and "Finished `dev` profile" in clippy, "incomplete build/clippy logs")
versions = {(p["name"], p["version"]) for p in probe_lock["package"]}
require(all(pair in versions for pair in re.findall(r"(?:Compiling|Checking) ([\w-]+) v([\w.]+)", build + clippy)), "compiled package version differs from probe lock")
require(read(ART / "timer-probe-borrowed-clean.stderr").strip() == "driver,IoUring", "unexpected driver or stderr")
records = list(csv.DictReader(read(ART / "timer-probe-borrowed-clean.csv").splitlines()))
require(len(records) == 14, "expected fourteen cases")
modes = ["relative_owned_fuse", "absolute_borrowed_fuse"]
pairs = []
for round_number in range(7):
    rows = records[2 * round_number:2 * round_number + 2]
    require([int(r["round"]) for r in rows] == [round_number, round_number], "unexpected round order")
    require([r["mode"] for r in rows] == (modes if round_number % 2 == 0 else list(reversed(modes))), "unexpected alternating order")
    for row in rows:
        require(int(row["operations"]) == 2_000_000, "unexpected operations")
        value = float(row["ns_per_operation"])
        require(math.isfinite(value) and value > 0, "invalid timing")
        require(int(row["future_bytes"]) == (120 if row["mode"] == modes[0] else 88), "unexpected stored future size")
    times = {r["mode"]: float(r["ns_per_operation"]) for r in rows}
    pairs.append(dict(round=round_number, relative_ns=times[modes[0]], borrowed_absolute_ns=times[modes[1]], percent_change=100 * (times[modes[1]] / times[modes[0]] - 1)))
summaries = {mode: dict(median_ns=statistics.median(float(r["ns_per_operation"]) for r in records if r["mode"] == mode), minimum_ns=min(float(r["ns_per_operation"]) for r in records if r["mode"] == mode), maximum_ns=max(float(r["ns_per_operation"]) for r in records if r["mode"] == mode), stored_f_type_bytes=120 if mode == modes[0] else 88) for mode in modes}
layouts = {}
for label, filename in [("baseline", "layout-baseline-probe.log"), ("borrowed", "absolute-timer-borrowed-footprint-layout.log")]:
    text = read(ART / filename)
    layouts[label] = {name: int(value) for name, value in re.findall(r"^(compat_serve_future_bytes|native_serve_future_bytes),(\d+)$", text, re.MULTILINE)}
require(layouts == {"baseline": {"compat_serve_future_bytes": 1576, "native_serve_future_bytes": 2984}, "borrowed": {"compat_serve_future_bytes": 1528, "native_serve_future_bytes": 2936}}, "unexpected separately measured serve layouts")
read(ART / "timer-probe-borrowed.csv")
result = dict(
    scope="Clean standalone borrowed timer mechanism probe; no HTTP socket qualification",
    executable_sha256=binary_hash, driver="IoUring", source_sha256=hashlib.sha256(source.encode()).hexdigest(),
    dependency_verification=dict(probe_lock_packages=len(probe_lock["package"]), matching_product_dependency_packages=len(probe_lock["package"]) - 1, extra_probe_package_only=True, comparison="name, version, registry source, checksum", packages=[{k:p[k] for k in ["name", "version", "source", "checksum"] if k in p} for p in probe_lock["package"]]),
    method=dict(rounds=7, operations_per_mode_per_round=2_000_000, warmup_operations_per_mode=10_000, order="relative first in even rounds; borrowed absolute first in odd rounds", context="actual Runtime::block_on poll context", operation="Construct fixed one-hour deadline timer, poll once expecting Pending, drop/cancel; fixed deadline chosen separately for each mode", controls="Expired deadline is Ready before every case; future deadline is Pending throughout warmup and timed operations", size_measurement="size_of::<F>() measures stored input F only, excluding the borrowed Fuse wrapper and enclosing async frame"),
    root_tool_observed_run=dict(exit=0, exec_session=20280, command=["docker", "run", "--rm", "--security-opt", "seccomp=unconfined", "--cpuset-cpus", "0", "-v", "maximus-bench-target:/target:ro", "rust:1.96-bookworm", "/target/oct20-timer-probe-borrowed/release/maximus-timer-probe"], stdout="artifacts/performance-2026-10-20/timer-probe-borrowed-clean.csv", stderr="artifacts/performance-2026-10-20/timer-probe-borrowed-clean.stderr", own_campaigns_builds_and_allocation_probes_stopped=True, incremental_cache_removed="13 GB logical/about 7 GB physical; host free space about 12 to 19 GB", metadata_limit="Root observed the exact command and exit through exec_command/write_stdin. No separate probe environment capture independently records actual CPU affinity, compiler version, or immutable container image identity. The tagged Rust image is specified by the command; Rust 1.96.1 is known from other campaign captures, not a fresh probe compiler capture."),
    summaries=summaries, pairs=pairs, rows=records, separately_observed_serve_future_layouts=layouts,
    excluded_exploratory_csv="timer-probe-borrowed.csv overlaps a hashing helper at startup and contributes no timings to these summaries",
    limits=["One process, one host, seven alternating rounds; no independent repeated-process or socket estimate.", "Stored F type sizes are not full async-frame or borrowed Fuse-wrapper sizes.", "Serve-future layout differences are separate target-specific observations, not derived from F size.", "Build/source/lock hashes record provenance but alone cannot prove a binary was built from a specific source.", "Cache cleanup and stopping owned work are root-reported run context, not proof of the prior stall cause.", "The HTTP campaign remains aborted and unqualified; no throughput, tail-latency, allocation or framework-leadership claim follows."],
    artifact_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(observed)},
    analysis_script=Path(__file__).read_text(),
)
(OUT / "borrowed-timer-component.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps({"summaries": summaries, "pairs": pairs, "matching_dependency_packages": result["dependency_verification"]["matching_product_dependency_packages"]}, indent=2))
