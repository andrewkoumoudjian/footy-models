#!/usr/bin/env python3
"""Render the 36-month Monte Carlo PDF report from the Rust footy-mc outputs.

Reads artifacts/mc_36m/{paths_long.tsv, paths_basket.tsv, stats.tsv}
(produced by apps/mc using crates/forecast primitives) and emits a
dark-themed PDF: overview basket fan chart + top-10 player fan charts +
stats table.
"""
import numpy as np
import pandas as pd
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import os, sys, html

BASE = "/home/drewclaw/footy-models/artifacts/mc_36m"
OUT_PDF = "/tmp/footy_mc_36m_report.pdf"
TOP_N_IND = 10
TOP_N_TABLE = 25

paths = pd.read_csv(f"{BASE}/paths_long.tsv", sep="\t")
basket = pd.read_csv(f"{BASE}/paths_basket.tsv", sep="\t")
stats = pd.read_csv(f"{BASE}/stats.tsv", sep="\t")

# money fields in stats.tsv: current_mv is EUR; MC stats (mean/median/p10/p90) are cents -> EUR
for c in ["mean_36m", "median_36m", "p10", "p90"]:
    stats[c] = stats[c] / 100.0

BG = "#0d1117"; PANEL = "#161b22"; TXT = "#e6edf3"; MUT = "#8b949e"
PATH_C = "#1f6feb"; MEAN_C = "#00d4aa"; MED_C = "#f0f6fc"; BAND_C = "#1f6feb"
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

def fan(ax, mat, ydiv, title, sub, yfmt, sub_below=True):
    """mat: (n_paths, n_months) in EUR (or index for basket)."""
    months = np.arange(mat.shape[1])
    for row in mat:
        ax.plot(months, row / ydiv, color=PATH_C, alpha=0.05, lw=0.55, zorder=1)
    mean = mat.mean(axis=0) / ydiv
    med = np.percentile(mat, 50, axis=0) / ydiv
    p10 = np.percentile(mat, 10, axis=0) / ydiv
    p90 = np.percentile(mat, 90, axis=0) / ydiv
    ax.fill_between(months, p10, p90, color=BAND_C, alpha=0.14, zorder=2, label="10–90% band")
    ax.plot(months, mean, color=MEAN_C, lw=2.8, zorder=3, label="Mean path")
    ax.plot(months, med, color=MED_C, lw=1.6, ls="--", zorder=4, label="Median path")
    ax.set_title(title, loc="left")
    ax.set_xlabel("Months from now"); ax.set_ylabel("Market value")
    ax.grid(True, alpha=0.35)
    ax.legend(loc="upper left", fontsize=8, frameon=False, labelcolor=TXT)
    ax.set_xlim(0, mat.shape[1] - 1)
    if sub_below:
        ax.text(0.0, -0.16, sub, transform=ax.transAxes, fontsize=7.5, color=MUT, va="top")

chart_files = []

# ---- overview: basket (index) ----
b = basket.pivot_table(index="path", columns="month", values="index_cents").values  # index x 100 (start = 100)
fig, ax = plt.subplots(figsize=(11, 6.4), dpi=150)
basket_stats = stats.head(25)
sub = (f"{basket_stats.shape[0]}-player equal-weight basket | 1,000 paths, seed 42 | "
       f"drift from model 12m forecast, vol from conformal 80% interval | "
       f"mean 36m index {b.mean(axis=0)[-1]:.0f}, 10–90% "
       f"[{np.percentile(b,10,axis=0)[-1]:.0f}, {np.percentile(b,90,axis=0)[-1]:.0f}]")
fan(ax, b, 1.0, f"Equal-Weight Basket of Top {basket_stats.shape[0]} Forecast Players — 36-Month Monte Carlo (index = 100)",
    sub, None)
ax.set_ylabel("Index (start = 100)")
ax.axhline(100, color=MUT, lw=1, ls=":", alpha=0.6)
fig.tight_layout(rect=(0, 0.06, 1, 1))
f = f"{BASE}/chart_overview.png"; fig.savefig(f, bbox_inches="tight", pad_inches=0.35); plt.close(fig); chart_files.append(f)

