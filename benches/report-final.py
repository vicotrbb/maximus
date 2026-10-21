#!/usr/bin/env python3
"""Summarize paired trials without normalizing away peer or adverse observations."""

import argparse
import json
import pathlib
import statistics

from workloads import DEFAULT_CASES, SOCKET_CASES, response_keys


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--campaign", type=pathlib.Path, required=True)
    p.add_argument("--output", type=pathlib.Path, required=True)
    p.add_argument("--expected-repeats", type=int, default=3)
    p.add_argument("--expected-cases", default=DEFAULT_CASES)
    p.add_argument(
        "--allow-incomplete",
        action="store_true",
        help="explicitly unqualified live inspection",
    )
    args = p.parse_args()
    if args.expected_repeats < 1:
        p.error("expected-repeats must be positive")
    try:
        expected_cases = {int(value) for value in args.expected_cases.split(",")}
    except ValueError:
        p.error("expected-cases must contain integer indices")
    if not expected_cases or not expected_cases.issubset(range(len(SOCKET_CASES))):
        p.error(f"expected-cases must be between 0 and {len(SOCKET_CASES) - 1}")
    trials = json.loads(args.campaign.read_text())
    expected = {
        (repeat, case, variant)
        for repeat in range(args.expected_repeats)
        for case in expected_cases
        for variant in ("baseline", "candidate")
    }
    observed = [(trial["repeat"], trial["case"], trial["variant"]) for trial in trials]
    failed = [
        trial["label"]
        for trial in trials
        if trial.get("passed") is not True
        or trial["exit"] != 0
        or not any(row["engine"] == "maximus" for row in trial["rows"])
    ]
    diagnostics = {
        "missing": sorted(expected - set(observed)),
        "unexpected": sorted(set(observed) - expected),
        "duplicate_trials": len(observed) != len(set(observed)),
        "failed_trials": failed,
    }
    response_errors = []
    for trial in trials:
        try:
            command = trial["command"]
            engines = command[command.index("--engines") + 1].split(",")
            required = response_keys(trial["case"], engines)
        except (KeyError, ValueError, IndexError):
            response_errors.append(
                {"label": trial["label"], "invalid_case_command": True}
            )
            continue
        actual = [
            (row["protocol"], row["path"], int(row["body_bytes"]), row["engine"])
            for row in trial["rows"]
        ]
        missing = required - set(actual)
        extra = set(actual) - required
        if missing or extra or len(actual) != len(set(actual)):
            response_errors.append(
                {
                    "label": trial["label"],
                    "missing": sorted(missing),
                    "unexpected": sorted(extra),
                    "duplicates": len(actual) != len(set(actual)),
                }
            )
    diagnostics["response_contract"] = response_errors
    grouped = {}
    pairs = {}
    duplicate_rows = []
    for trial in trials:
        for row in trial["rows"]:
            key = "|".join(
                str(row[x]) for x in ["protocol", "path", "body_bytes", "engine"]
            )
            grouped.setdefault(key, {}).setdefault(trial["variant"], []).append(row)
            if row["engine"] == "maximus":
                pair = pairs.setdefault(key, {}).setdefault(trial["repeat"], {})
                if trial["variant"] in pair:
                    duplicate_rows.append((key, trial["repeat"], trial["variant"]))
                pair[trial["variant"]] = row
    diagnostics["duplicate_rows"] = duplicate_rows
    diagnostics["unpaired_rows"] = [
        key
        for key, repeats in pairs.items()
        if set(repeats) != set(range(args.expected_repeats))
        or any(set(pair) != {"baseline", "candidate"} for pair in repeats.values())
    ]
    qualified_input = not any(diagnostics.values())
    result = {
        "qualified_input": qualified_input,
        "diagnostics": diagnostics,
        "trials": trials,
        "summary": {},
        "paired": {},
    }
    if not qualified_input and not args.allow_incomplete:
        args.output.write_text(json.dumps(result, indent=2) + "\n")
        raise SystemExit(
            "campaign is incomplete or failed; raw trials and diagnostics preserved, qualification statistics withheld"
        )
    metrics = [
        "rps",
        "p50_ms",
        "p99_ms",
        "p999_ms",
        "cpu_us_per_request",
        "rss_kib",
        "peak_rss_kib",
        "client_cpu_percent",
        "server_cpu_percent",
    ]
    for key, variants in grouped.items():
        result["summary"][key] = {
            v: {
                metric: statistics.median(float(row[metric]) for row in rows)
                for metric in metrics
            }
            for v, rows in variants.items()
        }
    for key, repeats in pairs.items():
        valid = [r for r in repeats.values() if set(r) == {"baseline", "candidate"}]
        comparison = {}
        for metric in metrics:
            changes = [
                100 * (float(r["candidate"][metric]) / float(r["baseline"][metric]) - 1)
                for r in valid
                if float(r["baseline"][metric]) != 0
            ]
            comparison[metric] = {
                "paired_percent": changes,
                "median_paired_percent": statistics.median(changes)
                if changes
                else None,
            }
        result["paired"][key] = comparison
    args.output.write_text(json.dumps(result, indent=2) + "\n")

    def rounded(value):
        return round(value, 2) if value is not None else None

    for key, row in result["paired"].items():
        print(
            key,
            "rps",
            rounded(row["rps"]["median_paired_percent"]),
            "p99",
            rounded(row["p99_ms"]["median_paired_percent"]),
            "cpu",
            rounded(row["cpu_us_per_request"]["median_paired_percent"]),
        )


if __name__ == "__main__":
    main()
