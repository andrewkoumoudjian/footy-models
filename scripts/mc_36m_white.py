#!/usr/bin/env python3
"""36-month Monte Carlo WHITE-theme charts + report (STEERING_36M.md).

Reads the Rust footy-mc outputs (artifacts/mc_36m_white/{paths_long.tsv,
paths_basket.tsv, stats.tsv}) — deterministic, seed 42, 1,000 paths, 36
monthly steps, drift = model 12m log forecast annualized, vol = 0.4688
(v2 conformal radius 0.6009 / 1.2816) — and renders:
  * artifacts/mc_36m_white/chart_overview.png   (top-25 basket fan)
  * artifacts/mc_36m_white/chart_player_01..10.png (top-10 individual fans)
  * data/forecast_36m_report.md                 (full report)
All charts on a WHITE background per the steering hard requirement.
"""
import numpy as np
import pandas as pd
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import os, html

BASE = "/home/drewclaw/footy-models/artifacts/mc_36m_white"
REPO = "/home/drewclaw/footy-models"
TOP_N_IND = 10
BASKET_N = 25
N_PATHS = 1000
SEED = 42
SIGMA_ANN = 0.4688
MODEL_HASH = "1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f"

os.makedirs(BASE, exist_ok=True)
paths = pd.read_csv(f"{BASE}/paths_long.tsv", sep="\t")
basket = pd.read_csv(f"{BASE}/paths_basket.tsv", sep="\t")
stats = pd.read_csv(f"{BASE}/stats.tsv", sep="\t")
for c in ["mean_36m", "median_36m", "p10", "p90"]:
    stats[c] = stats[c] / 100.0  # cents -> EUR

# ---- white theme ----
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
    if v >= 1e6: return f"€{v/1e6:.2f}M"
    if v >= 1e3: return f"€{v/1e3:.0f}k"
    return f"€{v:.0f}"

def fan(ax, mat, ydiv, title, sub, ylabel, yfmt=fmt_eur, n_paths=200, show_legend=True):
    """mat: (n_paths, n_months) in EUR (or index for basket)."""
    months = np.arange(mat.shape[1])
    sample = mat[:n_paths]  # draw a representative subset of faint paths
    for row in sample:
        ax.plot(months, row / ydiv, color=PATH_C, alpha=0.5, lw=0.4, zorder=1)
    mean = mat.mean(axis=0) / ydiv
    med = np.percentile(mat, 50, axis=0) / ydiv
    p10 = np.percentile(mat, 10, axis=0) / ydiv
    p90 = np.percentile(mat, 90, axis=0) / ydiv
    ax.fill_between(months, p10, p90, color=BAND_C, alpha=0.85, zorder=2, label="10–90% band")
    ax.plot(months, mean, color=MEAN_C, lw=2.6, zorder=3, label="Mean path")
    ax.plot(months, med, color=MED_C, lw=1.5, ls="--", zorder=4, label="Median path")
    ax.set_title(title, loc="left", color=W_TXT)
    ax.set_xlabel("Months from now"); ax.set_ylabel(ylabel)
    ax.grid(True, alpha=0.7, lw=0.7)
    if show_legend:
        ax.legend(loc="upper left", fontsize=8, frameon=False, labelcolor=W_TXT)
    ax.set_xlim(0, mat.shape[1] - 1)
    # subtitle BELOW the x-axis (hard requirement)
    ax.text(0.0, -0.17, sub, transform=ax.transAxes, fontsize=7.6, color=W_MUT, va="top")

chart_files = []

# ---- overview: basket (index; start = 100) ----
b = basket.pivot_table(index="path", columns="month", values="index_cents").values  # index (start = 100)
b_idx = b.astype(float)
term = b_idx[:, -1]
fig, ax = plt.subplots(figsize=(11, 6.4), dpi=150)
sub = (f"{BASKET_N}-player equal-weight basket of the top-{BASKET_N} by expected 36m % gain | "
       f"{N_PATHS:,} paths, seed {SEED}, annual vol {SIGMA_ANN} | "
       f"mean 36m index {term.mean():.0f}, median {np.median(term):.0f}, 10–90% "
       f"[{np.percentile(term,10):.0f}, {np.percentile(term,90):.0f}] | "
       f"P(≥1.5×) {(term>=150).mean()*100:.0f}%, P(≥2×) {(term>=200).mean()*100:.0f}%")
fan(ax, b_idx, 1.0, f"Equal-Weight Basket of Top {BASKET_N} 36-Month Forecast Players — Monte Carlo (index = 100)",
    sub, "Index (start = 100)", n_paths=400)
ax.axhline(100, color=W_MUT, lw=1, ls=":", alpha=0.7)
fig.tight_layout(rect=(0, 0.04, 1, 1))
f = f"{BASE}/chart_overview.png"; fig.savefig(f, bbox_inches="tight", pad_inches=0.35); plt.close(fig); chart_files.append(f)

# ---- top-N individual fan charts ----
for rank, meta in enumerate(stats.head(TOP_N_IND).itertuples(), 1):
    sub_p = paths[paths.player_id == meta.player_id]
    mat = sub_p.pivot_table(index="path", columns="month", values="price_cents").values / 100.0  # EUR
    fc12 = meta.current_mv * np.exp(meta.mu_ann)
    sub = (f"#{rank} | {meta.club} | {meta.pos} | age {meta.age:.1f} | CA {meta.ca:.0f}/PA {meta.pa:.0f} | {meta.attr_source} | "
           f"now {fmt_eur(meta.current_mv)} → 12m forecast {fmt_eur(fc12)} | "
           f"MC 36m mean {fmt_eur(meta.mean_36m)}, median {fmt_eur(meta.median_36m)}, "
           f"10–90% [{fmt_eur(meta.p10)}, {fmt_eur(meta.p90)}] | "
           f"P(≥1.5×) {meta.p15x*100:.0f}%, P(≥2×) {meta.p2x*100:.0f}%, P(transfer 3y) {meta.transfer_prob*100:.0f}%")
    fig, ax = plt.subplots(figsize=(10.5, 5.8), dpi=150)
    fan(ax, mat, 1.0, f"{rank}. {meta.name}", sub, "Market value", n_paths=300)
    fig.tight_layout(rect=(0, 0.05, 1, 1))
    f = f"{BASE}/chart_player_{rank:02d}.png"; fig.savefig(f, bbox_inches="tight", pad_inches=0.35); plt.close(fig); chart_files.append(f)

print("charts written:", len(chart_files))
for f in chart_files: print(" ", os.path.basename(f))
np.save("/tmp/basket_term.npy", term)
stats.head(TOP_N_IND).to_csv("/tmp/mc_stats_top10.tsv", sep="\t", index=False)