# ---- top-N individual fan charts ----
pid_to_meta = {r.player_id: r for r in stats.head(TOP_N_IND).itertuples()}
for rank, (pid, meta) in enumerate(pid_to_meta.items(), 1):
    sub_p = paths[paths.player_id == pid]
    mat = sub_p.pivot_table(index="path", columns="month", values="price_cents").values / 100.0
    sub = (f"#{rank} | {meta.club} | {meta.pos} | age {meta.age:.1f} | CA {meta.ca:.0f}/PA {meta.pa:.0f} | {meta.attr_source} | "
           f"now {fmt_eur(meta.current_mv)} → 12m forecast {fmt_eur(stats.loc[stats.player_id==pid,'current_mv'].iloc[0] * np.exp(stats.loc[stats.player_id==pid,'mu_ann'].iloc[0]))} | "
           f"MC 36m mean {fmt_eur(meta.mean_36m)}, median {fmt_eur(meta.median_36m)}, 10–90% [{fmt_eur(meta.p10)}, {fmt_eur(meta.p90)}] | "
           f"P(≥1.5x) {meta.p15x*100:.0f}%, P(≥2x) {meta.p2x*100:.0f}%, P(transfer 3y) {meta.transfer_prob*100:.0f}%")
    fig, ax = plt.subplots(figsize=(10.5, 5.6), dpi=150)
    fan(ax, mat, 1.0, f"{rank}. {meta.name}", sub, None)
    fig.tight_layout()
    f = f"{BASE}/chart_player_{rank:02d}.png"; fig.savefig(f, bbox_inches="tight"); plt.close(fig); chart_files.append(f)

# ---- stats table (top 25) ----
rows_md = []
for i, r in enumerate(stats.head(TOP_N_TABLE).itertuples(), 1):
    rows_md.append(
        f"<tr><td>{i}</td><td>{html.escape(r.name)}</td><td>{html.escape(str(r.club))}</td>"
        f"<td>{r.pos}</td><td>{r.age:.0f}</td><td>{fmt_eur(r.current_mv)}</td>"
        f"<td>{fmt_eur(r.current_mv * np.exp(r.mu_ann))}</td><td>{fmt_eur(r.mean_36m)}</td>"
        f"<td>{fmt_eur(r.median_36m)}</td><td>{fmt_eur(r.p10)}</td><td>{fmt_eur(r.p90)}</td>"
        f"<td>{r.p15x*100:.0f}%</td><td>{r.p2x*100:.0f}%</td><td>{r.transfer_prob*100:.0f}%</td></tr>"
    )
table_html = "\n".join(rows_md)

imgs = "\n".join(f'<img src="file://{p}" style="width:100%;margin-bottom:14px;">' for p in chart_files)
html_doc = f"""<!DOCTYPE html><html><head><meta charset="utf-8"><style>
body {{ background:#0d1117; color:#e6edf3; font-family:-apple-system,'Segoe UI',Roboto,sans-serif; margin:0; padding:32px 40px; }}
h1 {{ font-size:24px; margin:0 0 4px; }} h2 {{ font-size:17px; margin:26px 0 8px; color:#00d4aa; }}
.mut {{ color:#8b949e; font-size:12px; }} .box {{ background:#161b22; border:1px solid #30363d; border-radius:8px; padding:14px 18px; margin:14px 0; }}
table {{ border-collapse:collapse; width:100%; font-size:11px; margin-top:8px; }}
th,td {{ border:1px solid #30363d; padding:5px 7px; text-align:left; }} th {{ background:#1c2128; color:#00d4aa; }}
</style></head><body>
<h1>36-Month Monte Carlo Value Forecast</h1>
<div class="mut">Footy-Models · U23 / FM PA ≥ 140 / MV ≤ €1M · FM26 × Transfermarkt · simulated with the repo's own Rust primitives (crates/forecast: Rng64, FootballPathModel, monte_carlo_paths) · 1,000 paths per player · seed 42</div>
<div class="box"><b>Method.</b> Each player's value evolves monthly as a log-normal process. Annual drift = the model's 12-month log forecast annualized (μ = ln(forecast/current)/1y); annual vol = conformal 80% interval width converted to σ (radius 0.591 log-ratio / 1.2816 = 0.461). Driver noise channels (development/performance) are disabled so the simulated dispersion equals the forecast model's uncertainty; the transfer hazard (25%/yr, contract-boosted) and contract bookkeeping come from <code>FootballPathModel</code>. 1,000 deterministic paths × 37 monthly points × 36-month horizon. Mean path highlighted; dashed = median; shaded = 10–90% band.</div>
{imgs}
<h2>Monte Carlo 36-Month Statistics — Top {TOP_N_TABLE}</h2>
<table><tr><th>#</th><th>Player</th><th>Club</th><th>Pos</th><th>Age</th><th>Current</th><th>12m Forecast</th><th>MC Mean 36m</th><th>MC Median</th><th>P10</th><th>P90</th><th>P(≥1.5×)</th><th>P(≥2×)</th><th>P(transfer 3y)</th></tr>
{table_html}</table>
<div class="mut" style="margin-top:18px;">Prices in EUR. MC statistics from the Rust simulation (footy-mc, apps/mc). The mean path is pulled up by the log-normal right tail — the median is the more typical outcome. Calibrated forecasts, not guarantees.</div>
</body></html>"""
with open("/tmp/mc_36m_report.html", "w") as fh:
    fh.write(html_doc)

import weasyprint
weasyprint.HTML(string=html_doc).write_pdf(OUT_PDF)
print(f"PDF written: {OUT_PDF} ({os.path.getsize(OUT_PDF)} bytes)")
print("charts:", len(chart_files))
