#!/usr/bin/env python3
"""Build data/forecast_36m_report.md + data/forecast_36m_report.pdf (STEERING_36M.md).

White-background charts (artifacts/mc_36m_white/) were produced from the
deterministic Rust MC (apps/mc + crates/forecast). This script assembles the
markdown report and renders the white-theme PDF with weasyprint.
"""
import numpy as np
import pandas as pd
import os, html

REPO = "/home/drewclaw/footy-models"
BASE = f"{REPO}/artifacts/mc_36m_white"
TOP_N_IND = 10
BASKET_N = 25
N_PATHS = 1000
SEED = 42
SIGMA_ANN = 0.4688
MODEL_HASH = "1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f"

# ---------------- data ----------------
all_df = pd.read_csv(f"{REPO}/data/forecast_36m_all.csv")
for c in ["age","current_mv","fm_ca","fm_pa","predicted_12m_mv","gap_eur","gap_pct",
          "interval_10","interval_90","predicted_36m_mv","gap_36m_eur","gap_36m_pct","mu_12"]:
    all_df[c] = pd.to_numeric(all_df[c], errors="coerce")
top100 = pd.read_csv(f"{REPO}/data/forecast_36m_top100.csv")
for c in ["age","current_mv","fm_ca","fm_pa","predicted_12m_mv","gap_eur","gap_pct","interval_10","interval_90"]:
    top100[c] = pd.to_numeric(top100[c], errors="coerce")
stats = pd.read_csv(f"{BASE}/stats.tsv", sep="\t")
for c in ["mean_36m","median_36m","p10","p90"]:
    stats[c] = stats[c] / 100.0
basket = pd.read_csv(f"{BASE}/paths_basket.tsv", sep="\t")
b_idx = basket.pivot_table(index="path", columns="month", values="index_cents").values.astype(float)  # index (start = 100)
bt = b_idx[:, -1]

def eur(v, d=0):
    return f"€{v:,.{d}f}" if abs(v) < 1e6 else f"€{v/1e6:,.2f}M"

def eur_k(v):
    if abs(v) >= 1e6: return f"€{v/1e6:.2f}M"
    return f"€{v/1e3:,.0f}k"

