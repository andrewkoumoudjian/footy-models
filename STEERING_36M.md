# STEERING — 36-Month Forecast: U23, TM value €400k–€1.2M, Top-100 by % Gain

## Mission

Score the 36-month value forecast for **all players aged < 23 with Transfermarkt value between €400,000 and €1,200,000**, using the **optimized v2 forecast model** (ridge = 1,000,000, seed 42), and produce a report of the **top 100 by expected % gain** with 36-month Monte Carlo fan charts. Charts in this report must be on a **WHITE background** (unlike the earlier dark-themed reports).

Read `architecture.md` and `AGENTS.md` first. This is a SCORING + REPORTING run: do not retrain, do not re-sweep ridge, do not re-run the forward test. Use the deployed optimized model as-is.

## Model (hard requirement — use exactly this)

- Model: `/home/drewclaw/footy-models/data/forecast/model.bin`
- SHA-256 (must match): `1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f`
- Config: ridge = 1,000,000, seed 42, 47-feature causal schema, trained through 2025-06-12, walk-forward Spearman IC 0.5063 pooled OOF / **real forward-test IC 0.5100** (as-of 2025-11-06 → realized TM through 2026-06-12)
- Conformal 80% interval radius (12-month, from the v2 config): **0.6009** → annual σ = 0.6009 / 1.2816 ≈ **0.4688** for Monte Carlo paths.

## Candidate pool

- Source: `/home/drewclaw/footy-models/data/fm_tm_joined.csv` (14,783 rows; MERGING IS DONE — DO NOT RE-MERGE)
- Filter: `age < 23` AND `400000 <= current_mv <= 1200000` AND `attr_source != 'staff'`
- Pool count: **971 players** (849 db attrs / 99 derived / 23 staff — the 23 staff rows must be excluded; db vs derived stays flagged in all outputs)
- NOTE: this run has **no PA filter** — U23 + value band only (the user's spec).

## Scoring (use existing primitives)

1. Build a candidates TSV for the pool above (same schema `apps/train forecast-materialize-candidates` expects — check how `data/forecast/candidates.tsv` + `candidates_meta.tsv` were built and reuse that path; do not invent a new feature extractor).
2. Materialize the candidate matrix: `footy-train forecast-materialize-candidates --input candidates.tsv --dataset-version N --feature-set-version N --output candidates_matrix.bin` (reuse the dataset/feature versions from the existing artifacts: dataset 1, feature-set 1).
3. Score: `footy-score forecast --model data/forecast/model.bin --candidates candidates_matrix.bin --meta candidates_meta.tsv --top-n 1000 --output data/forecast_36m_all.csv` (output ALL scored players, not just 100).
4. Compute 36-month expected values (model-consistent compounding of the 12m log forecast):
   - `mu_12 = ln(predicted_12m_mv / current_mv)` (the model's 12-month log ratio)
   - `predicted_36m_mv = current_mv * exp(3 * mu_12)`
   - `gap_pct_36m = exp(3 * mu_12) - 1` → rank by this, descending
   - Keep the 12m columns too (the conformal interval_10/interval_90 are 12m; for the 36m chart uncertainty use the MC fan).
5. Top 100 by `gap_pct_36m` → `data/forecast_36m_top100.csv` (100 rows, schema like the existing top-100: player_id, name, club, competition, country, position, pos_cls, age, current_mv, fm_ca, fm_pa, attr_source, predicted_12m_mv, gap_eur (36m € gap), gap_pct (36m), plus rationale/key_strengths kept from the scoring meta).

## 36-month Monte Carlo charts (white background)

- Use the existing Rust MC primitives: `apps/mc` (`footy-mc run`) with `crates/forecast` `FootballPathModel` / `monte_carlo_paths` — extend if needed (paths_long.tsv/paths_basket.tsv already produced by that app).
- Per top-10 player (or top-25 for a basket overview): drift `mu_ann = mu_12` (annualized model forecast), vol `sigma_ann = 0.4688` (v2 conformal radius), 1,000 paths, 36 monthly steps, seed 42. Use `data/mc_input.tsv` as the template for the app's input TSV.
- CHART STYLE — WHITE BACKGROUND (this is a hard requirement):
  - figure/axes facecolor: `#ffffff`, text/ticks/labels: dark gray `#1f2328` / `#57606a`, grid: light `#d0d7de`
  - paths faint blue `#9cc3f5` (alpha ~0.5, thin), mean path bold `#c62828` (red) or `#d97706` (amber), median dashed dark gray, 10–90% band light blue `#d6e6fb`
  - Same layout as the previous report: basket overview + individual fans, subtitle below the x-axis
- Charts saved under `artifacts/mc_36m_white/`.

## Deliverables

- `data/forecast_36m_top100.csv` (100 rows, ranked by 36m % gain) + `data/forecast_36m_all.csv` (all ~971 scored)
- `data/forecast_36m_report.md` — methodology (model hash, pool filter, compounding), top-100 table (all 100), top-10 profile cards with MC stats (median/mean 36m, P(≥1.5×), P(≥2×), P(transfer 3y)), basket overview stats, white-background chart inventory
- PDF report `data/forecast_36m_report.pdf` (white background, charts embedded, tables readable on white)
- Do NOT overwrite: `data/forecast_u23_top100.csv`, `data/forecast_u23_top100_v2.csv`, `data/forecast/model.bin`

## Non-negotiables

- Deterministic: seed 42 everywhere; report the seed.
- The v2 model file must be byte-identical after the run (SHA-256 check in the gate).
- attr_source honesty: flag db vs derived in every output; the staff rows are excluded.
- Missing data is not zero. Censored/absent players are dropped, not zero-filled.
- DO NOT RE-MERGE `fm_tm_joined.csv`. DO NOT retrain or re-sweep. DO NOT touch the Transfermarkt DuckDB (read-only).
- Rust-first for scoring; Python only for reporting/PDF.
- 15 bps costs apply wherever a portfolio is evaluated (top-N tables may be gross, labeled as such).

## Gate

`bash scripts/gate_check_36m.sh` (already on disk). It requires:
1. `cargo test --release` passes (footy-forecast, footy-backtest, footy-models).
2. `data/forecast_36m_top100.csv` exists with exactly 100 rows and every row satisfies age < 23 AND 400000 <= current_mv <= 1200000 AND attr_source != 'staff'; ranked by gap_pct_36m descending; no NaN.
3. `data/forecast_36m_all.csv` has >= 900 rows, same filter constraints.
4. `data/forecast_36m_report.md` exists and contains: the model SHA-256 `1d023e96…`, the pool definition, the 36m compounding formula, the top-100 table, and MC stats.
5. `data/forecast_36m_report.pdf` exists AND contains white-background charts (the gate checks for the chart files under `artifacts/mc_36m_white/` with white backgrounds by inspecting that the PNGs are present and not the dark theme — verify via at least one visual check or assert the renderer used the white palette).
6. `data/forecast/model.bin` SHA-256 still `1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f` (untouched).

Iterate until the gate passes. A passing gate is the completion criterion.
