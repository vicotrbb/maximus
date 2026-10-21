#!/usr/bin/env python3
"""Response-checked rotating source comparisons; preserve every trial and failure."""

import argparse
import csv
import json
import pathlib
import subprocess
import sys

from workloads import DEFAULT_CASES, SOCKET_CASES

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--baseline", required=True, help="frozen /target/... directory")
    p.add_argument("--candidate", required=True, help="frozen /target/... directory")
    p.add_argument("--label", required=True)
    p.add_argument("--repeats", type=int, default=3)
    p.add_argument("--duration", default="8s")
    p.add_argument("--cases", default=DEFAULT_CASES, help="case indices to run")
    p.add_argument(
        "--no-peers", action="store_true", help="Maximus-only control trials"
    )
    args = p.parse_args()
    try:
        selected = {int(value) for value in args.cases.split(",")}
    except ValueError:
        p.error("cases must be comma-separated integer indices")
    if not selected or not selected.issubset(range(len(SOCKET_CASES))):
        p.error(f"cases must be between 0 and {len(SOCKET_CASES) - 1}")
    if args.repeats < 1:
        p.error("repeats must be positive")
    out = ROOT / "artifacts/bench"
    out.mkdir(parents=True, exist_ok=True)
    results = []
    certificate = False
    for repeat in range(args.repeats):
        for case, (protocol, paths, size, generator, binary) in enumerate(SOCKET_CASES):
            if case not in selected:
                continue
            variants = [("baseline", args.baseline), ("candidate", args.candidate)]
            if repeat % 2:
                variants.reverse()
            for variant, directory in variants:
                label = f"{args.label}-{repeat}-{case}-{variant}"
                if list(out.glob(label + "*")):
                    raise SystemExit("refusing to overwrite " + label)
                engines = ["maximus"]
                if variant == "candidate" and not args.no_peers:
                    engines += ["hyper", "axum"]
                    offset = repeat % 3
                    engines = engines[offset:] + engines[:offset]
                command = [
                    sys.executable,
                    str(ROOT / "benches/run.py"),
                    "--engines",
                    ",".join(engines),
                    "--paths",
                    paths,
                    "--protocol",
                    protocol,
                    "--generator",
                    generator,
                    "--body-bytes",
                    str(size),
                    "--duration",
                    args.duration,
                    "--repeats",
                    "1",
                    "--client-cpus",
                    "1,2",
                    "--binary",
                    directory + "/" + binary,
                    "--label",
                    label,
                ]
                if protocol != "plain" and certificate:
                    command += ["--reuse-certificate"]
                with (out / (label + "-driver.log")).open("w") as f:
                    result = subprocess.run(
                        command, check=False, stdout=f, stderr=subprocess.STDOUT
                    )
                certificate = certificate or (
                    protocol != "plain" and (out / "cert.pem").exists()
                )
                entry = {
                    "repeat": repeat,
                    "case": case,
                    "variant": variant,
                    "label": label,
                    "command": command,
                    "exit": result.returncode,
                    "rows": [],
                }
                path = out / (label + ".csv")
                if path.exists():
                    entry["rows"] = list(csv.DictReader(path.open()))
                entry["passed"] = (
                    result.returncode == 0
                    and len(entry["rows"]) == len(engines) * len(paths.split(","))
                    and all(
                        float(row[key]) == 0
                        for row in entry["rows"]
                        for key in (
                            "failures",
                            "checks_failed",
                            "network_errors",
                            "dropped",
                            "exit",
                        )
                    )
                )
                results.append(entry)
                (out / (args.label + "-campaign.json")).write_text(
                    json.dumps(results, indent=2)
                )
                print(
                    label,
                    "exit",
                    result.returncode,
                    "rows",
                    len(entry["rows"]),
                    flush=True,
                )
    if not all(x["passed"] for x in results):
        raise SystemExit("campaign contains failed trials; inspect all results")


if __name__ == "__main__":
    main()
