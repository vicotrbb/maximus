#!/usr/bin/env python3
"""Summarize every component observation without dropping adverse cases."""

import argparse
import csv
import json
import pathlib
import statistics


def records(root, pattern):
    prefix = pattern.removesuffix("*.csv")
    return [
        (p.name, list(csv.DictReader(p.open())))
        for p in sorted(root.glob(pattern))
        if p.name[len(prefix) : -4].isdigit()
    ]


def grouped(files, keys):
    result = {}
    for name, rows in files:
        repeat = int(name.rsplit("-", 1)[1].split(".")[0])
        for row in rows:
            key = "|".join(row[k] for k in keys)
            result.setdefault(key, {})[repeat] = float(row["ns_per_operation"])
    return result


def comparison(baseline, candidate):
    if baseline.keys() != candidate.keys():
        raise ValueError("mismatched benchmark cases")
    result = {}
    for key in baseline:
        before, after = baseline[key], candidate[key]
        if before.keys() != after.keys() or len(before) != 9:
            raise ValueError(f"incomplete repeats for {key}")
        ratios = [after[i] / before[i] for i in sorted(before)]
        result[key] = {
            "baseline_ns": list(before.values()),
            "candidate_ns": list(after.values()),
            "baseline_median": statistics.median(before.values()),
            "candidate_median": statistics.median(after.values()),
            "paired_percent": [100 * (r - 1) for r in ratios],
            "median_paired_percent": 100 * (statistics.median(ratios) - 1),
            "slower_repeats": sum(r > 1 for r in ratios),
        }
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    root = args.input
    hpack = grouped(records(root, "hpack-baseline-*.csv"), ["case"])
    events = grouped(records(root, "events-baseline-*.csv"), ["engine", "case"])
    result = {
        "hpack": {
            v: comparison(hpack, grouped(records(root, f"hpack-{v}-*.csv"), ["case"]))
            for v in ["hpack", "hpack-first"]
        },
        "events": {
            v: comparison(
                events, grouped(records(root, f"events-{v}-*.csv"), ["engine", "case"])
            )
            for v in ["sse-fields", "sse-split"]
        },
        "status": grouped(records(root, "status-*.csv"), ["variant", "status"]),
        "allocations": {
            p.name: rows
            for p, rows in [
                (path, list(csv.DictReader(path.open())))
                for path in sorted(root.glob("allocations-*.csv"))
            ]
        },
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    for variant, cases in result["hpack"].items():
        print(
            variant,
            "exact median %",
            statistics.median(
                v["median_paired_percent"]
                for k, v in cases.items()
                if k.startswith("exact")
            ),
            "literal median %",
            statistics.median(
                v["median_paired_percent"]
                for k, v in cases.items()
                if k.startswith("literal")
            ),
        )
        print(
            "adverse >5%, >=7 slower repeats:",
            [
                k
                for k, v in cases.items()
                if v["median_paired_percent"] > 5 and v["slower_repeats"] >= 7
            ],
        )
    for variant, cases in result["events"].items():
        for key, row in cases.items():
            print(
                variant,
                key,
                round(row["median_paired_percent"], 2),
                row["slower_repeats"],
            )


if __name__ == "__main__":
    main()
