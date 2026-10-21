#!/usr/bin/env python3
"""Preserve the aborted borrowed-timer cohort. Offline; never launches workloads."""

import collections
import csv
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "artifacts/performance-2026-10-20"
RAW = ROOT / "artifacts/bench"
OUT = ROOT / "benches/results"
observed = set()


def read_json(path):
    observed.add(path)
    return json.loads(path.read_text())


def require(condition, message):
    if not condition:
        raise ValueError(message)


snapshot = read_json(ART / "borrowed-timer-aborted.json")
outcomes = read_json(RAW / "borrowed-timer-wrk-outcomes.json")
require(outcomes == snapshot["completed_outcomes"], "outcome snapshot differs")
require(len(outcomes) == 9, "expected nine completed invocation outcomes")
require(all(x["passed"] and x["exit"] == 0 for x in outcomes), "completed outcome failed")
require(not list(RAW.glob("borrowed-timer-tls*")), "unexpected TLS artifacts")
completed = {x["label"] for x in outcomes}
rows = []
for path in sorted(RAW.glob("borrowed-timer-wrk-*.csv")):
    observed.add(path)
    for row in csv.DictReader(path.open()):
        require(all(float(row[k]) == 0 for k in ["failures", "checks_failed", "network_errors", "dropped", "exit"]), "failed CSV observation")
        stem = f"{path.stem}-{row['engine']}-{row['path'].strip('/').replace('/', '-')}-w1-r0-n0"
        summary = read_json(RAW / f"{stem}.json")
        metrics = summary["metrics"]
        count = metrics["http_reqs"]["values"]["count"]
        require(count == int(row["requests"]) == metrics["checks"]["values"]["passes"], "response count mismatch")
        require(metrics["checks"]["values"]["fails"] == metrics["http_req_failed"]["values"]["passes"] == metrics["network_errors"]["values"]["count"] == 0, "raw validation failure")
        require(metrics["status_error_responses"]["values"]["count"] == (count if row["path"] == "/error" else 0), "status count mismatch")
        for column, key, field in [("rps", "http_reqs", "rate"), ("p50_ms", "http_req_duration", "med"), ("p99_ms", "http_req_duration", "p(99)"), ("p999_ms", "http_req_duration", "p(99.9)")]:
            require(math.isclose(float(row[column]), metrics[key]["values"][field], rel_tol=1e-9, abs_tol=1e-8), "CSV/JSON metric mismatch")
        rows.append(dict(file=path.name, **row))
require(rows == snapshot["rows"], "preserved snapshot differs from current raw rows")
require(len(rows) == 19, "expected nineteen engine observations including partial invocation")
for path in RAW.glob("borrowed-timer-*"):
    if path.is_file():
        observed.add(path)
for filename in ["borrowed-timer-wrk-campaign.log", "borrowed-timer-build.log", "core-h2-baseline-build.log", "absolute-timer-borrowed-manifest.json", "core-h2-baseline-manifest.json"]:
    observed.add(ART / filename)
groups = collections.defaultdict(list)
for row in rows:
    groups[row["engine"]].append(row)
fields = ["rps", "cpu_us_per_request", "p50_ms", "p99_ms", "p999_ms", "cpu_interval_seconds", "rss_kib", "peak_rss_kib"]


def ranges(records):
    return {field: dict(minimum=min(float(r[field]) for r in records), maximum=max(float(r[field]) for r in records)) for field in fields}


pairs = []
for base in rows:
    if not base["file"].endswith("-baseline.csv"):
        continue
    candidate_file = base["file"].replace("-baseline.csv", "-candidate.csv")
    matches = [r for r in rows if r["file"] == candidate_file and r["engine"] == base["engine"]]
    if matches:
        candidate = matches[0]
        pairs.append(dict(baseline_file=base["file"], candidate_file=candidate_file, engine=base["engine"], path=base["path"], both_invocations_completed=candidate_file.removesuffix(".csv") in completed, percent_changes={k: 100 * (float(candidate[k]) / float(base[k]) - 1) for k in fields}))
result = dict(
    status="ABORTED / SOCKET PERFORMANCE UNQUALIFIED",
    reason="Campaign interrupted after severe VM control-plane stalls; not source acceptance or favorable-result selection.",
    planned=dict(engine_observations=84, invocation_outcomes=42, repeats=3, duration_seconds=5, tls_engine_observations=24),
    observed=dict(engine_observations=len(rows), completed_invocation_outcomes=len(outcomes), rows_from_completed_invocations=sum(r["file"].removesuffix(".csv") in completed for r in rows), rows_from_partial_invocations=sum(r["file"].removesuffix(".csv") not in completed for r in rows), recorded_checked_responses=sum(int(r["requests"]) for r in rows), tls_launched=False, tls_engine_observations=0),
    partial_invocations=[dict(label=label, recorded_engines=[r["engine"] for r in rows if r["file"] == label + ".csv"], missing_engine_rows=["axum"], outcome_recorded=False) for label in sorted({r["file"].removesuffix(".csv") for r in rows} - completed)],
    termination_reported_by_root=dict(signal="SIGINT", parent_exit=130, orphan_cleanup="Sole owned maximus-bench-55518-2 server removed after verifying its command was Axum from the frozen borrowed candidate."),
    source_review_reported_by_root="Independent agent static source review found no actionable findings. This is limited to source correctness and does not qualify measured performance.",
    limits=["No full 84-observation analyzer execution or qualification.", "CPU snapshot interval is not the configured client duration; exact stalled interval is retained.", "Raw server CPU snapshots were not retained, so CSV CPU arithmetic is not independently reconstructed here.", "All existing rows, including adverse and partial-invocation rows, remain present; no pooling with earlier campaigns.", "Observed stalls do not establish the cause of earlier adverse measurements or source-level performance effects.", "Disk/cache/resource pressure remains under investigation, not a proven cause."],
    completed_outcomes=outcomes, rows=rows, paired_observations=pairs,
    ranges_all_observations=ranges(rows), ranges_by_engine={k:ranges(v) for k,v in groups.items()},
    artifact_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(observed)},
    analysis_script=Path(__file__).read_text(),
)
(OUT / "borrowed-timer-aborted.json").write_text(json.dumps(result, indent=2) + "\n")
with (OUT / "borrowed-timer-aborted-trials.csv").open("w") as output:
    writer = csv.DictWriter(output, fieldnames=list(rows[0]))
    writer.writeheader()
    writer.writerows(rows)
print(json.dumps({"observed":result["observed"], "ranges":result["ranges_all_observations"], "pairs":pairs}, indent=2))
