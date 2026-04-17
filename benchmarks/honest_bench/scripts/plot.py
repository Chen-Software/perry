#!/usr/bin/env python3
"""Render bar charts from results/results.json into charts/*.png.

One chart per workload: median wall time with error bars (± stddev) + a
second axis for peak RSS. All axes log-scale where helpful so Perry's gap
is visible without crushing Rust / Zig.
"""
import os
import json
import statistics
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

ROOT = Path(__file__).resolve().parent.parent
RESULTS = ROOT / "results" / "results.json"
CHARTS = ROOT / "charts"
CHARTS.mkdir(exist_ok=True)

LANGUAGE_ORDER = ["rust", "zig", "perry", "node", "bun"]
COLORS = {"rust": "#CE422B", "zig": "#F7A41D", "perry": "#1F6FEB", "node": "#68A063", "bun": "#FBF0DF"}

WORKLOAD_TITLES = {
    "image_convolution":   "Image convolution (5×5 blur, 3840×2160)",
    "json_pipeline_small": "JSON pipeline — 100 records (21 KB)",
    "json_pipeline_full":  "JSON pipeline — 500k records (108 MB)",
}

def agg(rows):
    walls = [r["wall_ms"] for r in rows if r["exit_code"] == 0]
    rss   = [r["max_rss_kb"] for r in rows if r["exit_code"] == 0]
    if not walls:
        return None
    return {
        "wall_median": statistics.median(walls),
        "wall_sd":     statistics.stdev(walls) if len(walls) >= 2 else 0.0,
        "rss_median":  statistics.median(rss),
        "rss_sd":      statistics.stdev(rss) if len(rss) >= 2 else 0.0,
        "n":           len(walls),
    }

def main():
    if not RESULTS.exists():
        raise SystemExit(f"missing {RESULTS}")
    data = json.loads(RESULTS.read_text())["rows"]

    by_wl = {}
    for r in data:
        by_wl.setdefault(r["workload"], {}).setdefault(r["language"], []).append(r)

    for wl_id, by_lang in by_wl.items():
        langs = [l for l in LANGUAGE_ORDER if l in by_lang]
        stats = {l: agg(by_lang[l]) for l in langs}
        # drop missing
        langs = [l for l in langs if stats[l] is not None]
        if not langs:
            continue

        fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(11, 4.3))
        fig.suptitle(WORKLOAD_TITLES.get(wl_id, wl_id), fontsize=13, y=1.02)

        xs = np.arange(len(langs))
        walls = [stats[l]["wall_median"] for l in langs]
        wall_errs = [stats[l]["wall_sd"] for l in langs]
        colors = [COLORS[l] for l in langs]

        ax1.bar(xs, walls, yerr=wall_errs, capsize=4, color=colors, edgecolor="black")
        ax1.set_xticks(xs)
        ax1.set_xticklabels(langs)
        ax1.set_ylabel("wall time (ms, lower is better)")
        ax1.set_title("wall time — median ± σ")
        for x, w in zip(xs, walls):
            ax1.text(x, w, f"{w:.0f} ms", ha="center", va="bottom", fontsize=9)
        ax1.margins(y=0.15)

        rss_mb = [stats[l]["rss_median"] / 1024.0 for l in langs]
        rss_errs = [stats[l]["rss_sd"] / 1024.0 for l in langs]
        ax2.bar(xs, rss_mb, yerr=rss_errs, capsize=4, color=colors, edgecolor="black")
        ax2.set_xticks(xs)
        ax2.set_xticklabels(langs)
        ax2.set_ylabel("peak RSS (MB, lower is better)")
        ax2.set_title("peak memory footprint — median ± σ")
        for x, r in zip(xs, rss_mb):
            ax2.text(x, r, f"{r:.0f} MB", ha="center", va="bottom", fontsize=9)
        ax2.margins(y=0.15)

        fig.tight_layout()
        out = CHARTS / f"{wl_id}.png"
        fig.savefig(out, dpi=130, bbox_inches="tight")
        plt.close(fig)
        print(f"wrote {out}")

if __name__ == "__main__":
    main()
