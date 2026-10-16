#!/usr/bin/env python3
"""Plot qualified hardware throughput. Optional dependency: matplotlib."""

import argparse
import csv
import pathlib
import statistics
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import FuncFormatter

ROOT = pathlib.Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument(
    "--native", action="store_true",
    help="plot native throughput and p99 with repeat ranges",
)
args = parser.parse_args()
name = "native-http-trials.csv" if args.native else "hardware-trials.csv"
rows = [
    row
    for row in csv.DictReader((ROOT / "benches/results" / name).open())
    if row["campaign"] == "wrk"
]
assert len(rows) == (54 if args.native else 45)
fig, axes = plt.subplots(
    1, 2 if args.native else 1,
    figsize=(12, 5.5) if args.native else (9, 5.5),
    layout="constrained", squeeze=False,
)
ax = axes[0, 0]
styles = [
    ("maximus", "Maximus (io_uring)", "#155eaa", "-"),
    ("epoll", "Maximus (epoll)", "#6889ab", "--"),
    ("hyper", "Hyper", "#218a68", "-"),
    ("axum", "Axum", "#b86a18", "-"),
    ("actix", "Actix Web", "#8158a8", "-"),
]
if args.native:
    styles = [
        ("native", "Maximus native", "#155eaa", "-"),
        ("baseline", "Maximus previous", "#66727e", "--"),
        ("baseline-wouldblock", "Previous + WouldBlock patch", "#8b715c", ":"),
        *styles[2:],
    ]
charts = [(ax, "rps")]
if args.native:
    charts.append((axes[0, 1], "p99_ms"))
for engine, label, color, style in styles:
    for chart, metric in charts:
        samples = [
            [float(r[metric]) for r in rows
             if r["engine"] == engine and int(r["workers"]) == w]
            for w in [1, 2, 4]
        ]
        assert all(len(group) == 3 for group in samples)
        medians = [statistics.median(group) for group in samples]
        errors = [
            [m - min(g) for m, g in zip(medians, samples)],
            [max(g) - m for m, g in zip(medians, samples)],
        ]
        chart.errorbar(
            [1, 2, 4], medians, yerr=errors, color=color,
            linestyle=style, marker="o", capsize=4,
            linewidth=2.5 if engine in ("maximus", "native") else 1.7,
            label=label,
        )
ax.set(
    xlabel="Server workers / permitted physical cores",
    ylabel="Validated requests per second",
    xticks=[1, 2, 4],
    ylim=(0, 450000),
)
ax.yaxis.set_major_formatter(FuncFormatter(lambda value, _: f"{value / 1000:.0f}k"))
ax.grid(axis="y", alpha=0.2)
ax.spines[["top", "right"]].set_visible(False)
ax.legend(loc="upper left", frameon=False)
ax.set_title(
    "Plaintext throughput on shared Ryzen 7 5825U\nwrk 4.1.0 • 128 connections • median and range of 3 × 10-second trials",
    loc="left",
    fontsize=12,
    pad=16,
)
fig.supxlabel(
    "Disjoint physical client/server cores; same-host loopback. Observed rates are not universal capacity.",
    fontsize=9,
)
if args.native:
    tail = axes[0, 1]
    tail.set(
        xlabel="Server workers / permitted physical cores",
        ylabel="p99 response latency (ms)", xticks=[1, 2, 4], ylim=(0, None),
    )
    tail.set_title(
        "Saturated p99: lower is better\nDifferent completed rates; not matched offered load",
        loc="left", fontsize=12, pad=16,
    )
    tail.grid(axis="y", alpha=0.2)
    tail.spines[["top", "right"]].set_visible(False)
    ax.set_title(
        "Plaintext throughput: higher is better\nShared Ryzen 7 5825U · native product 8a8fa9b",
        loc="left", fontsize=12, pad=16,
    )
    ax.legend(loc="upper left", frameon=False, fontsize=8)
    fig.supxlabel(
        "wrk 4.1.0 · 128 connections · medians and min–max ranges of 3 × 10-second trials"
        " · disjoint physical client/server cores", fontsize=9,
    )
out = ROOT / "benches/results"
out.mkdir(exist_ok=True)
stem = "native-scaling" if args.native else "scaling"
svg = out / f"{stem}.svg"
fig.savefig(svg)
svg.write_text("\n".join(line.rstrip() for line in svg.read_text().splitlines()) + "\n")
(ROOT / "artifacts").mkdir(exist_ok=True)
fig.savefig(ROOT / "artifacts" / f"{stem}.png", dpi=150)
