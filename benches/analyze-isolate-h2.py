#!/usr/bin/env python3
"""Reconcile the separate 42-observation H2 attribution diagnostic without load.

Requires --identity JSON with cohort, an environment reference, and frozen files:
  {"cohort":"isolate-h2", "reference_environment":{"path":...,"sha256":...},
   "plan":{"path":...,"sha256":...}, "runner":{"path":...,"sha256":...},
   "workload":{"path":...,"sha256":...}, "builds":{
     "baseline":{"binary_path":...,"binary_sha256":...,
       "source_root":...,"manifest":{"path":...,"sha256":...},
       "build_log":{"path":...,"sha256":...}},
     "error-only":{...}, "h1-only":{...}}}
Paths are relative to the repository root. The reference is an explicitly frozen
run.py environment, not a reference inferred from the observations under review.
Outcomes identify label, round, group, variant, passed and exit. Labels have the
form <cohort>-<round>-<group>-<variant>. No files are written without --write.
"""
import argparse
import csv
import hashlib
import json
import math
import re
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GROUPS = ("aa", "error-only", "h1-only")
VARIANTS = ("baseline", "candidate")
ROUNDS = 7
BASELINE = "be05e0aea314cfdda839441c4c3c923e4cae9c8100f4836f382d7ca1e703e8f2"
SETTINGS = dict(engines="maximus", paths="/echo", rates="0", body_bytes=131072,
                workers=1, duration="5s", repeats=1, vus=64, client_cpus="1,2",
                sample_seconds=0, protocol="h2", generator="k6", reuse_certificate=True)
FROZEN_ENV = ("source_commit", "source_dirty", "network", "docker", "kernel",
              "cpus", "images", "image_ids", "workload_sha256", "certificate_sha256")
CSV_SETTINGS = dict(body_bytes="131072", engine="maximus", path="/echo", workers="1",
                    protocol="h2", generator="k6", offered_rps="0", repeat="0")
METRICS = ("requests", "iterations_started", "iterations_completed", "rps", "p50_ms",
           "p99_ms", "p999_ms", "failures", "checks_failed", "network_errors", "dropped",
           "cpu_us_per_request", "server_cpu_percent", "client_cpu_percent",
           "cpu_interval_seconds", "rss_kib", "peak_rss_kib", "exit")
ERRORS = ("failures", "checks_failed", "network_errors", "dropped", "exit")
RAW_COLUMNS = {"rps": ("http_reqs", "rate"), "p50_ms": ("http_req_duration", "med"),
               "p99_ms": ("http_req_duration", "p(99)"),
               "p999_ms": ("http_req_duration", "p(99.9)")}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def kernel_identity(value):
    """Omit only uname -a's container nodename; preserve the rest byte for byte."""
    require(isinstance(value, str), "kernel identity must be a uname -a string")
    match = re.fullmatch(
        r"(Linux) ([A-Za-z0-9_.-]+) "
        r"([0-9]+\.[0-9]+\.[0-9]+\S* #[0-9]+(?: [^\r\n]+)? [A-Za-z0-9_.-]+ GNU/Linux)",
        value,
    )
    require(match is not None, f"unrecognized Linux uname -a structure: {value!r}")
    return f"{match[1]} {match[3]}"


def number(value, context, positive=False, integer=False):
    require(not isinstance(value, bool) and isinstance(value, (int, float)),
            f"{context}: expected a number")
    require(math.isfinite(value) and (value > 0 if positive else value >= 0),
            f"{context}: invalid numeric value {value!r}")
    require(not integer or value == int(value), f"{context}: expected an integer")
    return value


def metric(summary, name, field):
    return summary["metrics"][name]["values"][field]


def close(actual, expected, context):
    require(math.isclose(actual, expected, rel_tol=1e-9, abs_tol=1e-8),
            f"{context}: CSV/raw value mismatch {actual!r} != {expected!r}")


