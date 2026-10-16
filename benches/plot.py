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
    for row in csv.DictReader((ROOT / "benches/results/hardware-trials.csv").open())
    if row["campaign"] == "wrk"
]
assert len(rows) == 45
fig, ax = plt.subplots(figsize=(9, 5.5), layout="constrained")
styles = [
    ("maximus", "Maximus (io_uring)", "#155eaa", "-"),
    ("epoll", "Maximus (epoll)", "#6889ab", "--"),
    ("hyper", "Hyper", "#218a68", "-"),
    ("axum", "Axum", "#b86a18", "-"),
    ("actix", "Actix Web", "#8158a8", "-"),
]
for engine, label, color, style in styles:
    samples = [
        [
            float(r["rps"])
            for r in rows
            if r["engine"] == engine and int(r["workers"]) == w
        ]
        for w in [1, 2, 4]
    ]
    assert all(len(group) == 3 for group in samples)
    medians = [statistics.median(group) for group in samples]
    errors = [
        [m - min(g) for m, g in zip(medians, samples)],
        [max(g) - m for m, g in zip(medians, samples)],
    ]
    ax.errorbar(
        [1, 2, 4],
        medians,
        yerr=errors,
        color=color,
        linestyle=style,
        marker="o",
        capsize=4,
        linewidth=2.5 if engine == "maximus" else 1.7,
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
out = ROOT / "benches/results"
out.mkdir(exist_ok=True)
svg = out / "scaling.svg"
fig.savefig(svg)
svg.write_text("\n".join(line.rstrip() for line in svg.read_text().splitlines()) + "\n")
(ROOT / "artifacts").mkdir(exist_ok=True)
fig.savefig(ROOT / "artifacts/scaling.png", dpi=150)
