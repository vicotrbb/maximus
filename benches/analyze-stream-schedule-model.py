#!/usr/bin/env python3
"""Offline reconciliation of the rejected standard-library traversal model."""

import csv
import hashlib
import json
import math
import re
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "artifacts/performance-2026-10-20"
OUT = ROOT / "benches/results"
CASES = [(0, 1), (1, 1), (16, 1), (16, 16), (256, 1), (256, 256)]
observed = set()


def read(name):
    path = ART / name
    observed.add(path)
    return path.read_text()


def require(condition, message):
    if not condition:
        raise ValueError(message)


source = read("stream-schedule-probe.rs")
require(hashlib.sha256(source.encode()).hexdigest() == "a60fe474a98a13e197051006c4d989b0cbea195666a9964cabce6c659b7db6fd", "model source differs from reviewed artifact")
build = read("stream-schedule-probe-build.log")
stderr = read("stream-schedule-probe.stderr")
binary_hash = "01769a13e115047d7bd56807d0cc464c38cde587b2951dd010157462a2e0a8b8"
require(re.findall(r"^([0-9a-f]{64})\s+\S*/stream-schedule-probe$", build, re.MULTILINE) == [binary_hash], "unexpected model executable hash")
require("release: 1.96.1" in build and "host: aarch64-unknown-linux-gnu" in build, "unexpected compiler/target")
require(stderr == "", "unexpected stderr")
require("let operations = (1_000_000 / count.min(budget).max(1)).max(2000);" in source and "elapsed.as_nanos() as f64 / operations as f64" in source, "model denominator form differs")
require("for _ in 0..1000" in source and "(operations + 1000) as u64 * count.min(budget) as u64" in source, "model warmup/visit invariant differs")
records = list(csv.DictReader(read("stream-schedule-probe.csv").splitlines()))
expected = [(round_number, count, budget, mode) for round_number in range(7) for count, budget in CASES for mode in (["snapshot", "live"] if round_number % 2 == 0 else ["live", "snapshot"])]
require(len(records) == 84 and [(int(r["round"]), int(r["streams"]), int(r["budget"]), r["mode"]) for r in records] == expected, "model rows/order differ")
for row in records:
    streams, budget = int(row["streams"]), int(row["budget"])
    require(int(row["operations"]) == max(1_000_000 // max(min(streams, budget), 1), 2000), "operation denominator differs")
    require(math.isfinite(float(row["ns_per_operation"])) and float(row["ns_per_operation"]) > 0, "invalid timing")
summaries, pairs = [], []
for streams, budget in CASES:
    by_mode = {mode:[float(r["ns_per_operation"]) for r in records if int(r["streams"]) == streams and int(r["budget"]) == budget and r["mode"] == mode] for mode in ["snapshot", "live"]}
    pp = [dict(streams=streams, budget=budget, round=i, snapshot_ns=by_mode["snapshot"][i], live_ns=by_mode["live"][i], percent_change=100 * (by_mode["live"][i] / by_mode["snapshot"][i] - 1)) for i in range(7)]
    pairs.extend(pp)
    summaries.append(dict(streams=streams, budget=budget, operations_per_mode_per_round=max(1_000_000 // max(min(streams, budget), 1), 2000), mode_statistics={mode:dict(median=statistics.median(v), minimum=min(v), maximum=max(v), samples=v) for mode, v in by_mode.items()}, pairs=pp))
result = dict(status="REJECTED standalone traversal model; product implementation untouched", scope="Standard-library BTreeMap stream-ID traversal model, not Connection::poll or an HTTP engine benchmark", executable_sha256=binary_hash, source_sha256=hashlib.sha256(source.encode()).hexdigest(), dependencies="Rust standard library only; no Cargo dependency or product change", compiler_metadata=build, embedded_source=source, rows=records, summaries=summaries, pairs=pairs, timed_model_poll_operations=sum(int(r["operations"]) for r in records), warmup_model_poll_operations=len(records) * 1000, model_equivalence_controls=dict(counts=[0,1,3,16,256], cursors="every integer from zero through 2*count+2 inclusive", budgets="1 and max(count,1)", checks="one-step returned sums, full maps, and final cursors equal; measured state sum matches expected visited entries after warmup+timing", cases=1134), root_tool_observed_run=dict(exit=0, exec_session=1033), inference="Repeated range_mut initialization/search plausibly contributes to the 256-stream full-budget slowdown; no profile or isolated cost measurement establishes its cause.", limits=["No product edits or actual Connection::poll test; independent model equivalence does not establish engine scheduling correctness.", "No HTTP throughput, socket latency, allocation-counter, retained-memory or RSS claim.", "ns_per_operation is per model poll; stream visit counts differ by budget and are not the printed denominator.", "One standalone process, six fixed cases and seven alternating pairs; all slower samples remain present.", "Raw elapsed timestamps, exact rustc invocation/optimization flags, CPU affinity and immutable execution image are not independently recorded in these files.", "Source/executable hashes record supplied provenance but alone cannot prove the executable was compiled from this exact source."], artifact_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(observed)}, analysis_script=Path(__file__).read_text())
(OUT / "stream-schedule-model-evidence.json").write_text(json.dumps(result, indent=2) + "\n")
for filename, data in [("stream-schedule-model-trials.csv", records), ("stream-schedule-model-pairs.csv", pairs)]:
    with (OUT / filename).open("w") as output:
        writer = csv.DictWriter(output, fieldnames=list(data[0]))
        writer.writeheader()
        writer.writerows(data)
print(json.dumps({"source_sha256":result["source_sha256"], "timed_model_poll_operations":result["timed_model_poll_operations"], "summaries":summaries}, indent=2))