def distribution(values):
    return dict(samples=values, median=statistics.median(values), minimum=min(values),
                maximum=max(values), mean=statistics.mean(values),
                sample_stdev=statistics.stdev(values) if len(values) > 1 else None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--identity", type=Path, required=True)
    parser.add_argument("--raw", type=Path, default=ROOT / "artifacts/bench")
    parser.add_argument("--outcomes", type=Path)
    parser.add_argument("--output", type=Path, default=ROOT / "benches/results/isolate-h2")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    observed = {}

    def read(path):
        path = path.resolve()
        data = path.read_bytes()
        observed[str(path.relative_to(ROOT))] = hashlib.sha256(data).hexdigest()
        return data

    def load(path):
        return json.loads(read(path))

    def frozen(record):
        require(re.fullmatch(r"[0-9a-f]{64}", record["sha256"]) is not None,
                "frozen file requires an exact SHA-256")
        path = ROOT / record["path"]
        data = read(path)
        require(hashlib.sha256(data).hexdigest() == record["sha256"],
                f"frozen file changed: {record['path']}")
        return data

    identity = load(args.identity)
    cohort = identity["cohort"]
    require(isinstance(cohort, str) and re.fullmatch(r"[a-z0-9][a-z0-9-]*", cohort),
            "invalid cohort label")
    reference = json.loads(frozen(identity["reference_environment"]))
    reference_kernel = kernel_identity(reference["kernel"])
    for key in ("plan", "runner", "workload"):
        frozen(identity[key])
    require(reference["workload_sha256"] == identity["workload"]["sha256"],
            "reference workload differs from frozen workload")
    require(set(reference["image_ids"]) == {"grafana/k6:1.6.0", "rust:1.96-bookworm"},
            "reference must identify k6 and Rust images")
    require(all(re.fullmatch(r"sha256:[0-9a-f]{64}", value)
                for value in reference["image_ids"].values()), "invalid reference image ID")
    require(re.fullmatch(r"[0-9a-f]{64}", reference["certificate_sha256"]) is not None,
            "reference must identify the TLS certificate")
    require(reference["network"] == {"server": "none", "client": "container:<server>"},
            "wrong reference network topology")
    builds = identity["builds"]
    require(set(builds) == {"baseline", "error-only", "h1-only"}, "wrong build identities")
    require(builds["baseline"]["binary_sha256"] == BASELINE, "wrong original baseline")
    manifests = {}
    for name, build in builds.items():
        require(re.fullmatch(r"[0-9a-f]{64}", build["binary_sha256"]) is not None,
                f"{name}: invalid binary hash")
        manifests[name] = json.loads(frozen(build["manifest"]))
        for filename, digest in manifests[name].items():
            require(re.fullmatch(r"[0-9a-f]{64}", digest) is not None,
                    f"{name}: invalid source digest")
            frozen(dict(path=str(Path(build["source_root"]) / filename), sha256=digest))
        text = frozen(build["build_log"]).decode()
        require("Finished `release` profile" in text, f"{name}: incomplete build log")
        hashes = re.findall(r"^([0-9a-f]{64})\s+\S*/maximus-comparison\s*$", text, re.MULTILINE)
        require(hashes == [build["binary_sha256"]], f"{name}: build/binary hash mismatch")
    for name, changed in (("error-only", "crates/core/src/lib.rs"),
                          ("h1-only", "crates/http1/src/lib.rs")):
        differences = {path for path in manifests["baseline"].keys() | manifests[name].keys()
                       if manifests["baseline"].get(path) != manifests[name].get(path)}
        require(differences == {changed}, f"{name}: source isolation violated: {differences}")

    expected = []
    for round_number in range(ROUNDS):
        offset = round_number % len(GROUPS)
        groups = GROUPS[offset:] + GROUPS[:offset]
        variants = VARIANTS[::-1] if round_number % 2 else VARIANTS
        expected.extend((round_number, group, variant) for group in groups for variant in variants)
    outcomes_path = args.outcomes or args.raw / f"{cohort}-outcomes.json"
    outcomes = load(outcomes_path)
    require(len(outcomes) == 42, "expected exactly 42 retained outcomes")
    expected_labels = [f"{cohort}-{r}-{g}-{v}" for r, g, v in expected]
    require([item["label"] for item in outcomes] == expected_labels,
            "outcomes are incomplete, duplicated, or out of the planned execution order")
    rows, environments, summaries = [], [], []
    for item, (round_number, group, variant) in zip(outcomes, expected):
        label = item["label"]
        require((item["round"], item["group"], item["variant"]) == (round_number, group, variant),
                f"{label}: outcome identity mismatch")
        require(item["passed"] is True and item["exit"] == 0, f"{label}: failed target retained")
        build = builds["baseline" if variant == "baseline" or group == "aa" else group]
        env = load(args.raw / f"{label}-environment.json")
        environments.append(dict(trial_label=label, kernel_identity=kernel_identity(env["kernel"]), **env))
        wanted = dict(SETTINGS, label=label, binary=build["binary_path"])
        require(env["args"] == wanted, f"{label}: load, placement, or executable setting changed")
        require(env["binary_sha256"].split() == [build["binary_sha256"], build["binary_path"]],
                f"{label}: wrong executable identity")
        for key in FROZEN_ENV:
            actual = kernel_identity(env[key]) if key == "kernel" else env[key]
            wanted = reference_kernel if key == "kernel" else reference[key]
            require(actual == wanted, f"{label}: frozen environment changed: {key}")
        records = list(csv.DictReader(read(args.raw / f"{label}.csv").decode().splitlines()))
        require(len(records) == 1, f"{label}: expected exactly one Maximus observation")
        row = records[0]
        require(set(row) == set(CSV_SETTINGS) | set(METRICS), f"{label}: unexpected CSV schema")
        require(all(row[key] == value for key, value in CSV_SETTINGS.items()),
                f"{label}: CSV workload mismatch")
        numeric = {key: number(float(row[key]), f"{label}/{key}") for key in METRICS}
        require(all(numeric[key] == 0 for key in ERRORS), f"{label}: errors or unsuccessful exit")
        require(numeric["peak_rss_kib"] >= numeric["rss_kib"] > 0, f"{label}: invalid RSS")
        for key in ("requests", "rps", "cpu_us_per_request", "cpu_interval_seconds"):
            number(numeric[key], f"{label}/{key}", positive=True)
        stem = f"{label}-maximus-echo-w1-r0-n0"
        raw = {}
        for stage in ("warmup", "measured"):
            suffix = "-warmup" if stage == "warmup" else ""
            summary = load(args.raw / f"{stem}{suffix}.json")
            raw[stage] = summary
            count = number(metric(summary, "http_reqs", "count"), f"{stem}/{stage}/requests",
                           positive=True, integer=True)
            require(metric(summary, "iteration_starts", "count") == count
                    == metric(summary, "iterations", "count"), f"{stem}/{stage}: incomplete iterations")
            require(metric(summary, "checks", "passes") == count
                    and metric(summary, "checks", "fails") == 0, f"{stem}/{stage}: response checks")
            require(metric(summary, "http_req_failed", "passes") == 0,
                    f"{stem}/{stage}: failed responses")
            dropped = summary["metrics"].get("dropped_iterations", {}).get("values", {}).get("count", 0)
            require(dropped == 0, f"{stem}/{stage}: dropped arrivals")
            for name, observation in summary["metrics"].items():
                for field, value in observation["values"].items():
                    number(value, f"{stem}/{stage}/{name}/{field}")
            if stage == "measured":
                require(numeric["requests"] == numeric["iterations_started"]
                        == numeric["iterations_completed"] == count, f"{stem}: CSV iteration/count mismatch")
                for column, (name, field) in RAW_COLUMNS.items():
                    close(numeric[column], metric(summary, name, field), f"{stem}/{column}")
        cpu_text = read(args.raw / f"{stem}-client-cpu.txt").decode()
        cpu = dict(line.split() for line in cpu_text.splitlines())
        require(all(re.fullmatch(r"\d+", value) for value in cpu.values()), f"{stem}: invalid CPU counters")
        close(numeric["client_cpu_percent"],
              100 * int(cpu["usage_usec"]) / (numeric["cpu_interval_seconds"] * 1_000_000),
              f"{stem}/client CPU")
        close(numeric["server_cpu_percent"],
              100 * numeric["cpu_us_per_request"] * numeric["requests"]
              / (numeric["cpu_interval_seconds"] * 1_000_000), f"{stem}/server CPU arithmetic")
        read(args.raw / f"{stem}.log")
        rows.append(dict(trial_label=label, round=round_number, group=group, variant=variant, **row))
        summaries.append(dict(trial_label=label, stages=raw, client_cpu=cpu))

    by_key = {(row["round"], row["group"], row["variant"]): row for row in rows}
    pairs, statistics_by_group = [], []
    for group in GROUPS:
        changes = {key: [] for key in METRICS}
        for round_number in range(ROUNDS):
            baseline = by_key[(round_number, group, "baseline")]
            candidate = by_key[(round_number, group, "candidate")]
            values = {}
            for key in METRICS:
                before, after = float(baseline[key]), float(candidate[key])
                change = 100 * (after / before - 1) if before else None
                values[key] = dict(baseline=before, candidate=after, absolute_change=after - before,
                                   percent_change=change)
                if change is not None:
                    changes[key].append(change)
            pairs.append(dict(group=group, round=round_number, baseline=baseline["trial_label"],
                              candidate=candidate["trial_label"], metrics=values))
        statistics_by_group.append(dict(group=group, metrics={
            key: dict(baseline=distribution([float(by_key[(r, group, "baseline")][key]) for r in range(ROUNDS)]),
                      candidate=distribution([float(by_key[(r, group, "candidate")][key]) for r in range(ROUNDS)]),
                      paired_percent_changes=distribution(changes[key]) if changes[key] else None)
            for key in METRICS}))
    read(Path(__file__))
    report = dict(cohort=cohort, status="reconciled attribution diagnostic; performance acceptance unresolved",
                  observations=len(rows), matched_pairs=len(pairs), rounds=ROUNDS,
                  checked_responses=sum(int(float(row["requests"])) for row in rows),
                  pooling="Maximus-only five-second diagnostic; never pool with primary comparisons or other cohorts",
                  limitations=["Seven pairs describe observed variability and cannot establish universal non-regression.",
                               "AA uses the exact same executable twice and is retained as its own variance control.",
                               "Do not subtract AA changes from another group's outcomes or treat AA as an extra candidate.",
                               "Closed-loop k6 may be client-limited; this does not establish server capacity.",
                               "CPU intervals include client startup and teardown. Server CPU raw snapshots were not retained; only CSV arithmetic is reconciled.",
                               "RSS is an endpoint observation, not retained heap or a continuous peak.",
                               "Source manifests and build logs document provenance; hashes alone cannot prove a binary was produced from that source."],
                  kernel_identity=dict(method="Linux uname -a with only its second nodename field omitted; all remaining characters preserved",
                                       reference_raw=reference["kernel"], reference=reference_kernel),
                  identity=identity, outcomes=outcomes, environments=environments, raw_summaries=summaries,
                  summaries=statistics_by_group, pairs=pairs, artifact_sha256=observed)
    print(json.dumps({key: report[key] for key in ("status", "observations", "matched_pairs", "checked_responses")}, indent=2))
    if args.write:
        destinations = [args.output.with_name(args.output.name + suffix)
                        for suffix in ("-evidence.json", "-trials.csv", "-pairs.csv")]
        require(not any(path.exists() for path in destinations), "refusing to overwrite an existing report")
        destinations[0].parent.mkdir(parents=True, exist_ok=True)
        destinations[0].write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
        with destinations[1].open("x", newline="") as stream:
            writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
            writer.writeheader()
            writer.writerows(rows)
        with destinations[2].open("x", newline="") as stream:
            writer = csv.DictWriter(stream, fieldnames=("group", "round", "baseline", "candidate", "metric",
                                                      "baseline_value", "candidate_value", "absolute_change", "percent_change"))
            writer.writeheader()
            for pair in pairs:
                for name, values in pair["metrics"].items():
                    writer.writerow(dict(group=pair["group"], round=pair["round"], baseline=pair["baseline"],
                                         candidate=pair["candidate"], metric=name, baseline_value=values["baseline"],
                                         candidate_value=values["candidate"], absolute_change=values["absolute_change"],
                                         percent_change=values["percent_change"]))


if __name__ == "__main__":
    main()