# ---------------- report markdown ----------------
md = []
md.append("# Footy-Models — 36-Month Value Forecast: U23, TM value €400k–€1.2M, Top-100 by % Gain\n")
md.append("## 0. Executive summary\n")
md.append(f"- **Scored pool**: all players aged **< 23** with Transfermarkt value **€400,000–€1,200,000**, FM attributes present (`attr_source` in {{db, derived}}; the 23 `staff` rows are excluded). **948 players scored** out of the 971-row pool (849 db attrs / 99 derived / 23 staff excluded).")
md.append(f"- **Model**: deployed v2 forecast artifact `data/forecast/model.bin`, SHA-256 `{MODEL_HASH}` (byte-identical after the run), ridge = 1,000,000, seed 42, 47-feature causal schema, trained through 2025-06-12, walk-forward Spearman IC 0.5063 (pooled OOF) / real forward-test IC 0.5100 (as-of 2025-11-06 → realized TM through 2026-06-12).")
md.append(f"- **36-month view**: model-consistent compounding of the 12-month log forecast: `predicted_36m_mv = current_mv * exp(3 * mu_12)` with `mu_12 = ln(predicted_12m_mv / current_mv)`; ranking by `gap_pct_36m = exp(3 * mu_12) - 1` (descending).")
md.append(f"- **Uncertainty**: 1,000-path Monte Carlo per player (Rust `apps/mc` + `crates/forecast`), 36 monthly steps, seed {SEED}; annual vol = v2 conformal radius 0.6009 / 1.2816 ≈ **0.4688**; drift = the model's annualized 12m log forecast. Top-10 fan charts + top-25 basket on a **white background** (`artifacts/mc_36m_white/`).")
md.append(f"- **Costs**: tables below are **gross of transaction costs**; a 15 bps cost applies wherever a portfolio is evaluated (see Limitations).")
md.append("\n## 1. Methodology\n")
md.append("### 1.1 Model (used exactly as deployed — no retraining, no re-sweep)\n")
md.append("| Item | Value |\n|---|---|\n")
md.append(f"| Artifact | `data/forecast/model.bin` |\n")
md.append(f"| SHA-256 | `{MODEL_HASH}` |\n")
md.append("| Ridge λ | 1,000,000 |\n")
md.append("| Seed | 42 |\n")
md.append("| Schema | 47-feature causal (standardized + paired missingness indicators → 94 model inputs) |\n")
md.append("| Trained through | 2025-06-12 |\n")
md.append("| Walk-forward Spearman IC | 0.5063 (pooled OOF) |\n")
md.append("| Real forward-test IC | 0.5100 (as-of 2025-11-06 → realized TM through 2026-06-12) |\n")
md.append("| Conformal 80% radius (12m) | 0.6009 → annual σ = 0.6009 / 1.2816 ≈ 0.4688 |\n")
md.append("\n### 1.2 Candidate pool\n")
md.append("- Source: `data/fm_tm_joined.csv` (14,783 rows; **merging is done — not re-merged**).")
md.append("- Filter: `age < 23` AND `400000 <= current_mv <= 1200000` AND `attr_source != 'staff'`.")
md.append("- Pool count: **971** players (849 db attrs / 99 derived / 23 staff). The 23 staff rows are excluded → **948 scored**; db vs derived stays flagged in every output.")
md.append("- No PA filter on this run (U23 + value band only, per spec).")
md.append("- Missing data is not zero: absent features are NA + missingness indicators; censored/absent players are dropped, not zero-filled.")
md.append("\n### 1.3 Scoring (existing primitives)\n")
md.append("- Candidates: `data/forecast_36m/candidates.tsv` + `candidates_meta.tsv` built with the exact `build_forecast_dataset.py` feature extractor (as-of each player's latest Transfermarkt valuation date, strictly-prior observations only).")
md.append("- Materialize: `footy-train forecast-materialize-candidates --input data/forecast_36m/candidates.tsv --dataset-version 1 --feature-set-version 1 --output data/forecast_36m/candidates_matrix.bin` (948 rows, 47 features).")
md.append("- Score: `footy-score forecast --model data/forecast/model.bin --candidates data/forecast_36m/candidates_matrix.bin --meta data/forecast_36m/candidates_meta.tsv --top-n 1000 --min-pa 0 --max-mv 1200000 --rows 0 --output data/forecast_36m_all.csv` → all 948 scored players (12m columns retained: `predicted_12m_mv`, `gap_eur`, `gap_pct`, conformal `interval_10`/`interval_90`).")
md.append("\n### 1.4 36-month compounding (model-consistent)\n")
md.append("```\nmu_12             = ln(predicted_12m_mv / current_mv)     # model 12-month log ratio\npredicted_36m_mv  = current_mv * exp(3 * mu_12)\ngap_pct_36m       = exp(3 * mu_12) - 1                       # rank key, descending\ngap_eur_36m       = predicted_36m_mv - current_mv\n```\n")
md.append("The 12m conformal interval columns are kept as-is (they are 12m; 36m chart uncertainty comes from the MC fan).")
md.append("\n### 1.5 36-month Monte Carlo (Rust `apps/mc`, `crates/forecast`)\n")
md.append(f"- Drift: `mu_ann = mu_12` (annualized model forecast); vol: `sigma_ann = 0.4688` (v2 conformal radius).")
md.append(f"- {N_PATHS:,} paths per player, 36 monthly steps, seed {SEED}. Driver noise channels (development/performance) off → dispersion = forecast-model uncertainty; transfer hazard 25%/yr + contract bookkeeping from `FootballPathModel`.")
md.append(f"- Determinism verified: re-running `footy-mc run` reproduces `paths_long.tsv` / `paths_basket.tsv` / `stats.tsv` byte-identically.")
md.append(f"- Charts on **white background** (`#ffffff` figure/axes, dark-gray text, light grid), saved under `artifacts/mc_36m_white/`.\n")

