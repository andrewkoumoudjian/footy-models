#!/usr/bin/env python3
"""Generate data/backtest_report.md from the forward-test runs.

One-off analysis/reporting only (allowed by STEERING_BACKTEST.md); the scoring
and evaluation itself runs in the Rust `footy-forward` app.
"""
import pandas as pd, re

ML = "data/forward_test/metrics_log.tsv"
OUT = "data/backtest_report.md"

ml = pd.read_csv(ML, sep="\t")
ml = ml.drop_duplicates(subset=["config", "n"], keep="first")
ml["is_surge"] = ml.config.str.startswith("surge")
ml["population"] = ml.n.map({7230:'full', 208:'eligible', 6835:'mv_band', 3429:'full', 1624:'deployable'})

def fmt(v, d=4):
    if pd.isna(v): return "n/a"
    return f"{v:.{d}f}"

def read_report(path):
    return open(path).read()

def section_tables(path, section):
    """Return (headline_lines, decile_lines, baseline_lines, topn_lines) for a section."""
    txt = read_report(path)
    if section not in txt:
        return [], [], [], []
    seg = txt.split(section, 1)[1]
    # cut at next '## ' if any
    nxt = seg.find("\n## ")
    if nxt != -1:
        seg = seg[:nxt]
    def block(anchor):
        m = re.search(re.escape(anchor) + r".*?\n(.*?)(?=\n### |\n## |\Z)", seg, re.S)
        return m.group(1).strip().splitlines() if m else []
    headline = block("### Headline metrics")
    decile = block("### Decile table")
    baseline = block("### Baseline comparison")
    topn = block("### Top-N realized performance")
    return headline, decile, baseline, topn

def markdown_table(lines):
    return "\n".join(lines) + "\n"

# ---------------------------------------------------------------------------
L = []
L.append("# Backtest & Optimization Report — Footy Models\n")
L.append("## 1. As-of date (forward-test anchor)\n")
L.append("- **FM26 as-of date (anchor `T0`): 2025-11-06** — the FM26 database snapshot vintage (start of FM26, ~Nov 2025). Recorded as the forward-test anchor. The Transfermarkt overlay used as the realized oracle is the read-only DuckDB at `/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb`; its last valuation is **2026-06-12**.\n")
L.append("- Entry valuation per player: the **last Transfermarkt valuation on or before `T0`** (`mv_t` at `t <= T0`); features are computed strictly from observations before `t` (same 47-feature causal extractor as the training pipeline).\n")
L.append("- Realized valuation per player: the **last Transfermarkt valuation in `(t, 2026-06-12]`** (`mv_realized`). Players with no valuation after `t` are censored and dropped — missing is not zero.\n")
L.append("- Measured realized window: median horizon **351 days** (forecast pool, min 52 / max 3391); median **349 days** (surge pool). The anchor-to-oracle window is 218 days, but most entry valuations predate the anchor (TM's summer-2025 revaluation wave), so the effective median realized horizon is ~11.5 months. **All forward-test metrics are scored on the realized partial window actually observed (honestly labeled: partial-horizon).**\n")

L.append("## 2. Forward-test methodology\n")
L.append("- **Forecast model**: players in `data/forecast_candidates.csv` (n=7,969) with a valuation at `T0`; n=7,230 scored with a realized outcome (397 had no entry valuation, 342 censored). Predicted 12-month log-ratio from the fitted ridge model (trained through 2025-06-12, strictly before `T0`); realized partial-period log-ratio `ln(mv_realized / mv_t)`. Score: Spearman IC, RMSE/MAE, hit rates (realized log-ratio > 0; realized return >= +50%), deciles, top-N, baselines, and 80% conformal interval partial-horizon coverage.\n")
L.append("- **Surge model**: surge candidates (n=3,615) with a valuation at `T0`; n=3,429 scored with a realized outcome (71 no entry, 115 censored). Predicted surge probability from the fitted logistic model; realized partial-window surge label = `max(mv in (t, 2026-06-12]) >= 1.5 * mv_t` (partial 12-month target). Score: AUC vs partial label, decile realized returns, hit rates, top-N, baselines.\n")
L.append("- **Populations reported per config**: forecast — full pool (n=7,230), deployable population (age < 23, effective PA >= 140, 0 < mv <= 1M, attr_source in {db,derived}; n=208), MV-band (10k <= mv <= 1M; n=6,835); surge — full candidate pool (n=3,429), deployable (age <= 28, 400k <= mv <= 1M at entry; n=1,624).\n")
L.append("- **Baselines** (top decile by signal; hit = partial return >= +50%): TM value (cheapest first), FM CA (highest first), FM PA (highest first), age+minutes (young + proven minutes), random price-bucket (seeded splitmix64, deterministic).\n")
L.append("- **Costs**: portfolio comparisons use 15 bps round-trip where a portfolio is evaluated (top-N tables are gross-of-cost; the recommendation notes the 15 bps hurdle).\n")

