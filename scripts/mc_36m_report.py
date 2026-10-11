#!/usr/bin/env python3
"""36-month Monte Carlo value forecast report (footy-models).

For each player: monthly drift = ln(pred_12m / current) / 12 (model's
12-month forecast annualized); monthly vol derived from the player's
conformal 80% interval width (fallback: pooled OOF RMSE / sqrt(12)).
Simulates N log-normal paths for 36 months (seed 42), renders a fan chart
per player (all paths + mean highlighted + 10-90 band), an equal-weight
basket overview, stats table, and exports a dark-themed PDF.
"""
import numpy as np
import pandas as pd
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import os, sys

# ---------------- config ----------------
DATA = "/home/drewclaw/footy-models/data/forecast_u23_top100.csv"
OUT_DIR = "/home/drewclaw/footy-models/artifacts/mc_36m"
HORIZON = 36          # months
N_PATHS = 500
SEED = 42
RMSE_OOF = 0.5554     # pooled OOF RMSE on 12m log ratio
Z80 = 1.2816          # two-sided 80% normal quantile
TOP_N_IND = 10        # individual fan charts
BASKET_N = 25         # equal-weight basket size
os.makedirs(OUT_DIR, exist_ok=True)
rng = np.random.default_rng(SEED)

# ---------------- data ----------------
p = pd.read_csv(DATA).sort_values("gap_eur", ascending=False).reset_index(drop=True)
p["mu_m"] = np.log(p["predicted_12m_mv"] / p["current_mv"]) / 12.0
log_lo = np.log(p["interval_10"]); log_hi = np.log(p["interval_90"])
p["sigma_12"] = (log_hi - log_lo) / (2 * Z80)
p["sigma_m"] = (p["sigma_12"] / np.sqrt(12.0)).clip(lower=RMSE_OOF / np.sqrt(12.0) * 0.5)

# ---------------- simulation ----------------
def simulate(player):
    lv = np.full((N_PATHS, HORIZON + 1), np.log(player["current_mv"]))
    eps = rng.standard_normal((N_PATHS, HORIZON))
    for t in range(HORIZON):
        lv[:, t + 1] = lv[:, t] + player["mu_m"] + player["sigma_m"] * eps[:, t]
    return np.exp(lv)  # shape (N, 37) in EUR

# individual simulations
sims = {}
for i, row in p.head(TOP_N_IND).iterrows():
    sims[row["player_id"]] = simulate(row)
sims_arr = np.stack(list(sims.values()))          # (n_players, N, 37)

# basket (equal-weight index of top BASKET_N: each player normalized to 1.0)
basket_paths = np.zeros((N_PATHS, HORIZON + 1))
for i, row in p.head(BASKET_N).iterrows():
    s = simulate(row)
    basket_paths += s / row["current_mv"]
basket_paths /= BASKET_N
basket_mean = basket_paths.mean(axis=0) * 100.0
basket_p10 = np.percentile(basket_paths, 10, axis=0) * 100.0
basket_p90 = np.percentile(basket_paths, 90, axis=0) * 100.0

# stats table (top-25): MC 36m mean/median, P10/P90, P(>=1.5x), P(>=2x)
stats = []
for i, row in p.head(BASKET_N).iterrows():
    s = simulate(row)
    end = s[:, -1]
    stats.append({
        "name": row["name"], "club": row["club"], "pos": row["pos_cls"],
        "age": f"{row['age']:.1f}", "cur": row["current_mv"],
        "fc12": row["predicted_12m_mv"],
        "mc_mean": end.mean(), "mc_med": np.median(end),
        "p10": np.percentile(end, 10), "p90": np.percentile(end, 90),
        "p15x": (end >= 1.5 * row["current_mv"]).mean(),
        "p2x": (end >= 2.0 * row["current_mv"]).mean(),
    })
stats_df = pd.DataFrame(stats)

# ---------------- styling ----------------
BG = "#0d1117"; PANEL = "#161b22"; TXT = "#e6edf3"; MUT = "#8b949e"
PATH_C = "#1f6feb"; MEAN_C = "#00d4aa"; BAND_C = "#1f6feb"
plt.rcParams.update({
    "figure.facecolor": BG, "axes.facecolor": PANEL, "savefig.facecolor": BG,
    "text.color": TXT, "axes.labelcolor": TXT, "axes.edgecolor": "#30363d",
    "xtick.color": MUT, "ytick.color": MUT, "grid.color": "#21262d",
    "font.size": 10, "axes.titlesize": 13, "axes.titleweight": "bold",
})

def fmt_eur(v):
    if v >= 1e6: return f"€{v/1e6:.2f}M"
    if v >= 1e3: return f"€{v/1e3:.0f}k"
    return f"€{v:.0f}"