# ---------------- pool overview ----------------
md.append("## 2. Pool and scored overview\n")
md.append(f"| Metric | Value |\n|---|---|\n")
md.append(f"| Pool rows (pre-screen) | 971 (849 db / 99 derived / 23 staff) |\n")
md.append(f"| Scored rows (`data/forecast_36m_all.csv`) | {len(all_df)} |\n")
md.append(f"| — db attrs | {(all_df['attr_source']=='db').sum()} |\n")
md.append(f"| — derived attrs | {(all_df['attr_source']=='derived').sum()} |\n")
md.append(f"| Age range | {all_df['age'].min():.1f} – {all_df['age'].max():.1f} |\n")
md.append(f"| MV range (€) | {all_df['current_mv'].min():,.0f} – {all_df['current_mv'].max():,.0f} |\n")
md.append(f"| Positions | " + ", ".join(f"{k}={v}" for k,v in all_df['pos_cls'].value_counts().items()) + " |\n")
md.append(f"| 12m forecast mean (€) | {all_df['predicted_12m_mv'].mean():,.0f} |\n")
md.append(f"| 36m % gain mean | {all_df['gap_36m_pct'].mean()*100:.1f}% |\n")
md.append(f"| 36m % gain median | {all_df['gap_36m_pct'].median()*100:.1f}% |\n")
md.append(f"| 36m € gap mean (€) | {all_df['gap_36m_eur'].mean():,.0f} |\n")
md.append("\n")

# ---------------- top-10 profile cards ----------------
md.append("## 3. Top-10 profiles with Monte Carlo statistics\n")
md.append("MC stats from the Rust simulation (1,000 paths, seed 42). `P(≥1.5×)`/`P(≥2×)` are the share of paths ending at ≥1.5×/≥2.0× the current value; `P(transfer 3y)` is the share of paths with a transfer event over 36 months.\n")
for i, r in enumerate(stats.head(TOP_N_IND).itertuples(), 1):
    t = top100[top100['player_id'] == r.player_id].iloc[0]
    fc12 = r.current_mv * np.exp(r.mu_ann)
    md.append(f"### {i}. {r.name}\n")
    md.append(f"- **Club / pos / age**: {r.club} · {r.pos} · {r.age:.1f} yrs  |  **CA/PA**: {r.ca:.0f}/{r.pa:.0f}  |  **attrs**: {r.attr_source}")
    md.append(f"- **Now**: {eur(r.current_mv)}  →  **12m forecast**: {eur(fc12)}  |  **36m forecast (deterministic compounding)**: {eur(r.current_mv * np.exp(3*r.mu_ann))}  |  **36m % gain**: {t['gap_pct']*100:.1f}%")
    md.append(f"- **MC 36m (1,000 paths, seed 42)**: mean {eur(r.mean_36m)}, median {eur(r.median_36m)}, 10–90% [{eur(r.p10)}, {eur(r.p90)}]; P(≥1.5×) {r.p15x*100:.0f}%, P(≥2×) {r.p2x*100:.0f}%, P(transfer 3y) {r.transfer_prob*100:.0f}%")
    md.append(f"- **Chart**: `artifacts/mc_36m_white/chart_player_{i:02d}.png`\n")

# ---------------- top-100 table ----------------
md.append("## 4. Top-100 by expected 36-month % gain (gross of costs)\n")
md.append("Ranked by `gap_pct_36m` descending (tie-break `player_id` ascending). All rows: age < 23, €400k ≤ current_mv ≤ €1.2M, attr_source ≠ staff. Amounts in EUR; `gap_eur`/`gap_pct` are the **36-month** gap. Gross of the 15 bps transaction cost (applies at portfolio evaluation).\n")
md.append("| # | player_id | Name | Club | Comp | Pos | Age | Current MV | 12m fcast | gap_eur (36m) | gap_pct (36m) | attrs |\n")
md.append("|---|---|---|---|---|---|---|---|---|---|---|---|---|\n")
for i, r in enumerate(top100.itertuples(), 1):
    name = str(r.name).replace("|","/")
    club = str(r.club).replace("|","/")[:22]
    comp = str(r.competition).replace("|","/")[:18]
    md.append(f"| {i} | {r.player_id} | {name[:30]} | {club} | {comp} | {r.pos_cls} | {r.age:.1f} | {eur_k(r.current_mv)} | {eur_k(r.predicted_12m_mv)} | {eur_k(r.gap_eur)} | {r.gap_pct*100:.1f}% | {r.attr_source} |\n")
md.append("\n")

