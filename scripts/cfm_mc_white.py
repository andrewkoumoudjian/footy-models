#!/usr/bin/env python3
"""CFM scout Monte Carlo WHITE-theme fan charts (STEERING_CFM.md section 5).

Reads the Rust footy-mc outputs for BOTH runs (artifacts/cfm_mc,
artifacts/cfm_mc_spike — deterministic, seed 42, 1,000 paths, 36 monthly
steps, sigma_ann = 0.4688) and renders, for each run:
  * chart_overview.png           (20-player equal-weight basket fan)
  * chart_player_01..20.png      (individual fans)
White background; paths faint blue, mean bold red #c62828, median dashed
#57606a, 10-90% band #d6e6fb, subtitle BELOW the x-axis (same style as
scripts/mc_36m_white.py).
"""
import numpy as np
import pandas as pd
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import os

RUNS = {
    "base": "/home/drewclaw/footy-models/artifacts/cfm_mc",
    "spike": "/home/drewclaw/footy-models/artifacts/cfm_mc_spike",
}
N_PATHS = 1000
SEED = 42
SIGMA_ANN = 0.4688

W_BG = "#ffffff"; W_TXT = "#1f2328"; W_MUT = "#57606a"; W_GRID = "#d0d7de"
PATH_C = "#9cc3f5"; MEAN_C = "#c62828"; MED_C = "#57606a"; BAND_C = "#d6e6fb"
plt.rcParams.update({
    "figure.facecolor": W_BG, "axes.facecolor": W_BG, "savefig.facecolor": W_BG,
    "savefig.edgecolor": W_BG, "text.color": W_TXT, "axes.labelcolor": W_TXT,
    "axes.edgecolor": "#1f2328", "xtick.color": W_MUT, "ytick.color": W_MUT,
    "grid.color": W_GRID, "font.size": 10, "axes.titlesize": 13,
    "axes.titleweight": "bold",
})

def fmt_eur(v):
    if v >= 1e6: return f"EUR {v/1e6:.2f}M"
    if v >= 1e3: return f"EUR {v/1e3:.0f}k"
    return f"EUR {v:.0f}"

def fan(ax, mat, ydiv, title, sub, ylabel, n_paths=200):
    months = np.arange(mat.shape[1])
    sample = mat[:n_paths]
    for row in sample:
        ax.plot(months, row / ydiv, color=PATH_C, alpha=0.5, lw=0.4, zorder=1)
    mean = mat.mean(axis=0) / ydiv
    med = np.percentile(mat, 50, axis=0) / ydiv
    p10 = np.percentile(mat, 10, axis=0) / ydiv
    p90 = np.percentile(mat, 90, axis=0) / ydiv
    ax.fill_between(months, p10, p90, color=BAND_C, alpha=0.85, zorder=2, label="10-90% band")
    ax.plot(months, mean, color=MEAN_C, lw=2.6, zorder=3, label="Mean path")
    ax.plot(months, med, color=MED_C, lw=1.5, ls="--", zorder=4, label="Median path")
    ax.set_title(title, loc="left", color=W_TXT)
    ax.set_xlabel("Months from now"); ax.set_ylabel(ylabel)
    ax.grid(True, alpha=0.7, lw=0.7)
    ax.legend(loc="upper left", fontsize=8, frameon=False, labelcolor=W_TXT)
    ax.set_xlim(0, mat.shape[1] - 1)
    # subtitle BELOW the x-axis (hard requirement)
    ax.text(0.0, -0.17, sub, transform=ax.transAxes, fontsize=7.6, color=W_MUT, va="top")

for run, base in RUNS.items():
    os.makedirs(f"{base}/charts", exist_ok=True)
    paths = pd.read_csv(f"{base}/paths_long.tsv", sep="\t")
    basket = pd.read_csv(f"{base}/paths_basket.tsv", sep="\t")
    stats = pd.read_csv(f"{base}/stats.tsv", sep="\t")
    for c in ["mean_36m", "median_36m", "p10", "p90"]:
        stats[c] = stats[c] / 100.0  # cents -> EUR
    stats["p3x"] = np.nan  # filled from paths below
    chart_files = []

    # ---- overview: basket index ----
    b = basket.pivot_table(index="path", columns="month", values="index_cents").values.astype(float)
    term = b[:, -1]
    fig, ax = plt.subplots(figsize=(11, 6.4), dpi=150)
    sub = (f"20-player equal-weight basket of the CFM top-20 | {N_PATHS:,} paths, seed {SEED}, "
           f"annual vol {SIGMA_ANN} | mean 36m index {term.mean():.0f}, median {np.median(term):.0f}, "
           f"10-90% [{np.percentile(term,10):.0f}, {np.percentile(term,90):.0f}] | "
           f"P(>=1.5x) {(term>=150).mean()*100:.0f}%, P(>=2x) {(term>=200).mean()*100:.0f}%")
    fan(ax, b, 1.0, f"CFM Top-20 Equal-Weight Basket - Monte Carlo ({run} run, index = 100)",
        sub, "Index (start = 100)", n_paths=400)
    ax.axhline(100, color=W_MUT, lw=1, ls=":", alpha=0.7)
    fig.tight_layout(rect=(0, 0.04, 1, 1))
    f = f"{base}/charts/chart_overview.png"
    fig.savefig(f, bbox_inches="tight", pad_inches=0.35); plt.close(fig); chart_files.append(f)

    # ---- individual fans (rank order as in input = stats order) ----
    for rank, meta in enumerate(stats.itertuples(), 1):
        sub_p = paths[paths.player_id == meta.player_id]
        mat = sub_p.pivot_table(index="path", columns="month", values="price_cents").values / 100.0
        term_v = mat[:, -1]
        cur = meta.current_mv
        p3x = (term_v >= 3.0 * cur).mean()
        stats.loc[stats.player_id == meta.player_id, "p3x"] = p3x
        fc12 = cur * np.exp(meta.mu_ann)
        sub = (f"#{rank} | {meta.club} | {meta.pos} | age {meta.age:.1f} | CA {meta.ca:.0f}/PA {meta.pa:.0f} | "
               f"now {fmt_eur(cur)} -> 12m drift {fmt_eur(fc12)} | "
               f"MC 36m mean {fmt_eur(meta.mean_36m)}, median {fmt_eur(meta.median_36m)}, "
               f"10-90% [{fmt_eur(meta.p10)}, {fmt_eur(meta.p90)}] | "
               f"P(>=1.5x) {meta.p15x*100:.0f}%, P(>=2x) {meta.p2x*100:.0f}%, P(>=3x) {p3x*100:.0f}%")
        fig, ax = plt.subplots(figsize=(10.5, 5.8), dpi=150)
        fan(ax, mat, 1.0, f"{rank}. {meta.name}", sub, "Market value", n_paths=300)
        fig.tight_layout(rect=(0, 0.05, 1, 1))
        f = f"{base}/charts/chart_player_{rank:02d}.png"
        fig.savefig(f, bbox_inches="tight", pad_inches=0.35); plt.close(fig); chart_files.append(f)

    stats.to_csv(f"{base}/stats.tsv", sep="\t", index=False)
    print(f"[{run}] charts written: {len(chart_files)} (overview + {len(chart_files)-1} players)")