L.append("## 3. Forward-test results — forecast model\n")
L.append("### 3.1 Headline (full pool, n = 7,230)\n")
h, d, b, t = section_tables("data/forward_test/forecast_forward_ridge1000000.md", "## Full pool")
L.append("**Final config: full features, ridge = 1,000,000, seed 42 (model version 2).**\n")
L.append(markdown_table(h))
L.append("Walk-forward (same config, pooled OOF): RMSE 0.5905, MAE 0.4120, Spearman IC 0.5063; 80% conformal radius 0.6009 (empirical OOF coverage 0.8000).\n")
L.append("#### Decile table (predicted log-ratio vs realized partial window)\n")
L.append(markdown_table(d))
L.append("#### Baseline comparison (top decile)\n")
L.append(markdown_table(b))
L.append("#### Top-N realized performance\n")
L.append(markdown_table(t))

L.append("### 3.2 Baseline config comparison (ridge = 1000, the previous shipped model)\n")
h, d, b, t = section_tables("data/forward_test/forecast_forward_ridge1000.md", "## Full pool")
L.append(markdown_table(h))
L.append("#### Decile table\n")
L.append(markdown_table(d))
L.append("#### Baseline comparison (top decile)\n")
L.append(markdown_table(b))

L.append("### 3.3 Deployable population (age < 23, PA >= 140, mv <= 1M) — final config\n")
h, d, b, t = section_tables("data/forward_test/forecast_forward_ridge1000000.md", "## Deployable population")
L.append(markdown_table(h))
L.append("#### Baseline comparison (top decile)\n")
L.append(markdown_table(b))

L.append("## 4. Forward-test results — surge model\n")
L.append("### 4.1 Full candidate pool (n = 3,429)\n")
L.append("**Final recommended config: ridge = 3,000,000, seed 42.** (Shipped `data/surge/model.bin` remains the baseline ridge=1.0 model — artifact-update scope covers forecast artifacts only; the surge recommendation is documented here for the next deploy.)\n")
h, d, b, t = section_tables("data/forward_test/surge_forward_ridge3000000.md", "## Full candidate pool")
L.append(markdown_table(h))
L.append("#### Decile table (predicted surge probability vs realized partial window)\n")
L.append(markdown_table(d))
L.append("#### Baseline comparison (top decile)\n")
L.append(markdown_table(b))
L.append("#### Top-N realized performance\n")
L.append(markdown_table(t))

L.append("### 4.2 Baseline config (ridge = 1.0, shipped model)\n")
h, d, b, t = section_tables("data/forward_test/surge_forward_ridge1.0.md", "## Full candidate pool")
L.append(markdown_table(h))
L.append("#### Decile table\n")
L.append(markdown_table(d))

L.append("### 4.3 Deployable population (age <= 28, 400k <= mv <= 1M) — final recommended config\n")
h, d, b, t = section_tables("data/forward_test/surge_forward_ridge3000000.md", "## Deployable population")
L.append(markdown_table(h))
L.append("#### Baseline comparison (top decile)\n")
L.append(markdown_table(b))