# ---------------- basket overview ----------------
md.append("## 5. Basket overview (top-25 equal weight)\n")
md.append(f"Equal-weight basket of the top {BASKET_N} by expected 36m % gain, each player normalized to 1.0 (index start = 100), {N_PATHS:,} paths, seed {SEED}.\n")
md.append("| Basket stat (36m) | Value |\n|---|---|\n")
md.append(f"| Mean index | {bt.mean():.0f} |\n")
md.append(f"| Median index | {np.median(bt):.0f} |\n")
md.append(f"| 10–90% range | [{np.percentile(bt,10):.0f}, {np.percentile(bt,90):.0f}] |\n")
md.append(f"| P(≥1.5×) | {(bt>=150).mean()*100:.0f}% |\n")
md.append(f"| P(≥2×) | {(bt>=200).mean()*100:.0f}% |\n")
md.append(f"| P(≤0.5×) | {(bt<=50).mean()*100:.1f}% |\n")
md.append(f"- Chart: `artifacts/mc_36m_white/chart_overview.png`\n")
md.append("\n## 6. White-background chart inventory\n")
md.append("All charts use the white palette (`#ffffff` figure/axes, text `#1f2328`/`#57606a`, grid `#d0d7de`, paths `#9cc3f5`, mean `#c62828`, median dashed dark gray, 10–90% band `#d6e6fb`), subtitle below the x-axis.\n")
md.append("| File | Content |\n|---|---|\n")
md.append("| `artifacts/mc_36m_white/chart_overview.png` | Top-25 basket fan (index = 100) |\n")
for i in range(1, 11):
    md.append(f"| `artifacts/mc_36m_white/chart_player_{i:02d}.png` | #{i} player fan (1,000 paths, 36 months) |\n")
md.append("\n## 7. Reproducibility and limitations\n")
md.append(f"- Seed 42 used for model training, scoring determinism, and all MC paths; `footy-mc` re-run reproduces identical artifacts (verified byte-for-byte).")
md.append(f"- Deterministic ranking (gap_pct_36m desc, player_id asc); identical inputs → identical outputs.")
md.append("- `data/forecast/model.bin` untouched: SHA-256 matches `1d023e96…` before and after the run.")
md.append("- `data/forecast_u23_top100.csv`, `data/forecast_u23_top100_v2.csv`, `data/forecast/model.bin` not overwritten.")
md.append("- **attr_source honesty**: db vs derived flagged in every output; staff rows excluded; no zero-fill of missing data (censored/absent players dropped).")
md.append("- Tables are gross of transaction costs; a 15 bps cost applies wherever a portfolio is evaluated (top-N tables may be gross, labeled as such — they are here).")
md.append("- Monte Carlo dispersion equals the forecast model's conformal uncertainty (driver noise off); transfer hazard is a fixed 25%/yr parameter, not a re-estimated per-player hazard.")
md.append("- Transfermarkt DuckDB accessed read-only; `fm_tm_joined.csv` not re-merged.\n")

report_md = "\n".join(md)
with open(f"{REPO}/data/forecast_36m_report.md", "w") as f:
    f.write(report_md)
print("report md:", len(report_md), "bytes ->", f"{REPO}/data/forecast_36m_report.md")

# ---------------- PDF ----------------
chart_files = [f"{BASE}/chart_overview.png"] + [f"{BASE}/chart_player_{i:02d}.png" for i in range(1, 11)]
imgs = "\n".join(f'<img src="file://{p}" style="width:100%;margin-bottom:16px;">' for p in chart_files)

# top-25 stats table html
rows_html = []
for i, r in enumerate(stats.head(25).itertuples(), 1):
    fc12 = r.current_mv * np.exp(r.mu_ann)
    rows_html.append(
        f"<tr><td>{i}</td><td>{html.escape(str(r.name))}</td><td>{html.escape(str(r.club))}</td>"
        f"<td>{r.pos}</td><td>{r.age:.1f}</td><td>{eur_k(r.current_mv)}</td>"
        f"<td>{eur_k(fc12)}</td><td>{eur_k(r.mean_36m)}</td><td>{eur_k(r.median_36m)}</td>"
        f"<td>{eur_k(r.p10)}</td><td>{eur_k(r.p90)}</td><td>{r.p15x*100:.0f}%</td><td>{r.p2x*100:.0f}%</td><td>{r.transfer_prob*100:.0f}%</td></tr>"
    )
table_html = "\n".join(rows_html)

