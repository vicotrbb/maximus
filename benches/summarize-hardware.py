#!/usr/bin/env python3
"""Summarize separate k6 and wrk Linux-hardware campaigns without pooling latency."""

import csv
import json
import pathlib
import statistics

ROOT = pathlib.Path(__file__).resolve().parents[1]
RAW = ROOT / "artifacts/homelab"
OUT = ROOT / "benches/results"
CASES = {"wrk": 45, "k6": 30, "arrival": 15, "sustained": 1}


def main():
    OUT.mkdir(exist_ok=True)
    all_rows, environments, tables = [], {}, ["# Linux hardware results", ""]
    for case, expected in CASES.items():
        rows = list(csv.DictReader((RAW / f"qualified-{case}.csv").open()))
        assert len(rows) == expected, f"{case}: {len(rows)}/{expected} trials"
        for row in rows:
            row["campaign"] = case
            all_rows.append(row)
        environment = json.loads(
            (RAW / f"qualified-{case}-environment.json").read_text()
        )
        environment["container_images"] = [
            {"name": c["name"], "image": c["image"], "imageID": c["imageID"]}
            for c in environment["container_images"]
        ]
        environments[case] = environment
        tables.extend(
            [
                f"## {case}",
                "",
                "Performance values are medians of independent trials; error/drop/failed-trial counts are totals. Percentiles are medians of per-trial percentiles. wrk uses its corrected histogram; k6 uses request-duration observations. CPU values are approximate; RSS is observed before/after each trial, not a sampled peak.",
                "",
                "| Generator | Engine | Path | Workers | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Server CPU % | Client CPU % | RSS MiB | HTTP/check/wrk-network errors | Dropped | Failed trials |",
                "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: |",
            ]
        )
        dimensions = (
            "generator",
            "engine",
            "path",
            "workers",
            "protocol",
            "offered_rps",
        )
        groups = sorted({tuple(r[k] for k in dimensions) for r in rows})
        for key in groups:
            group = [r for r in rows if tuple(r[k] for k in dimensions) == key]
            med = lambda k: statistics.median(float(r[k]) for r in group)
            total = lambda k: sum(int(r[k]) for r in group)
            failed = sum(int(r["exit"]) != 0 for r in group)
            tables.append(
                f'| {key[0]} | {key[1]} | {key[2]} | {key[3]} | {med("rps"):,.0f} | {med("p50_ms"):.3f} | {med("p99_ms"):.3f} | {med("p999_ms"):.3f} | {med("cpu_us_per_request"):.2f} | {med("server_cpu_percent"):.1f} | {med("client_cpu_percent"):.1f} | {med("rss_kib")/1024:.2f} | {total("failures")}/{total("checks_failed")}/{total("network_errors")} | {total("dropped")} | {failed} |'
            )
        tables.append("")
        if "cycles_per_request" in rows[0]:
            tables.extend(
                [
                    "Task-attached PMU counters include attributed kernel/interrupt execution; they are a different scope from cgroup CPU. See the report for the diagnostic. Each trial attaches to existing server TIDs after warmup.",
                    "",
                    "| Engine | Workers | Cycles/request | Instructions/request | Cache misses/request |",
                    "| --- | ---: | ---: | ---: | ---: |",
                ]
            )
            for key in groups:
                group = [r for r in rows if tuple(r[k] for k in dimensions) == key]
                med = lambda k: statistics.median(float(r[k]) for r in group)
                tables.append(
                    f'| {key[1]} | {key[3]} | {med("cycles_per_request"):,.0f} | {med("instructions_per_request"):,.0f} | {med("cache_misses_per_request"):,.1f} |'
                )
            tables.append("")
    fields = list(dict.fromkeys(k for row in all_rows for k in row))
    with (OUT / "hardware-trials.csv").open("w") as file:
        writer = csv.DictWriter(file, fieldnames=fields, lineterminator="\n")
        writer.writeheader()
        writer.writerows(all_rows)
    (OUT / "hardware-tables.md").write_text("\n".join(tables))
    (OUT / "hardware-environment.json").write_text(
        json.dumps(environments, indent=2) + "\n"
    )
    counts = {
        k: sum(int(r[k]) for r in all_rows)
        for k in ["requests", "failures", "checks_failed", "network_errors", "dropped"]
    }
    print(f"{len(all_rows)} trials: {counts}")


if __name__ == "__main__":
    main()