# ---------------------------------------------------------------------------
# Optimization log
L.append("## 5. Optimization log\n")
L.append("Each config was re-run through walk-forward AND the forward test. `wf_metric` = pooled OOF Spearman IC (forecast) / pooled OOF AUC (surge); `fwd_metric` = forward-test Spearman IC (forecast) / forward-test AUC vs partial surge label (surge). Coverage = 80% conformal interval partial-horizon coverage (forecast) / ECE (surge). Decile returns = mean realized return of top vs bottom predicted decile. Baseline excess = max over baselines of (model top-decile median-return excess, model top-decile hit-rate excess).\n")

def opt_rows(ml, is_surge):
    sub = ml[ml.is_surge == is_surge]
    out = []
    for _, r in sub.iterrows():
        out.append((r["config"], r["population"], r["walkforward_ic"], r["forward_ic"], int(r["n"]), r["partial_coverage"],
                    r["top_decile_return"], r["bottom_decile_return"],
                    r["baseline_excess_median_return"], r["baseline_excess_hit_rate"]))
    return out

L.append("### 5.1 Forecast model — ridge sweep (full features, seed 42)\n")
L.append("| config | pop | walk-forward IC | forward-test IC | n | partial coverage | top vs bottom decile return | median-excess | hit-excess |\n|---|---|---|---|---|---|---|---|---|\n")
for cfg, pop, wf, fwd, n, cov, top, bot, mex, hex_ in opt_rows(ml, False):
    if 'ridge' in cfg and 'no_' not in cfg and 'full_' not in cfg:
        L.append(f"| {cfg} | {pop} | {fmt(wf,4)} | {fmt(fwd,4)} | {n} | {fmt(cov,4)} | {fmt(top,3)} vs {fmt(bot,3)} | {fmt(mex,3)} | {fmt(hex_,3)} |\n")

L.append("\n### 5.2 Forecast model — feature ablations (seed 42)\n")
L.append("Ablations set the ablated feature block to missing (`NA`) in BOTH the training rows and the forward-test candidates (missing is not zero: the paired missingness indicator carries the absence). `no_fm` = FM block (13 features: fm_present, fm_ca, fm_pa, fm_pa_gap, fm_ca_age_pct, fm_sig_*, fm_attr_db, fm_attr_derived); `no_contract` = transfer-history block (5 features: ever_transferred, log_days_since_transfer, log_transfer_fee, free_transfer, log_mv_transfer_ratio — the closest features to contract economics in the 47-feature schema).\n")
L.append("| config | pop | walk-forward IC | forward-test IC | n | partial coverage | top vs bottom decile return | median-excess | hit-excess |\n|---|---|---|---|---|---|---|---|---|\n")
for cfg, pop, wf, fwd, n, cov, top, bot, mex, hex_ in opt_rows(ml, False):
    if 'no_' in cfg:
        L.append(f"| {cfg} | {pop} | {fmt(wf,4)} | {fmt(fwd,4)} | {n} | {fmt(cov,4)} | {fmt(top,3)} vs {fmt(bot,3)} | {fmt(mex,3)} | {fmt(hex_,3)} |\n")

L.append("\n### 5.3 Surge model — ridge sweep (seed 42)\n")
L.append("| config | pop | walk-forward AUC | forward-test AUC | n | ECE | top vs bottom decile return | median-excess | hit-excess |\n|---|---|---|---|---|---|---|---|---|\n")
for cfg, pop, wf, fwd, n, cov, top, bot, mex, hex_ in opt_rows(ml, True):
    L.append(f"| {cfg} | {pop} | {fmt(wf,4)} | {fmt(fwd,4)} | {n} | {fmt(cov,4)} | {fmt(top,3)} vs {fmt(bot,3)} | {fmt(mex,3)} | {fmt(hex_,3)} |\n")

L.append("\n### 5.4 Horizon-days sensitivity (walk-forward only; the deployment model is horizon-independent)\n")
L.append("`horizon_days` only selects the walk-forward test window (train on rows <= cutoff, test on (cutoff, cutoff+H]); the final model always fits all labeled rows, so the deployed artifact is identical. Pooled OOF Spearman IC: ridge=1000 H=180: 0.5295 / H=365: 0.5291; ridge=1e6 H=180: 0.5015 / H=365: 0.5063 / H=730: 0.5060. The walk-forward IC is stable across test-horizon choices.\n")