# top-100 table html (condensed columns)
rows100 = []
for i, r in enumerate(top100.itertuples(), 1):
    rows100.append(
        f"<tr><td>{i}</td><td>{html.escape(str(r.name))}</td><td>{html.escape(str(r.club))}</td>"
        f"<td>{r.pos_cls}</td><td>{r.age:.1f}</td><td>{eur_k(r.current_mv)}</td>"
        f"<td>{eur_k(r.predicted_12m_mv)}</td><td>{r.gap_pct*100:.1f}%</td><td>{r.attr_source}</td></tr>"
    )
table100_html = "\n".join(rows100)

html_doc = f"""<!DOCTYPE html><html><head><meta charset="utf-8"><style>
@page {{ size: A4; margin: 16mm 14mm; }}
body {{ background:#ffffff; color:#1f2328; font-family:-apple-system,'Segoe UI',Roboto,'Helvetica Neue',Arial,sans-serif; margin:0; padding:8px 6px; }}
h1 {{ font-size:22px; margin:0 0 6px; color:#1f2328; }}
h2 {{ font-size:16px; margin:22px 0 8px; color:#1f2328; border-bottom:2px solid #d0d7de; padding-bottom:3px; }}
h3 {{ font-size:13px; margin:14px 0 4px; color:#1f2328; }}
.mut {{ color:#57606a; font-size:11px; }}
table {{ border-collapse:collapse; width:100%; font-size:8.5px; margin:8px 0; }}
th,td {{ border:1px solid #d0d7de; padding:3px 5px; text-align:left; }}
th {{ background:#f6f8fa; color:#1f2328; }}
code {{ background:#f6f8fa; padding:1px 4px; border-radius:3px; font-size:10px; }}
.box {{ border:1px solid #d0d7de; border-radius:6px; padding:10px 14px; margin:10px 0; background:#fcfcfd; }}
</style></head><body>
<h1>36-Month Value Forecast — U23, TM value €400k–€1.2M, Top-100 by % Gain</h1>
<div class="mut">Footy-Models · scoring: deployed v2 forecast model (ridge 1,000,000, seed 42, SHA-256 1d023e96…) · Monte Carlo: Rust apps/mc + crates/forecast · 1,000 paths · seed 42 · white-background charts</div>
<div class="box"><b>Method.</b> Pool = age &lt; 23, €400k ≤ current_mv ≤ €1.2M, attr_source ≠ staff (948 scored of 971; 23 staff excluded). 36m compounding: mu_12 = ln(predicted_12m_mv / current_mv); predicted_36m_mv = current_mv·exp(3·mu_12); gap_pct_36m = exp(3·mu_12) − 1. MC: drift = mu_12 (annualized), vol = 0.4688 (v2 conformal 0.6009/1.2816), 36 monthly steps, seed 42. Tables are gross of the 15 bps cost applied at portfolio evaluation.</div>
{imgs}
<h2>Monte Carlo 36-Month Statistics — Top 25</h2>
<table><tr><th>#</th><th>Player</th><th>Club</th><th>Pos</th><th>Age</th><th>Current</th><th>12m Fcast</th><th>MC Mean 36m</th><th>MC Median</th><th>P10</th><th>P90</th><th>P(≥1.5×)</th><th>P(≥2×)</th><th>P(transfer 3y)</th></tr>
{table_html}</table>
<h2>Top-100 by 36-Month % Gain (gross of costs)</h2>
<table><tr><th>#</th><th>Name</th><th>Club</th><th>Pos</th><th>Age</th><th>Current MV</th><th>12m Fcast</th><th>gap_pct (36m)</th><th>attrs</th></tr>
{table100_html}</table>
<div class="mut" style="margin-top:14px;">Prices in EUR. MC stats from the deterministic Rust simulation; the mean path is pulled up by the log-normal right tail — the median is the more typical outcome. Calibrated forecasts, not guarantees. attr_source flags db vs derived; staff excluded.</div>
</body></html>"""
with open("/tmp/forecast_36m_report.html", "w") as fh:
    fh.write(html_doc)

import weasyprint
weasyprint.HTML(string=html_doc).write_pdf(f"{REPO}/data/forecast_36m_report.pdf")
print("PDF:", f"{REPO}/data/forecast_36m_report.pdf", os.path.getsize(f"{REPO}/data/forecast_36m_report.pdf"), "bytes")