def fan_chart(ax, paths, mean, p10, p90, title, sub, yfmt=fmt_eur, show_band=True):
    t = np.arange(HORIZON + 1)
    for path in paths[:N_PATHS]:
        ax.plot(t, path, color=PATH_C, alpha=0.05, lw=0.55, zorder=1)
    if show_band:
        ax.fill_between(t, p10, p90, color=BAND_C, alpha=0.14, zorder=2, label="10–90% band")
    ax.plot(t, mean, color=MEAN_C, lw=2.6, zorder=3, label="Mean path")
    ax.set_title(title, loc="left", color=TXT)
    ax.text(0.0, 1.02, sub, transform=ax.transAxes, fontsize=8.5, color=MUT)
    ax.set_xlabel("Months from now"); ax.set_ylabel("Market value")
    ax.grid(True, alpha=0.35)
    ax.legend(loc="upper left", fontsize=8, frameon=False, labelcolor=TXT)
    ax.set_xlim(0, HORIZON)

# ---------------- overview chart ----------------
fig, ax = plt.subplots(figsize=(11, 6.2), dpi=150)
t = np.arange(HORIZON + 1)
for path in basket_paths[:N_PATHS]:
    ax.plot(t, path * 100, color=PATH_C, alpha=0.05, lw=0.55, zorder=1)
ax.fill_between(t, basket_p10, basket_p90, color=BAND_C, alpha=0.14, zorder=2, label="10–90% band")
ax.plot(t, basket_mean, color=MEAN_C, lw=2.8, zorder=3, label="Mean path")
ax.axhline(100, color=MUT, lw=1, ls="--", alpha=0.6)
ax.set_title(f"Equal-Weight Basket of Top {BASKET_N} Forecast Players — 36-Month Monte Carlo (index = 100)", loc="left")
ax.text(0.0, 1.02, f"{N_PATHS} paths, seed {SEED}, drift from model 12m forecast, vol from conformal 80% interval (fallback RMSE {RMSE_OOF})",
        transform=ax.transAxes, fontsize=8.5, color=MUT)
ax.set_xlabel("Months from now"); ax.set_ylabel("Index (start = 100)")
ax.grid(True, alpha=0.35); ax.legend(loc="upper left", fontsize=9, frameon=False, labelcolor=TXT)
ax.set_xlim(0, HORIZON)
fig.tight_layout()
fig.savefig(f"{OUT_DIR}/overview.png", bbox_inches="tight")
plt.close(fig)

# ---------------- individual charts ----------------
chart_files = []
for rank, (i, row) in enumerate(p.head(TOP_N_IND).iterrows(), 1):
    s = sims[row["player_id"]]
    mean = s.mean(axis=0); p10 = np.percentile(s, 10, axis=0); p90 = np.percentile(s, 90, axis=0)
    sub = (f"#{rank} | {row['club']} | {row['pos_cls']} | age {row['age']:.1f} | "
           f"CA {row['fm_ca']:.0f}/PA {row['fm_pa']:.0f} | {row['attr_source']} | "
           f"now {fmt_eur(row['current_mv'])} → 12m forecast {fmt_eur(row['predicted_12m_mv'])} | "
           f"MC 36m mean {fmt_eur(mean[-1])}, 10–90% [{fmt_eur(p10[-1])}, {fmt_eur(p90[-1])}]")
    fig, ax = plt.subplots(figsize=(10.5, 5.6), dpi=150)
    fan_chart(ax, s, mean, p10, p90, f"{rank}. {row['name']}", sub)
    fig.tight_layout()
    f = f"{OUT_DIR}/player_{rank:02d}.png"
    fig.savefig(f, bbox_inches="tight")
    plt.close(fig)
    chart_files.append(f)

# stats table markdown
def eur(v): return f"€{v/1e6:.2f}M" if v >= 1e6 else f"€{v/1e3:.0f}k"
rows_md = "\n".join(
    f"| {i+1} | {r['name'][:26]} | {r['club'][:20]} | {r['pos']} | {r['age']} | {eur(r['cur'])} | {eur(r['fc12'])} | {eur(r['mc_mean'])} | {eur(r['mc_med'])} | {eur(r['p10'])} | {eur(r['p90'])} | {r['p15x']*100:.0f}% | {r['p2x']*100:.0f}% |"
    for i, r in stats_df.iterrows())
with open(f"{OUT_DIR}/stats_table.md", "w") as f:
    f.write("| # | Player | Club | Pos | Age | Current | 12m Forecast | MC 36m Mean | MC 36m Med | P10 | P90 | P(≥1.5x) | P(≥2x) |\n"
            "|---|---|---|---|---|---|---|---|---|---|---|---|---|\n" + rows_md + "\n")

print(f"charts -> {OUT_DIR}/  (overview + {len(chart_files)} players)")
print(stats_df.head(5).to_string(index=False))
