#!/usr/bin/env python3
"""Plot qualified hardware throughput. Optional dependency: matplotlib."""

import csv
import pathlib
import statistics
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import FuncFormatter

ROOT = pathlib.Path(__file__).resolve().parents[1]
rows = [
    row
    for row in csv.DictReader((ROOT / "benches/results/native-http-trials.csv").open())
    if row["campaign"] == "wrk"
]
assert len(rows) == 54
fig, axes = plt.subplots(1, 2, figsize=(12, 5.5), layout="constrained")
ax, tail = axes
styles = [
    ("native", "Maximus native", "#155eaa", "-"),
    ("baseline", "Maximus previous", "#66727e", "--"),
    ("baseline-wouldblock", "Previous + WouldBlock patch", "#8b715c", ":"),
    ("hyper", "Hyper", "#218a68", "-"),
    ("axum", "Axum", "#b86a18", "-"),
    ("actix", "Actix Web", "#8158a8", "-"),
]
charts = [(ax, "rps"), (tail, "p99_ms")]
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
            linewidth=2.5 if engine == "native" else 1.7,
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
stem = "native-scaling"
svg = out / f"{stem}.svg"
fig.savefig(svg)
svg.write_text("\n".join(line.rstrip() for line in svg.read_text().splitlines()) + "\n")
(ROOT / "artifacts").mkdir(exist_ok=True)
fig.savefig(ROOT / "artifacts" / f"{stem}.png", dpi=150)
