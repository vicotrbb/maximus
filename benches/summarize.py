#!/usr/bin/env python3
"""Produce compact, auditable benchmark tables from the completed qualification campaign."""

import csv, json, pathlib, statistics

ROOT = pathlib.Path(__file__).resolve().parents[1]
RAW, OUT = ROOT / "artifacts/bench", ROOT / "benches/results"
CASES = {
    "workloads": 69,
    "arrival": 15,
    "scaling": 15,
    "tls-h1": 18,
    "tls-h2": 18,
    "sustained": 1,
    "capacity": 15,
    "arrival-provisioned": 15,
}
FIELDS = [
    "rps",
    "p50_ms",
    "p99_ms",
    "p999_ms",
    "cpu_us_per_request",
    "server_cpu_percent",
    "client_cpu_percent",
    "rss_kib",
    "peak_rss_kib",
]


def main():
    OUT.mkdir(exist_ok=True)
    all_rows, tables = [], []
    for case, expected in CASES.items():
        rows = list(csv.DictReader((RAW / f"qualified-{case}.csv").open()))
        assert len(rows) == expected, f"{case}: {len(rows)}/{expected} trials complete"
        for row in rows:
            row["campaign"] = case
            all_rows.append(row)
        dimensions = ("engine", "path", "workers", "protocol", "offered_rps")
        keys = sorted({tuple(r[key] for key in dimensions) for r in rows})
        table = [
            f"## {case}",
            "",
            "Performance values are medians of independent trials; error, check, drop and failed-trial columns are totals. Tails are medians of per-trial percentiles, not pooled percentiles. Max observed RSS is the median of each trial's largest observation; see the environment for the sampling interval.",
            "",
            "| Engine | Path | Workers | Protocol | Offered RPS | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Approx. server CPU % | Approx. client CPU % | Post-run RSS MiB | Max observed RSS MiB | HTTP failures | Failed checks | Dropped | Failed trials |",
            "| --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
        ]
        for scenario in keys:
            engine, path, workers, protocol, rate = scenario
            group = [r for r in rows if tuple(r[key] for key in dimensions) == scenario]
            med = {
                key: statistics.median(float(r[key]) for r in group) for key in FIELDS
            }
            failures = sum(int(r["exit"]) != 0 for r in group)
            counts = {
                key: sum(int(r[key]) for r in group)
                for key in ("failures", "checks_failed", "dropped")
            }
            table.append(
                f'| {engine} | {path} | {workers} | {protocol} | {rate} | {med["rps"]:,.0f} | {med["p50_ms"]:.3f} | {med["p99_ms"]:.3f} | {med["p999_ms"]:.3f} | {med["cpu_us_per_request"]:.2f} | {med["server_cpu_percent"]:.1f} | {med["client_cpu_percent"]:.1f} | {med["rss_kib"]/1024:.2f} | {med["peak_rss_kib"]/1024:.2f} | {counts["failures"]} | {counts["checks_failed"]} | {counts["dropped"]} | {failures} |'
            )
        tables.extend(table + [""])
    with (OUT / "trials.csv").open("w") as file:
        writer = csv.DictWriter(file, fieldnames=list(all_rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(all_rows)
    (OUT / "tables.md").write_text("# Measured results\n\n" + "\n".join(tables))
    environments = {
        case: json.loads((RAW / f"qualified-{case}-environment.json").read_text())
        for case in CASES
    }
    for environment in environments.values():
        environment["background_container_count"] = len(
            environment.pop("running_containers").splitlines()
        )
    (OUT / "environment.json").write_text(json.dumps(environments, indent=2) + "\n")
    errors = sum(int(r["failures"]) for r in all_rows)
    checks_failed = sum(int(r["checks_failed"]) for r in all_rows)
    dropped = sum(int(r["dropped"]) for r in all_rows)
    print(
        f"{len(all_rows)} trials; HTTP failures={errors}; failed checks={checks_failed}; dropped iterations={dropped}"
    )


if __name__ == "__main__":
    main()