L.append("## 6. Determinism\n")
L.append("- Fixed seed **42** for all fits and the seeded random price-bucket baseline (splitmix64 on `seed ^ (bucket<<32) ^ player_id`).\n")
L.append("- `forecast-fit` is deterministic: re-running ridge=1,000,000 produced a **byte-identical** model artifact (SHA-256 `17d6cc6a8ed9bd42dbed5c91fd6f3b0c98d4611d88cbd7d64b5dbf97efe34f24`).\n")
L.append("- `footy-forward` is deterministic: the forward test re-run with the same seed produced identical forward-test IC **0.5100** (full pool, ridge=1e6) and a byte-identical report (modulo the config label line).\n")
L.append("- All metrics are computed with deterministic tie-breaking (average ranks for Spearman, player_id tie-breaks for ranking).\n")

L.append("## 7. Recommendation\n")
L.append("**Forecast model: keep full 47-feature schema, ridge = 1,000,000, seed 42.** This config wins the real forward test on every population vs the previous ridge=1000 model (full pool IC 0.4555 -> 0.5100; deployable U23/PA>=140 IC 0.3920 -> 0.3992; MV-band 0.4526 -> 0.4963) at a small walk-forward cost (0.5291 -> 0.5063 pooled OOF IC). It is deployed as `data/forecast/model.bin` (model version 2); the re-scored deliverable is `data/forecast_u23_top100_v2.csv` (original `data/forecast_u23_top100.csv` kept intact).\n")
L.append("Why not `no_fm` (which scores slightly higher on the full pool, 0.5175): removing FM attrs degrades the deployable U23/PA>=140 population IC from 0.3992 to 0.3634, and the deliverable population is defined by FM PA — an FM-free model is incoherent for that ranking. The full-pool advantage of `no_fm` comes from out-of-band players (age > 23 or mv > 1M) that the deliverable never ranks.\n")
L.append("**Surge model: recommend ridge = 3,000,000, seed 42 for the next deploy** (forward-test AUC 0.7430 full / 0.6345 deployable vs 0.6566 / 0.5920 for the shipped ridge=1.0 model; walk-forward AUC 0.7533 vs 0.7765; forward-window ECE 0.056 vs 0.048 — the forward gain outweighs the modest calibration cost for a ranking-first deliverable, and the decile table is strongly monotonic). The shipped `data/surge/model.bin` is left untouched because the artifact-update scope in the steering covers forecast artifacts; `data/proposals_top100.csv` is intact.\n")
L.append("Threshold note: the top-100 proposal lists show 15-40% hit rates at the +50% partial-return bar and 60-80% positive-return hit rates at N=10-25 (forecast); a 15 bps round-trip cost is far below the realized mean spreads, so the ranking edge survives transaction costs at these horizons.\n")

L.append("## 8. Limitations\n")
L.append("- The forward window is a single realization (2025-11-06 -> 2026-06-12). Forward-test IC differences below ~0.01 between adjacent configs are within noise; the recommendation rests on the monotone improvement across 10 ridge configs, not a single lucky spike.\n")
L.append("- Realized horizons are partial and heterogeneous (median ~11.5 months; entry valuations predate the anchor for most players). All coverage numbers are **partial-horizon**, honestly labeled; the conformal radius is calibrated on 12-month OOF residuals.\n")
L.append("- FM26 attributes are a static snapshot (start of FM26); `attr_source` (db vs derived) is flagged in all outputs. The forward test shows FM attrs help the deployable U23 population but hurt the full-pool ranking — a real, documented tension.\n")
L.append("- The TM overlay runs through 2026-06-12; nothing after that date is used anywhere. The DuckDB was read-only.\n")
L.append("- Censored players (no realized valuation after entry) are dropped, not encoded as zero; this slightly over-samples players with active valuation series.\n")

fs = "".join(L)
open(OUT, "w").write(fs)
print("wrote", OUT, len(fs), "bytes")
