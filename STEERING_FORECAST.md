# STEERING — Footy-Models 12-Month Value Forecast (U23, PA>=140, MV<=1M)

You are the implementation agent for the `footy-models` Rust workspace. Read
`architecture.md` and the repo `AGENTS.md` FIRST. Your job is to build the
**12-month market-value forecast** model and produce the **top-100 players by
expected value gap** for the specific population below.

## Task

Forecast each candidate's Transfermarkt market value 12 months ahead, then
rank by the gap (predicted 12m value minus current value, in EUR and %).

Target population (filter at SCORING time, applied to the output):
- **age < 23** (use `age` column, floating years as of 2026-06-12)
- **FM potential ability >= 140**: use `pa_eff` (already computed in
  `forecast_candidates.csv`) — negative FM PA band codes must be converted to
  band midpoints FIRST, then compared (band table below)
- **current_mv <= 1,000,000 EUR** and > 0
- `attr_source` in {`db`, `derived`} (exclude `staff`/`none`)

Deliverable: `data/forecast_u23_top100.csv` — exactly 100 rows, ranked by
`gap_eur` descending, plus `data/forecast_report.md`.

## Data (already prepared — DO NOT RE-MERGE, DO NOT REBUILD)

### ⚠️ CRITICAL — data plumbing is DONE. Model work only.
- `data/forecast/train.tsv` — **340,870 strictly causal training rows**, 47
  features + `player_id, t, mv_t, mv_12m, log_ratio, realized_return`.
  Built by `data/build_forecast_dataset.py` (inspect for exact schema):
  target `log_ratio = log(mv_12m / mv_t)` where mv_12m is the valuation
  closest to t+365 within (t, t+365]; censored rows dropped; band
  10k <= mv_t <= 1M. 28.9% of rows have realized return >= +50%.
- `data/forecast/candidates.tsv` — **7,969 candidate rows** (same 47 features,
  as-of each player's latest valuation date) + `player_id, t, mv_t`.
- `data/forecast/candidates_meta.tsv` — metadata per candidate: `player_id,
  uid, name, current_club_name, competition_name, country_name, position,
  pos_cls, age, current_mv, latest_val_date, ca, pa, attr_source, pa_gap,
  ca_age_pct` + 18 FM attributes (attr_Finishing ... attr_OffTheBall).
- `data/forecast_candidates.csv` — raw candidate table with `pa_eff` column
  (FM PA band midpoint conversion already applied; use it for the PA>=140
  filter; `pa` is the raw value incl. negative band codes).
- `data/fm_universe.csv` — 563k FM men (context/reference distributions only).

FM negative-PA band conversion (for documentation):
-95→185, -85→175, -75→165, -65→155, -55→145, -45→135, -35→125, -25→115,
-15→105, -10→185, -9→165, -8→145, -7→125, -6→105, -5→85, -4→65, -3→45,
-2→30, -1→20.

## Model requirements

1. **Regression on `log_ratio`** (12m log value ratio), not classification.
   Prediction → `predicted_12m_mv = mv_t * exp(predicted_log_ratio)`.
2. **Ridge OLS** with L2 regularization (closed-form, deterministic, seed 42).
   Same feature scheme as the surge model: standardize each feature
   (mean/std fit on training rows) + paired missingness indicator → 94 model
   inputs. Missing is not zero.
3. Implement in Rust:
   - `crates/models`: add ridge-OLS fit/predict (deterministic; no external
     LA deps — implement small solve or use existing patterns) + tests.
   - `crates/artifacts`: forecast model artifact (weights, transform stats)
     and feature-matrix/batch artifacts (follow existing surge artifact
     patterns).
   - `apps/train` + `apps/score`: forecast subcommands (or a dedicated app
     `apps/forecast`) that run end-to-end: train on train.tsv, score
     candidates.tsv, emit ranked CSV.
4. **Walk-forward validation** (same cutoffs as surge: 2020-06-01 ...
   2024-06-01, test window = (cutoff, cutoff+365]): report per-cutoff and
   pooled RMSE, MAE, Spearman IC (predicted vs realized log_ratio), and a
   decile table (mean predicted, mean realized log_ratio, mean realized
   return per decile). Use `crates/backtest` classification helpers where
   applicable (add regression metrics).
5. **Uncertainty**: conformal 80% intervals on the log-ratio (absolute
   residual quantile from OOF), reported per player as `interval_10`,
   `interval_90` on predicted_12m_mv. Validate empirical coverage.

## Output format

`data/forecast_u23_top100.csv` columns (exact):
`player_id, name, club, competition, country, position, pos_cls, age,
current_mv, fm_ca, fm_pa, attr_source, predicted_12m_mv, gap_eur, gap_pct,
interval_10, interval_90, key_strengths, rationale`

- `current_mv` = the candidate's latest Transfermarkt value (from
  candidates_meta, EUR).
- `gap_eur = predicted_12m_mv - current_mv`, `gap_pct = gap_eur / current_mv`.
- `fm_pa` = effective PA (band midpoint conversion applied).
- `key_strengths`: top 4 FM attributes by value (from candidates_meta
  attr_* columns) formatted like "Finishing 15; Dribbling 14; Pace 14;
  FirstTouch 13".
- `rationale`: one line combining: age/PA upside (PA gap, age-relative CA
  percentile), current MV vs predicted, market context (competition,
  position). E.g. "U19 FW, PA 145 (gap 30), CA percentile 0.97; model sees
  3.1x 12m appreciation from EUR 350k".
- Rank by `gap_eur` descending; tie-break by `player_id` ascending for
  determinism. Exactly 100 rows.

`data/forecast_report.md`: model spec, features, target definition, walk-
forward metrics, decile table, filter counts (how many U23/PA>=140/MV<=1M),
position/league mix of the top-100, limitations, reproducibility.

## Iteration mandate

Do not stop at the first working version. If pooled Spearman IC < 0.20 or
the decile table is not strongly monotonic, iterate: feature interactions
(age x FM signals), better missing handling, target inspection (outliers),
hyperparameter sweep (ridge lambda) — until metrics are solid. Then rerun
everything end-to-end and confirm the gate.

## Non-negotiables (from AGENTS.md)

- Strictly causal/as-of features. No future observations, ever.
- Missing is not zero (paired missingness indicators).
- Deterministic: same input -> byte-identical output.
- Do NOT re-merge data files. Do NOT rebuild train.tsv. If a pandas op
  would take > 5 minutes, stop and reconsider.
- Commit nothing. Leave working artifacts + tests.
